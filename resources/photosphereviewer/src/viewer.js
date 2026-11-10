import { AutorotatePlugin } from '@photo-sphere-viewer/autorotate-plugin';
import { Viewer } from '@photo-sphere-viewer/core';
import '@photo-sphere-viewer/core/index.css';
import './style.css';
import { SphereViewAdapter } from './adapter.js';

/*
 * Insta360 photos carry no record of which way was up when they were taken (see
 * the module docs of src/insta360.rs), and the cameras have no up of their own,
 * so a sphere arrives rolled by whatever angle the camera was held at - the two
 * photos this was measured against are upside down and a quarter turn over.
 *
 * This button turns the content a quarter turn at a time about the forward axis,
 * which is what a camera roll needs: the sensors' up and the world's up are both
 * perpendicular to that axis.
 */
const ROTATE_STEP = Math.PI / 2;

const ROTATE_ICON = `
    <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
        <path fill="currentColor" d="M50 22a28 28 0 1 1-19.8 8.2l-8.5-8.5A40 40 0 1 0 50 10z"/>
        <path fill="currentColor" d="M50 2 26 14l24 12z"/>
    </svg>`;

function rotatePanorama(viewer) {
    const correction = viewer.config.sphereCorrection ?? {};
    // The option's parser keeps roll within [-PI, PI].
    const roll = (((correction.roll ?? 0) + ROTATE_STEP + Math.PI) % (2 * Math.PI)) - Math.PI;
    viewer.setOption('sphereCorrection', { ...correction, roll });
}

const rotateButton = {
    id: 'sphereview-rotate',
    title: 'Rotate a quarter turn',
    content: ROTATE_ICON,
    onClick: rotatePanorama,
};

window.SphereViewer = new Viewer({
    container: document.querySelector('#viewer'),
    adapter: SphereViewAdapter,
    keyboard: "always",
    panorama: window.location.pathname.endsWith("index.html") ? "demo.webp" : "",
    caption: "",
    navbar: ["autorotate", "zoom", "move", rotateButton, "caption", "fullscreen"],
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
