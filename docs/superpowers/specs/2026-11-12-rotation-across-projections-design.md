# Rotation survives a projection switch

Date: 2026-11-12
Status: approved

## Intent

The rotate button turns a dual fisheye photo in quarter turns, because an
Insta360 file carries no record of which way was up. Switching the projection
re-sends the same file to the viewer, and the viewer comes back level, so the
work of straightening a photo is thrown away by a look at the equirectangular
view. A rotation given to a photo belongs to that photo for as long as the app
runs; a projection switch is the same photo shown differently and must not lose
it.

## What is wrong today

`sphereCorrection.roll` is viewer-wide config, not per-image state. PSV's
`setPanorama` deliberately inherits it when the caller passes no
`sphereCorrection` (`@photo-sphere-viewer/core/index.module.js:6175-6183`), and
`viewer.js` zeroes the roll on every load, because an equirectangular image must
not arrive tilted. Every load looks the same to the page, so a projection switch
and a new file are indistinguishable there. The Rust side knows the difference
(`show_panorama` vs `refresh_panorama`).

Note that yaw, pitch and zoom already survive a re-projection - PSV leaves the
camera alone when `setPanorama` gets no options. The roll is the odd one out.

## Goals

- A rotation survives any number of projection switches, including
  `photo A -> photo B -> photo A`.
- The equirectangular view of a rotated photo stays level; the rotation waits
  for the dual fisheye view.
- A plain equirectangular photo never inherits a rotation.

## Non-goals

- Persistence of any kind: nothing on disk, nothing in the photo file, nothing
  across restarts. The memory dies with the window.
- Rotation in equirectangular mode, where the button stays hidden.
- Bounded memory: the page keeps one small entry per photo opened in the run.

## Approaches

1. **A "new image" flag from Rust.** Smallest change, but returning to an
   earlier photo loses its rotation. Rejected.
2. **Rotation remembered per file in the page, keyed by a hash of the path.**
   Chosen: re-opening a photo restores its rotation, and the key is a number, so
   no string escaping can go wrong and odd filenames need no special case.
3. **Never reset the roll.** Fewest lines, but a rotated fisheye photo followed
   by a normal equirectangular photo would open tilted with no button to fix it.
   Rejected.

## Design

- `resources/photosphereviewer/src/viewer.js`
  - `const rollByFile = new Map()`, keyed by the numeric file key, holding the
    roll a photo was last left at.
  - `let currentFileKey` is set by the loader; the rotate button writes its new
    roll into the map.
  - The loader applies `roll: projection ? remembered : 0` and clears nothing,
    always passing `sphereCorrection` so the intent is explicit rather than
    relying on PSV's inheritance.
  - Signature becomes
    `setPanoramaImageFromBase64(base64, mimeType, projection, fileKey)`.
- `src/window.rs`
  - `fn file_key(path: Option<&Path>) -> u64` hashes the path with
    `DefaultHasher`; the demo image has no path and gets `0`.
  - `render_panorama_with` passes the key from `active_panorama_file` as the
    fourth argument, and the projection as `null` when the file is flat, so the
    call always has four arguments.
- `CHANGELOG`: one line under a new unreleased heading.

## What is left to the builder

- Naming and comment wording, in the repo's existing why-focused style.
- No new tests (decided with the partner): the page logic is exercised by hand,
  the snippet stays inline in `render_panorama_with` since nothing would test an
  extracted builder.
