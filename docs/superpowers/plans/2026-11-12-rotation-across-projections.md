# Rotation Across Projections Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A quarter-turn rotation given to a dual fisheye photo survives switching the projection to equirectangular and back.

**Architecture:** The page keeps a `Map` from file key to the roll a photo was last left at, and applies it only in dual fisheye mode. The file key is a hash of the photo's path, computed in Rust and passed as the fourth argument of `setPanoramaImageFromBase64`, so a projection switch (same key) is distinguishable from a new file (new key).

**Tech Stack:** Rust (GTK4/libadwaita, `webkit6`), JavaScript (Photo Sphere Viewer 5.15, Vite single-file build).

**Spec:** `docs/superpowers/specs/2026-11-12-rotation-across-projections-design.md`

## Global Constraints

- No persistence: the rotation memory lives in the page only, nothing on disk, nothing in the photo file, nothing across restarts.
- No new tests (decided with the partner). Existing tests must keep passing: `npm test` in `resources/photosphereviewer`, `cargo test` at the repo root.
- Comments explain *why*, in the style of the surrounding code; no comment restates the code.
- The equirectangular view of a rotated photo must be level; the rotate button stays hidden there.

## Review Focus

- A rotated fisheye photo followed by an unrelated equirectangular photo: the second must open level, with no rotate button.
- Two different photos whose rotations differ: each must keep its own across projection switches.
- A projection switch while the demo image is open: no rotation, no button, no error in the console.
- A photo re-opened from the file dialog after being rotated: it comes back as it was left.
- The first load of a session (demo image, no path): the file key is stable and the loader does not throw.

---

### Task 1: The page remembers a photo's rotation

**Files:**
- Modify: `resources/photosphereviewer/src/viewer.js` (the `rotatePanorama` function and `setPanoramaImageFromBase64`)

**Interfaces:**
- Consumes: nothing from other tasks.
- Produces: `setPanoramaImageFromBase64(base64: string, mimeType: string, projection: object | null, fileKey: number)` - Task 2 sends the fourth argument; without it the key is `undefined`, which behaves as a single photo, so this task is safe on its own.

- [ ] **Step 1: Add the per-file memory next to the rotate button**

Above `function rotatePanorama(viewer)`, add:

```js
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
```

- [ ] **Step 2: Record the roll in `rotatePanorama`**

Keep the existing wrap of the roll into `[-PI, PI]`; store the result before applying it:

```js
function rotatePanorama(viewer) {
    const correction = viewer.config.sphereCorrection ?? {};
    // The option's parser keeps roll within [-PI, PI].
    const roll = (((correction.roll ?? 0) + ROTATE_STEP + Math.PI) % (2 * Math.PI)) - Math.PI;
    rollByFile.set(currentFileKey, roll);
    viewer.setOption('sphereCorrection', { ...correction, roll });
}
```

- [ ] **Step 3: Apply the remembered roll in the loader**

Replace the reset block in `setPanoramaImageFromBase64` (the `const correction = ...` / `const options = ...` pair) and the `setPanorama` call with:

```js
        // The roll belongs to the photo, not the projection: a flat view stays
        // level, and the rotation waits for the dual fisheye view. Passing
        // `sphereCorrection` every time says so, instead of relying on the
        // viewer handing the last image's correction to the next one.
        currentFileKey = fileKey;
        const correction = window.SphereViewer.config.sphereCorrection ?? {};
        const roll = projection ? rememberedRoll(fileKey) : 0;

        setRotateButtonVisible(Boolean(projection));
        window.SphereViewer.setPanorama(
            projection ? { path: currentObjectUrl, projection } : currentObjectUrl,
            { sphereCorrection: { ...correction, roll } }
        );
```

Change the function's signature line to
`window.setPanoramaImageFromBase64 = function (base64, mimeType, projection, fileKey) {`.

- [ ] **Step 4: Verify the page still builds and its tests pass**

Run: `cd resources/photosphereviewer && npm test && npm run build`
Expected: the `dual-fisheye` tests pass and Vite writes `dist/index.html` without errors.

- [ ] **Step 5: Commit**

```bash
git add resources/photosphereviewer/src/viewer.js
git commit -m "fix: keep a photo's rotation in the page, per file"
```

---

### Task 2: Rust tells the page which photo it is

**Files:**
- Modify: `src/window.rs` (imports, `file_key` helper, `render_panorama_with`)

**Interfaces:**
- Consumes: `setPanoramaImageFromBase64(base64, mimeType, projection, fileKey)` from Task 1.
- Produces: `fn file_key(path: Option<&Path>) -> u64` - a stable id for a path within a run.

- [ ] **Step 1: Add the key helper**

Widen the path import at the top of `src/window.rs` to `use std::path::{Path, PathBuf};` and add `use std::collections::hash_map::DefaultHasher;` plus `use std::hash::{Hash, Hasher};`.

Place this free function next to `impl Window`, above the `Projection` enum:

```rust
/// A stable id for a photo within this run, so the page can tell a file it has
/// already seen - a projection switch re-sends the same one - from a new one.
/// It keys the rotation the page remembers per photo. The demo image has no
/// path and hashes to whatever `None` hashes to, which is stable too.
fn file_key(path: Option<&Path>) -> u64 {
    let mut hasher = DefaultHasher::new();
    path.hash(&mut hasher);
    hasher.finish()
}
```

- [ ] **Step 2: Send the key as the fourth argument**

In `render_panorama_with`, replace the `argument`/`params`/`js` construction with a call that always has four arguments, so a flat image sends `null` rather than dropping the parameter:

```rust
        // Four arguments even for a flat image: the page keys each photo's
        // rotation by the file key, which it needs whichever way the photo is
        // shown.
        let projection = imp
            .projection()
            .argument(panorama.dual_fisheye.as_deref())
            .unwrap_or_else(|| "null".to_string());
        let key = file_key(imp.active_panorama_file.borrow().as_deref());

        let base64 = base64::engine::general_purpose::STANDARD.encode(&panorama.data);
        let js = format!(
            "window.setPanoramaImageFromBase64('{}', '{}', {}, {});",
            base64, panorama.mime_type, projection, key
        );
```

Update the comment above it ("The third argument is the projection, absent for an equirectangular image") to match the new `null` convention, and add the key to the `debug!` log line.

- [ ] **Step 3: Verify the build and the Rust tests**

Run: `cargo test`
Expected: all tests pass (`the_flat_mode_sends_no_projection`, `a_calibration_is_passed_through`, the trailer/mime/version tests). The build script runs `npm test` and `npm run build`; both must succeed.

- [ ] **Step 4: Commit**

```bash
git add src/window.rs
git commit -m "fix: key the remembered rotation by the photo's path"
```

---

### Task 3: Record the change

**Files:**
- Modify: `CHANGELOG`

- [ ] **Step 1: Add an unreleased heading with one line**

Follow the existing format (newest first, one bullet per user-visible change), above the `## v0.1.6` heading:

```markdown
## Unreleased

- Keep a photo's rotation when switching between the fisheye and equirectangular projections.
```

- [ ] **Step 2: Commit the changelog, spec and plan together**

```bash
git add CHANGELOG docs/superpowers
git commit -m "docs: record rotation memory across projections"
```

---

## After Implementation

By hand, in the app (the plan's only check for the page logic):

1. Open an `.insp` photo, rotate it a quarter turn.
2. Toggle the projection to equirectangular: the raw image is level, the rotate button is gone.
3. Toggle back: the quarter turn is there again.
4. Open a normal equirectangular photo: it is level, and no rotate button appears.
