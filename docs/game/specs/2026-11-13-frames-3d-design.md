# G17 — 3D rotation for frames (design)

Owner's request (2026-11-13, in Portuguese): turn a frame in 3D, e.g. a health bar turned on its own
axis so its left side is nearer than its right; ideally it stays editable. Game UI use: diegetic,
tilted HUD panels.

## Decision (made while the owner was away)

**A frame keeps its real layers and gets Rotate X, Rotate Y and Perspective. It is drawn flat, then
warped by a projective transform (homography) in the one renderer every view uses. While a layer
inside it is selected, it shows flat ("edit flat") and tilts back when the selection leaves.**

Why: the canvas, the Prototype, Play and PNG/JPG export all draw through `figcraft-render`
(tiny-skia, CPU). One warp step there covers every view; no egui mesh, no second drawing path, no
duplicated "child component". Making every editing tool (handles, snapping, drag deltas, text caret)
projective would touch hundreds of upstream call sites; showing the frame flat while you edit inside
it keeps every tool exactly as it is and the layers are still the same layers.

## What the owner sees

- Design panel, for one selected frame, component, instance or group-less frame-like layer: a
  **3D** section (under 9-slice) with a `+` to turn it on. Fields: **X**, **Y** (degrees, −80…80)
  and **Perspective** (distance in px, default 1000; smaller = stronger). `−` turns it off.
  The existing rotation field is the Z rotation.
- Canvas: the frame is drawn in perspective. Selected, its outline is the tilted shape.
- Click on what you see: the click goes through the tilt (inverse homography), so you hit the layer
  you see under the pointer. Once a layer inside is selected the frame **shows flat** so the handles,
  text editing and dragging line up; select something outside (or press Esc up to the frame) and it
  tilts back.
- Prototype and Play show the tilt. **Smart animate** between two tilt values (variants, navigate)
  animates X, Y and Perspective.
- PNG/JPG export shows the tilt. Unity export writes `rotation3d` and the Live Link sets the
  RectTransform's X/Y rotation.

## Conventions (match Unity's numbers)

