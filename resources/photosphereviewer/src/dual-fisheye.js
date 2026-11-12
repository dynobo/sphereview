import { CONSTANTS, utils } from '@photo-sphere-viewer/core';
import { Mesh, ShaderMaterial, SphereGeometry, Vector4 } from 'three';

/*
 * Dual fisheye projection.
 *
 * An Insta360 photo is a JPEG holding both lenses' fisheye circles side by side,
 * plus a trailer that says where each circle sits and how wide it sees. Rather
 * than baking an equirectangular image - which resamples everything once more and
 * throws away the rim of each circle - the circles are sampled directly, per
 * pixel, as the sphere is drawn. No intermediate image, nothing to blur, and the
 * resolution follows the viewport.
 *
 * The shader is a port of PSV's standalone dual-fisheye adapter (MIT), with its
 * hardcoded 180 degrees and circle size replaced by per-file uniforms. Those
 * uniforms are read from the camera's calibration by `src/insta360.rs`.
 *
 * Three things were established by measuring real Insta360 photos (a ONE RS and a
 * ONE X2), not by reading the format's docs - none of it is documented:
 *
 *  - the lens axes are exactly opposite, so lens 1 is modeled as lens 0 turned
 *    half a turn about the vertical axis (one axis flip and one x flip)
 *  - `fov_deg` is what the lens sees *across* the disc, so its rim is at half
 *    that angle from the axis
 *  - the calibration's z rotation is the direction the lens faces - the two
 *    lenses differ by ~180 degrees - and *not* the roll of the image. Using it as
 *    a roll rotated one hemisphere 180 degrees against the other.
 *
 * The sign of the x axis was settled against a photograph, not derived: PSV's
 * frame, the mirrored sphere geometry and three's camera handedness are easy to
 * combine the wrong way round, and reasoning about them got it wrong twice. A
 * parking-garage sign visible in both lenses of `IMG_20220716_142858_00_010`
 * reads unmirrored with this mapping and mirrored with the opposite one.
 *
 * The two circles overlap by ~15 degrees, which is what makes this measurable:
 * in the overlap both lenses see the same directions, so a misaligned convention
 * shows up as a large difference between them.
 *
 * Not applied, because it is worth about a degree at the seam: the calibration's
 * relative lens yaw (`yaw_deg`, ~180 degrees off exact) and each lens's x and y
 * tilts. Each is a uniform away if a file ever needs it.
 */

/** Cross-fade half-width at the seam, as the cosine of the angle to the horizon. */
const FEATHER = 0.05;

export const GUESSED_NOTICE =
    'No lens calibration in this file, assuming two 180\u00b0 circles filling the frame.';

const VERTEX_SHADER = /* glsl */ `
varying vec3 vPosition;

void main() {
    vPosition = position;
    gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
}
`;

const FRAGMENT_SHADER = /* glsl */ `
varying vec3 vPosition;

uniform sampler2D map;
uniform float opacity;

// cx/H, cy/H, radius/H, and the angle at the rim in radians. H is the image
// height, so a circle stays a circle; sampling then rescales x by height/width.
uniform vec4 lens0;
uniform vec4 lens1;

uniform float uScale;
uniform float feather;

// Lens 0 faces +Z and lens 1 faces -Z, both with +Y up. The sense argument turns
// the back lens into the front one: it flips the axis and the x direction
// together, which is a half turn, not a mirror.
//
// The sign of x is the one thing here that was settled against a photograph
// rather than derived - see the note at the top of the file.
vec2 fisheyeUv(vec3 dir, vec4 lens, float sense) {
    float cosTheta = clamp(dir.z * sense, -1.0, 1.0);
    float theta = acos(cosTheta);

    // Direction in the lens image's own (right, up) frame.
    vec2 t = vec2(sense * dir.x, dir.y);

    // Equidistant lens: radius grows linearly with the angle from the axis.
    float rho = clamp(theta / lens.w, 0.0, 1.0);
    vec2 n = length(t) > 1e-6 ? normalize(t) : vec2(1.0, 0.0);

    // Image coordinates, normalised by height and measured down from the top.
    float x = lens.x + lens.z * rho * n.x;
    float y = lens.y - lens.z * rho * n.y;

    return vec2(x * uScale, 1.0 - y);
}

void main() {
    // The sphere geometry is mirrored on x (see createDualFisheyeMesh), so the
    // object-space position runs the other way round the sphere than the
    // direction it is seen from. PSV's own shaders compensate the same way.
    vec3 dir = normalize(vec3(-vPosition.x, vPosition.y, vPosition.z));

    vec3 color;
    if (dir.z > feather) {
        color = texture2D(map, fisheyeUv(dir, lens0, 1.0)).rgb;
    } else if (dir.z < -feather) {
        color = texture2D(map, fisheyeUv(dir, lens1, -1.0)).rgb;
    } else {
        // The lenses overlap here; both look at the same direction, so cross-fade.
        float weight = smoothstep(-feather, feather, dir.z);
        color = mix(
            texture2D(map, fisheyeUv(dir, lens1, -1.0)).rgb,
            texture2D(map, fisheyeUv(dir, lens0, 1.0)).rgb,
            weight
        );
    }

    gl_FragColor = vec4(color, opacity);
}
`;

