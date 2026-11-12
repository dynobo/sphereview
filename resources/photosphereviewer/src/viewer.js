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

/*
 * A clockwise arrow, on the same 100x100 grid as the bundled icons and with the
 * same 11 unit stroke, which is what the zoom buttons' ring is drawn with.
 *
 * The ring runs clockwise from 2 o'clock round to 12 o'clock, so its head sits
 * where the turn ends and points the way the content goes: to the right. The
 * head is a triangle whose base is centred on the ring's centreline and whose
 * axis is the tangent there, so it grows out of the stroke rather than hanging
 * off one side of it. Its corners are arcs of their own radius, since a fill has
 * no line joins to round off.
 *
 * The class is the one the viewer puts on the icons of its own buttons, and it
 * carries the sizing and the hover zoom; buttons given `content` do not get it.
 */
const ROTATE_ICON = `
    <svg class="psv-button-svg" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
        <path fill="none" stroke="currentColor" stroke-width="11" stroke-linecap="round"
              d="M78.19 43.74A30 30 0 1 1 52.61 24.11"/>
        <path fill="currentColor"
              d="M75.68 24.39L58.18 12.15A3 3 0 0 0 53.47 14.35L51.76 33.88A3 3 0 0 0 56.02 36.86L75.37 27.84A2 2 0 0 0 75.68 24.39Z"/>
    </svg>`;

// The rotation each photo was last left at, for this run only, keyed by the hash
// of its path (src/window.rs). A projection switch re-sends the same file and
// the viewer would otherwise come back level, and a photo opened again later
// comes back as it was left.
const rollByFile = new Map();

// The file the viewer is showing, so the rotate button knows whose rotation it
// is changing.
let currentFileKey;

function rememberedRoll(fileKey) {
    return rollByFile.get(fileKey) ?? 0;
}

function rotatePanorama(viewer) {
    const correction = viewer.config.sphereCorrection ?? {};
    // The option's parser keeps roll within [-PI, PI].
    const roll = (((correction.roll ?? 0) + ROTATE_STEP + Math.PI) % (2 * Math.PI)) - Math.PI;
    rollByFile.set(currentFileKey, roll);
    viewer.setOption('sphereCorrection', { ...correction, roll });
}

const rotateButton = {
    id: 'sphereview-rotate',
    title: 'Rotate 90°',
    content: ROTATE_ICON,
    // Custom buttons hard-code hoverScale to false, but the class it stands for
    // is all the stylesheet needs to zoom the icon on hover like its neighbours.
    className: 'psv-button--hover-scale',
    onClick: rotatePanorama,
};

/*
 * The roll turns the two lens circles of a dual fisheye photo, so the button is
 * only offered for that projection: a flat panorama brings its own horizon.
 *
 * The button stays in the navbar and is shown or hidden instead, which keeps the
 * layout right - the navbar leaves hidden buttons out when it decides whether
 * the others still fit.
 */
function setRotateButtonVisible(visible) {
    window.SphereViewer.navbar.getButton(rotateButton.id, false)?.toggle(visible);
}

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

// The viewer opens on the flat demo image, where there is nothing to rotate.
setRotateButtonVisible(false);

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
window.setPanoramaImageFromBase64 = function (base64, mimeType, projection, fileKey) {
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

        // The roll belongs to the photo, not to the projection: a flat view stays
        // level, and the rotation waits for the dual fisheye view. Passing
        // `sphereCorrection` every time says so, rather than relying on the
        // viewer handing the last image's correction to the next one.
        currentFileKey = fileKey;
        const correction = window.SphereViewer.config.sphereCorrection ?? {};
        const roll = projection ? rememberedRoll(fileKey) : 0;

        setRotateButtonVisible(Boolean(projection));
        window.SphereViewer.setPanorama(
            projection ? { path: currentObjectUrl, projection } : currentObjectUrl,
            { sphereCorrection: { ...correction, roll } }
        );
    } catch (error) {
        console.error('Error loading panorama image:', error);
    }
};
