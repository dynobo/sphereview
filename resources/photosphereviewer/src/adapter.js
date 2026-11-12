import { EquirectangularAdapter } from '@photo-sphere-viewer/core';
import {
    createDualFisheyeMesh,
    isDualFisheyeMesh,
    loadDualFisheyeTexture,
    setDualFisheyeOpacity,
    setDualFisheyeTexture,
} from './dual-fisheye.js';

/*
 * The viewer creates its adapter once, so a single class serves both kinds of
 * panorama this app opens: an ordinary equirectangular image, and an Insta360
 * photo rendered straight from its two fisheye circles.
 *
 * Everything that is not dual fisheye - equirectangular images, XMP GPano data,
 * partial panoramas - goes through PSV untouched. The fisheye half lives in
 * ./dual-fisheye.js; this class only decides which of the two answers a call.
 *
 * `projection` is this app's own field on the panorama, filled in by
 * `setPanoramaImageFromBase64`: the camera's calibration, or `{ fallback: true }`
 * when the file has none.
 */
export class SphereViewAdapter extends EquirectangularAdapter {
    static id = 'sphereview';
    // PSV warns when an adapter's version drifts from the core's; follow the
    // parent, which is the version this adapter is written against.
    static VERSION = EquirectangularAdapter.VERSION;
    static supportsDownload = true;

    static withConfig(config) {
        return [SphereViewAdapter, config];
    }

    async loadTexture(panorama, loader, panoData) {
        if (!panorama?.projection) {
            return super.loadTexture(panorama, loader, panoData);
        }
        return loadDualFisheyeTexture(this.viewer, panorama, loader);
    }

    createMesh(panoData) {
        const params = panoData?.dualFisheye;
        return params ? createDualFisheyeMesh(params) : super.createMesh(panoData);
    }

    setTexture(mesh, textureData) {
        if (isDualFisheyeMesh(mesh)) {
            setDualFisheyeTexture(mesh, textureData.texture);
        } else {
            super.setTexture(mesh, textureData);
        }
    }

    setTextureOpacity(mesh, opacity) {
        if (isDualFisheyeMesh(mesh)) {
            setDualFisheyeOpacity(mesh, opacity);
        } else {
            super.setTextureOpacity(mesh, opacity);
        }
    }
}