- Angles in degrees, clamped to ±80. **+X: top goes away, bottom comes near. +Y: right comes near,
  left goes away** (Unity's `Quaternion.Euler`, left-handed, Y up, camera looking +Z). The owner's
  example, "left nearer than right", is a **negative Y**.
- Order like Unity's Euler (Z, then X, then Y): the layer's own 2D transform (with its Z rotation)
  first, then the tilt about the layer's centre in its parent's space.
- Perspective: the viewer's distance to the layer's plane, in page units (CSS `perspective`).
  Clamped to 50…100 000. It is raised as needed so no corner of the layer's box comes closer
  than a fifth of it (no point behind the eye, no blow-up at ±80°).
- Projection, with centred coordinates (x, y) (y down) and depth toward the viewer
  `z = x·sin Y + y·sin X·cos Y`:
  `X' = d(x·cos Y − y·sin X·sin Y)/(d − z)`, `Y' = d(y·cos X)/(d − z)`, a 3×3 homography.

## Storage

Plugin data key `graybox.rotation3d` = `"x,y,perspective"` on the layer (like 9-slice), so copy,
paste, duplicate, undo, instances' overrides and old builds all keep or ignore it safely. Absent or
all-zero angles = flat.

## Code

| Piece | Where |
| --- | --- |
| Homography math (pure, property-tested) | new crate `crates/tilt` (`figcraft-tilt`, layer 0) |
| Reading the tilt of a node, hit-test point mapping, "edit flat" view | `crates/doc/src/tilt.rs` (ours) |
| Warp in the renderer, flat-content cache, bounds growth | `crates/render/src/tilt.rs` (ours) |
| Command `node.set_rotation_3d {ids, x?, y?, perspective?, enabled?}` | `crates/engine/src/cmd/tilt.rs` (ours) |
| Design panel section, canvas outline, smart animate lerp | `crates/ui-egui/src/game/tilt_ui.rs` (ours) |
| Unity layout field | `crates/engine/src/unity_export.rs` (ours), Live Link C# |

Upstream hooks (one line each, listed in STATUS.md): `draw.rs` `draw_layer` (warp),
`bounds_within` (tilted bounds), `paint_bounds` (a change inside a tilted frame repaints the frame);
`geometry.rs` `hit_rec` and `layers_at` (point mapped through the tilt); engine `Session::hit`
(edit-flat scope); canvas `render.rs` `request_render` (edit-flat view); `right.rs` (section);
`canvas.rs` (outline); `present/transition.rs` `smart_interpolate` (lerp); `doc/lib.rs`,
`render/lib.rs`, engine `cmd` registry (`mod` lines); workspace `Cargo.toml` + `xtask/layers.rs`.

### Rendering

`render::tilt::draw` runs first in the `draw_layer` hook. For a tilted layer it renders the layer
flat (its tilt removed, same children) into an offscreen pixmap at the device scale times the
nearest side's magnification (max 4096 a side, within the layer budget), then for each device pixel
of the projected quad maps back through the inverse homography and samples bilinearly
(premultiplied). Points that would be behind the eye are clipped (polygon clip against the
`d − z ≥ d/5` plane). The flat pixmap is cached (8 entries, 64 MB) keyed on the `Arc`s of the
layer's whole subtree, the scale and the render flags: a tilted frame re-renders only when its
content changes; moving it or panning only re-warps.

Limits (written down, not hidden): background blur / glass inside a tilted frame sees only what is
inside the frame; a blend mode on the tilted layer itself composites as normal; SVG and PDF export
draw it flat (they are vector formats; the tilt is a raster warp).

### Unity

`layout.json` gets `"rotation3d": {"x": .., "y": .., "perspective": ..}` on a tilted layer; its
sprite is rendered flat (Unity applies the rotation). The Live Link applies
`localEulerAngles = (x, y, 0)` about the rect's centre. Perspective needs a Screen Space – Camera
(or World Space) canvas; with Screen Space – Overlay Unity shows the rotation without perspective.

## Budgets

- Warp cost: one inverse map + bilinear sample per covered device pixel (~5 ms for a 1000×300
  frame at 2×). Flat render only on content change (cache).
- No allocation per frame when nothing is tilted (one plugin-data lookup per layer drawn).

## Phases

- G17.1 Math crate + doc model + command (junk sweep) + renderer warp + bounds + hit-test + edit
  flat + Design panel section. Checked in the app (health bar, left side near, edit fill and text,
  PNG export).
  Smart animate lerp, Unity export field + Live Link landed in G17.1 too.
- G17.2 (2026-11-13):
  - **Selection shows the tilted shape.** `doc::tilt::page_warp(doc, id, flat)` is the one page-space
    homography a layer's flat outline goes through to where it is drawn (its own tilt and every
    tilted ancestor's, inner first, those shown flat left out). The canvas outlines (selection and
    hover, `canvas/outline.rs`) and the transform box (`overlays.rs` `selection_geometry`) go
    through it, so the box and its corner handles sit on the tilted corners; the flat box is gone.
  - **Handles through the tilt.** A pointer event on the one selected tilted layer whose point,
    taken back through the warp, lands on a handle of its flat box is sent to the tools taken back
    (`canvas.rs` `pointer_event` → `tilt_ui::through`), and so is the rest of that gesture, through
    the warp it started with. Resize and rotate then work in the layer's own flat frame. Points
    that reach no handle go as they are (clicks already hit through the warp).
  - **Tilt handle.** A layer with 3D on shows a round handle 18 px above its drawn top edge. Drag
    it like a trackball: right turns the face right (Y down, the left side comes near), down turns
    it down (X down, the top comes near), 0.5° a pixel, Shift in 5° steps. One undo step
    ("3D rotation").
  - **SVG and PDF** draw a tilted layer through the renderer and embed it as an image
    (`raster_whole` / `raster_node`), over `raster_bounds` grown to what the warp paints.
- Later: Z order by depth between siblings; true 3D sorting.