/**
 * Geometry assumed when a file carries no calibration: the layout nearly every
 * consumer 360 camera uses, two 180 degree circles filling the image height.
 * Close enough to look around in, not close enough to match the camera's own
 * stitching, which is why the viewer says so when it is used.
 */
function guessedParams(width, height) {
    const radius = height / 2;
    const lens = (cx) => ({ radius, cx, cy: radius, rotation: [0, 0, 0] });
    return {
        lens_count: 2,
        lens: [lens(radius), lens(width - radius)],
        width,
        height,
        fov_deg: 180,
        yaw_deg: 0,
    };
}

/** Where the lens sits in the image: centre and radius over the image height,
 * and the angle its rim subtends. */
function lensSize(params, index) {
    const lens = params.lens[index];
    const height = params.height;
    return new Vector4(
        lens.cx / height,
        lens.cy / height,
        lens.radius / height,
        // fov_deg is measured across the disc, so the rim is at half of it.
        (params.fov_deg * Math.PI) / 360,
    );
}

/**
 * Loads a panorama whose `projection` asks for the dual fisheye view.
 *
 * The calibration travels on the panorama as `projection`; `{ fallback: true }`
 * means the camera's calibration was not available and the layout is a guess.
 * (`lens_count`, `model`, the lens rotations and `yaw_deg` ride along unread:
 * the rotations say which way each lens faces, which the shader assumes.)
 */
export async function loadDualFisheyeTexture(viewer, panorama, loader) {
    const progress = loader ? (p) => viewer.textureLoader.dispatchProgress(p) : null;
    const image = await viewer.textureLoader.loadImage(panorama.path, progress, panorama.path);

    const guessed = panorama.projection.fallback === true;
    if (guessed) {
        // Deferred: the viewer is mid-`setPanorama`, its notification area not ready yet.
        setTimeout(() => viewer.notification.show({ content: GUESSED_NOTICE, timeout: 8000 }), 0);
    }

    return {
        panorama,
        texture: utils.createSizedTexture(image),
        panoData: {
            fullWidth: image.width,
            fullHeight: image.height,
            croppedWidth: image.width,
            croppedHeight: image.height,
            croppedX: 0,
            croppedY: 0,
            dualFisheye: guessed ? guessedParams(image.width, image.height) : panorama.projection,
        },
    };
}

/** True for the meshes created here, so the adapter can leave the rest alone. */
export function isDualFisheyeMesh(mesh) {
    return mesh.userData.dualFisheye === true;
}

export function createDualFisheyeMesh(params) {
    // Finer than PSV's default: the fisheye mapping is exact per pixel, so the
    // mesh is the only place tessellation error can creep in.
    const geometry = new SphereGeometry(CONSTANTS.SPHERE_RADIUS, 128, 64).scale(-1, 1, 1);
    const material = new ShaderMaterial({
        uniforms: {
            map: { value: null },
            opacity: { value: 1 },
            lens0: { value: lensSize(params, 0) },
            lens1: { value: lensSize(params, 1) },
            uScale: { value: params.height / params.width },
            feather: { value: FEATHER },
        },
        vertexShader: VERTEX_SHADER,
        fragmentShader: FRAGMENT_SHADER,
        depthTest: false,
        depthWrite: false,
        transparent: true,
    });

    const mesh = new Mesh(geometry, material);
    mesh.userData.dualFisheye = true;
    return mesh;
}

export function setDualFisheyeTexture(mesh, texture) {
    mesh.material.uniforms.map.value = texture;
}

export function setDualFisheyeOpacity(mesh, opacity) {
    mesh.material.uniforms.opacity.value = opacity;
}
