import assert from 'node:assert/strict';
import test from 'node:test';
import { createDualFisheyeMesh } from './dual-fisheye.js';

/*
 * Regression tests for the projection conventions.
 *
 * Both cases below were wrong in the first implementation, and both were found
 * by measuring a real Insta360 photo rather than by looking at the synthetic
 * fixture - which was generated with the same wrong assumptions, so it agreed
 * with the shader and proved nothing.
 */

// Calibration exactly as an Insta360 ONE RS writes it (IMG_20220716_142858_00_010).
const calibration = {
    lens_count: 2,
    width: 6528,
    height: 3264,
    fov_deg: 194,
    lens: [
        { radius: 1632.3, cx: 1605.97, cy: 1629.61, rotation: [-0.045, -0.138, 179.385] },
        { radius: 1629.17, cx: 4870.5, cy: 1627.02, rotation: [0.15, -0.366, 0.672] },
    ],
};

/**
 * The shader's mapping, kept in step with FRAGMENT_SHADER: sphere direction to
 * image pixel. Lens 0 looks along +Z, lens 1 along -Z, both with +Y up.
 */
function samplePoint(dir, uniform) {
    const [cx, cy, radius, rim] = uniform.value.toArray();
    const sense = uniform === uniforms.lens0 ? 1 : -1;
    const theta = Math.acos(Math.min(1, Math.max(-1, dir[2] * sense)));
    let [tx, ty] = [sense * dir[0], dir[1]];
    const length = Math.hypot(tx, ty);
    if (length > 1e-6) {
        [tx, ty] = [tx / length, ty / length];
    }
    const rho = theta / rim;
    return {
        x: cx + radius * rho * tx,
        y: cy - radius * rho * ty,
        rho,
    };
}

const mesh = createDualFisheyeMesh(calibration);
const uniforms = mesh.material.uniforms;

test('the rim of the circle is at half the stated field of view', () => {
    // 194 degrees is what the lens sees across the disc, so its rim sits 97
    // degrees from the axis. Using 194 as the angle stretched the sphere by 2x.
    const rim = (uniforms.lens0.value.w * 180) / Math.PI;
    assert.ok(Math.abs(rim - calibration.fov_deg / 2) < 1e-6, `rim angle is ${rim} deg`);
});

test('a direction at the rim of the lens samples the rim of its circle', () => {
    const rim = uniforms.lens0.value.w;
    // 97 degrees off lens 0's axis, straight up from it.
    const dir = [0, Math.sin(rim), Math.cos(rim)];
    const point = samplePoint(dir, uniforms.lens0);
    assert.ok(Math.abs(point.rho - 1) < 1e-6, `sampled at radius ${point.rho} of the circle`);
});

test('content above the seam stays above the seam in both lenses', () => {
    // The seam is the great circle z = 0, where the two circles overlap. A
    // direction just above it must land above the centre of *both* images: the
    // lenses face opposite ways, but neither image is upside down.
    const above = [Math.cos(0.05), Math.sin(0.05), 0];
    for (const [name, uniform] of [['lens0', uniforms.lens0], ['lens1', uniforms.lens1]]) {
        const [, cy] = uniform.value.toArray();
        const point = samplePoint(above, uniform);
        assert.ok(point.y < cy, `${name} sampled below its centre, so that half is upside down`);
    }
});

test('the disc keeps the photo unmirrored', () => {
    // Anchored on a real photo rather than derived: with lens 0 facing +Z and +Y
    // up, +X has to sample the right half of lens 0's circle. That is the mapping
    // that shows the "LEARANCE 13'-8\"" sign in IMG_20220716_142858 unmirrored;
    // the opposite sign mirrors the whole sphere. It is worth keeping as a test
    // because combining PSV's frame, the mirrored sphere geometry and three's
    // camera handedness got it wrong twice.
    const [cx] = uniforms.lens0.value.toArray();
    const point = samplePoint([1, 0, 0], uniforms.lens0);
    assert.ok(point.x > cx, 'the sphere is mirrored: +X samples the left of the circle');
});
