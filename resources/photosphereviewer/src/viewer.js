import { AutorotatePlugin } from '@photo-sphere-viewer/autorotate-plugin';
import { CONSTANTS, EquirectangularAdapter, Viewer, utils } from '@photo-sphere-viewer/core';
import '@photo-sphere-viewer/core/index.css';
import './style.css';
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
 * This adapter extends PSV's equirectangular adapter and only takes over for
 * panoramas that ask for a projection. Everything else - ordinary
 * equirectangular images, XMP GPano data, partial panoramas - still goes through
 * PSV untouched.
 *
 * The shader is a port of PSV's dual-fisheye adapter (MIT), with its hardcoded
 * 180 degrees and circle size replaced by per-file uniforms. Those uniforms are
 * read from the camera's calibration by `src/insta360.rs`.
 *
 * What is a convention rather than a measurement (to be checked against a real
 * file, which is the point of the opt-in tests):
 *
 *  - lens 0 faces +Z, lens 1 faces -Z, the camera body being the panorama frame
 *  - the calibration's z rotation is the roll of that lens's image
 *  - lens 1's image runs the other way along x, so its projection is not mirrored
 *
 * Each is a uniform, so correcting one is a one-line change here.
 */

/** Cross-fade half-width at the seam, as the cosine of the angle to the horizon. */
const FEATHER = 0.05;

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

// cx/H, cy/H, radius/H, theta_max in radians. H is the image height, so a
// circle stays a circle; sampling then rescales x by height/width.
uniform vec4 lens0;
uniform vec4 lens1;

// roll in radians, x axis sense, lens axis sense, unused
uniform vec4 orient0;
uniform vec4 orient1;

uniform float uScale;
uniform float feather;

const float PI = 3.1415926535897932384626433832795;

vec2 fisheyeUv(vec3 dir, vec4 lens, vec4 orient) {
    float cosTheta = clamp(dir.z * orient.z, -1.0, 1.0);
    float theta = acos(cosTheta);

    // Direction in the lens image's own (right, up) frame, then rolled.
    vec2 t = vec2(dir.x * orient.y, dir.y);
    float c = cos(orient.x);
    float s = sin(orient.x);
    t = vec2(c * t.x - s * t.y, s * t.x + c * t.y);

    // Equidistant lens: radius grows linearly with the angle from the axis.
    float rho = clamp(theta / lens.w, 0.0, 1.0);
    vec2 n = length(t) > 1e-6 ? normalize(t) : vec2(1.0, 0.0);

    // Image coordinates, normalised by height and measured down from the top.
    float x = lens.x + lens.z * rho * n.x;
    float y = lens.y - lens.z * rho * n.y;

    return vec2(x * uScale, 1.0 - y);
}

