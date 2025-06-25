import { Viewer } from '@photo-sphere-viewer/core';
import '@photo-sphere-viewer/core/index.css';
import './style.css';


const viewer = new Viewer({
    container: document.querySelector('#viewer'),
    keyboard: "always",
    panorama: window.location.pathname.endsWith("index.html") ? "demo.webp" : "",
    caption: "",
    navbar: ["zoom", "move", "caption", "fullscreen"],
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
    }
});

// Keep reference to blob url for memory management
let currentObjectUrl = null;

window.setPanoramaImageFromBase64 = function (base64, mimeType) {
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
        viewer.setPanorama(currentObjectUrl);
    } catch (error) {
        console.error('Error loading panorama image:', error);
    }
};