void main() {
    // The sphere geometry is mirrored on x (see createMesh), so the object-space
    // position runs the other way round the sphere than the direction it is seen
    // from. PSV's own shaders compensate the same way.
    vec3 dir = normalize(vec3(-vPosition.x, vPosition.y, vPosition.z));

    vec3 color;
    if (dir.z > feather) {
        color = texture2D(map, fisheyeUv(dir, lens0, orient0)).rgb;
    } else if (dir.z < -feather) {
        color = texture2D(map, fisheyeUv(dir, lens1, orient1)).rgb;
    } else {
        // The lenses overlap here; both look at the same direction, so cross-fade.
        float weight = smoothstep(-feather, feather, dir.z);
        color = mix(
            texture2D(map, fisheyeUv(dir, lens1, orient1)).rgb,
            texture2D(map, fisheyeUv(dir, lens0, orient0)).rgb,
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
        theta_max_deg: 180,
        yaw_deg: 0,
    };
}

const GUESSED_NOTICE =
    'No lens calibration in this file, assuming two 180\u00b0 circles filling the frame.';

/** Where the lens sits in the image, normalised by the image height. */
function lensSize(params, index) {
    const lens = params.lens[index];
    const height = params.height;
    return new Vector4(
        lens.cx / height,
        lens.cy / height,
        lens.radius / height,
        (params.theta_max_deg * Math.PI) / 180,
    );
}

/** How that lens's image is turned in the camera body. */
function lensOrient(params, index) {
    const roll = (params.lens[index].rotation[2] * Math.PI) / 180;
    const sense = index === 0 ? 1 : -1;
    return new Vector4(roll, sense, sense, 0);
}

class DualFisheyeAdapter extends EquirectangularAdapter {
    static id = 'sphereview-dual-fisheye';
    // PSV warns when an adapter's version drifts from the core's; follow the
    // parent, which is the version this adapter is written against.
    static VERSION = EquirectangularAdapter.VERSION;
    static supportsDownload = true;

    static withConfig(config) {
        return [DualFisheyeAdapter, config];
    }

    async loadTexture(panorama, loader, panoData) {
        const requested = panorama && typeof panorama === 'object' ? panorama.projection : null;
        if (!requested) {
            return super.loadTexture(panorama, loader, panoData);
        }

        const progress = loader ? (p) => this.viewer.textureLoader.dispatchProgress(p) : null;
        const image = await this.viewer.textureLoader.loadImage(panorama.path, progress, panorama.path);

        // `fallback` means the camera's calibration was not available.
        const guessed = requested.fallback === true;
        if (guessed) {
            setTimeout(() => this.viewer.notification.show({ content: GUESSED_NOTICE, timeout: 8000 }), 0);
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
                dualFisheye: guessed ? guessedParams(image.width, image.height) : requested,
            },
        };
    }

    createMesh(panoData) {
        const params = panoData?.dualFisheye;
        if (!params) {
            return super.createMesh(panoData);
        }

        // Finer than PSV's default: the fisheye mapping is exact per pixel, so the
        // mesh is the only place tessellation error can creep in.
        const geometry = new SphereGeometry(CONSTANTS.SPHERE_RADIUS, 128, 64).scale(-1, 1, 1);
        const material = new ShaderMaterial({
            uniforms: {
                map: { value: null },
                opacity: { value: 1 },
                lens0: { value: lensSize(params, 0) },
                lens1: { value: lensSize(params, 1) },
                orient0: { value: lensOrient(params, 0) },
                orient1: { value: lensOrient(params, 1) },
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

    setTexture(mesh, textureData) {
        if (!mesh.userData.dualFisheye) {
            return super.setTexture(mesh, textureData);
        }
        mesh.material.uniforms.map.value = textureData.texture;
    }

    setTextureOpacity(mesh, opacity) {
        if (!mesh.userData.dualFisheye) {
            return super.setTextureOpacity(mesh, opacity);
        }
        mesh.material.uniforms.opacity.value = opacity;
    }
}

window.SphereViewer = new Viewer({
    container: document.querySelector('#viewer'),
    adapter: DualFisheyeAdapter,
    keyboard: "always",
    panorama: window.location.pathname.endsWith("index.html") ? "demo.webp" : "",
    caption: "",
    navbar: ["autorotate", "zoom", "move", "caption", "fullscreen"],
    keyboardActions: {
        // Note keys which might need modifiers (e.g. '+') do not work reliably!
        'ArrowUp': 'ROTATE_UP',
        'ArrowDown': 'ROTATE_DOWN',
        'ArrowRight': 'ROTATE_RIGHT',
        'ArrowLeft': 'ROTATE_LEFT',
        'PageUp': 'ZOOM_IN',
        'PageDown': 'ZOOM_OUT',
        // Vim like
        'k': 'ROTATE_UP',
        'j': 'ROTATE_DOWN',
        'l': 'ROTATE_RIGHT',
        'h': 'ROTATE_LEFT',
        'i': 'ZOOM_IN',
        'o': 'ZOOM_OUT',
    },
    plugins: [
        [AutorotatePlugin, {
            autostartDelay: null,
            autostartOnIdle: false,
            autorotateSpeed: '1rpm',
        }],
    ],
});

// Remap +/- for zooming, as work around for keyboardActions not working properly 
// for keys which need modifiers
document.addEventListener('keydown', function (event) {
    if (event.key === '+') {
        event.preventDefault();
        window.dispatchEvent(new KeyboardEvent('keydown', { key: 'PageUp' }));
    }
    if (event.key === '-') {
        event.preventDefault();
        window.dispatchEvent(new KeyboardEvent('keydown', { key: 'PageDown' }));
    }
}, { capture: true });

// Keep reference to blob url for memory management
let currentObjectUrl = null;

// `projection` is absent for the flat view, holds the camera's calibration when
// the file has one, and `{ fallback: true }` when it does not.
window.setPanoramaImageFromBase64 = function (base64, mimeType, projection) {
    // Clean memory allocated for the last image's blob 
    if (currentObjectUrl) {
        URL.revokeObjectURL(currentObjectUrl);
        currentObjectUrl = null;
    }

    try {
        // Convert base64 to binary
        const binaryString = atob(base64);

        // Create byte array directly from the binary string
        const bytes = Uint8Array.from(binaryString, char => char.charCodeAt(0));

        // Create blob directly from the binary data
        const blob = new Blob([bytes], { type: mimeType });
        currentObjectUrl = URL.createObjectURL(blob);
        window.SphereViewer.setPanorama(
            projection ? { path: currentObjectUrl, projection } : currentObjectUrl
        );
    } catch (error) {
        console.error('Error loading panorama image:', error);
    }
};
