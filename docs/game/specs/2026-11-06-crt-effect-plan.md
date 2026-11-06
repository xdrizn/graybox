# CRT effect Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A static CRT backdrop effect (curvature, vignette, scanlines, bleed, RGB split, phosphor, glow, brightness) selectable in the Effects section.

**Architecture:** New `EffectKind::Crt(Crt)` in the doc crate (data in our own `crates/doc/src/crt.rs`), drawn like Glass through the render crate's backdrop path (our own `crates/render/src/crt.rs`), edited in the Effects panel. Every other exhaustive `EffectKind` match gets a one-line arm that outputs nothing.

**Tech Stack:** Rust, serde, tiny-skia (render), egui (panel).

**Spec:** `docs/game/specs/2026-11-06-crt-effect-design.md`

## Global Constraints

- No `unwrap`/`expect`/`panic!`/`unreachable!` outside tests; data-derived indexes via `.get()`; floats checked finite.
- Must build for wasm (`cargo xtask wasm`) and pass `cargo xtask layers`.
- Field names (serde, camelCase): `curvature, vignette, lineSize, thickness, lineOpacity, bleed, direction ("horizontal"|"vertical"), rgbSplit, phosphor, glow, brightness`; effect `"type": "crt"`; label `"CRT"`.
- Defaults: curvature 0.3, vignette 0.4, line_size 4.0, thickness 0.5, line_opacity 0.5, bleed 0.5, Horizontal, rgb_split 0.2, phosphor 0.2, glow 0.3, brightness 1.2.
- Ranges: 0–1 for the percentages, line_size 1–64, brightness 0.5–2.0.
- Each upstream file touched is listed in `docs/game/STATUS.md` "Upstream hooks" (Task 4).
- Commit messages: `G9.x: …`, ending with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Review Focus

1. Zoomed far out (line period under 2 device pixels): lines must not alias into moiré bands — they fade to an even darkening. Test in Task 2.
2. The canvas renders in bands/tiles: the scanline phase must come from layer-local document coordinates, never from pixel indices, so a cut never shifts lines. Test in Task 2 (two different render origins give the same pixels).
3. Degenerate layers (zero width/height, 1×1, empty backdrop) return nothing without panicking. Test in Task 2.
4. A hidden CRT (`visible: false`) draws nothing and adds no backdrop padding. Test in Task 1 (`backdrop_radius` 0) and Task 2.
5. Junk JSON for a CRT effect (`NaN`, strings, negative line size) is refused or clamped, never fatal. Test in Task 1.

---

### Task 1: CRT data and compile-through hooks

**Files:**
- Create: `crates/doc/src/crt.rs`, `crates/doc/src/crt_tests.rs`
- Modify: `crates/doc/src/lib.rs` (`pub mod crt; pub use crt::{Crt, CrtDirection};`), `crates/doc/src/props.rs` (EffectKind, TYPES → 8 entries, `crt()`, `of_type`, `label`, `backdrop_radius`)
- Modify (one arm each, no output): `doc/colors.rs`, `engine/assets/style_vars.rs`, `engine/cmd/arrange.rs`, `engine/cmd/styles.rs`, `engine/dev/codegen/css.rs`, `engine/dev/devinspect.rs`, `format/fig/export/paint.rs`, `format/rest/props.rs`, `pdf/draw.rs`, `svg/lib.rs`, `render/retained.rs`, `render/draw.rs`, `vectordrawable/walk.rs`, `ui-egui/panels/right/effects.rs` (temporary `EffectKind::Crt(_) => {}`)
- Test: `crates/doc/src/crt_tests.rs`, `crates/engine/src/effects_tests.rs`

**Interfaces:**
- Produces: `pub struct Crt { pub curvature: f64, pub vignette: f64, pub line_size: f64, pub thickness: f64, pub line_opacity: f64, pub bleed: f64, pub direction: CrtDirection, pub rgb_split: f64, pub phosphor: f64, pub glow: f64, pub brightness: f64 }` (`Clone, Debug, PartialEq, Serialize, Deserialize, Default` via impl; `#[serde(default)]`, camelCase aliases); `pub enum CrtDirection { Horizontal, Vertical }` (serde lowercase, `ALL: [Self; 2]`, `label() -> &'static str` = "Horizontal"/"Vertical"); `impl Crt { pub fn sanitized(&self) -> Crt; pub fn glow_radius(&self) -> f64 /* document units: glow * 12.0 */; pub fn max_split(&self) -> f64 /* document units: rgb_split * 6.0 */ }`; `Effect::crt() -> Effect`; `EffectKind::Crt(Crt)`.

- [ ] **Step 1: Write failing tests** in `crt_tests.rs`:
  - `defaults_match_the_spec`: `Crt::default()` equals the Global Constraints defaults.
  - `round_trips_and_reads_camel_case`: `serde_json::from_value::<Effect>(json!({"type":"crt","lineSize":8,"rgbSplit":0.5,"direction":"vertical"}))` → `EffectKind::Crt(c)` with `line_size == 8.0`, `rgb_split == 0.5`, `Vertical`, other fields default; serialise → parse gives the same `Effect`.
  - `sanitized_clamps_junk`: `Crt { curvature: f64::NAN, line_size: -3.0, brightness: 9.0, glow: f64::INFINITY, ..default }.sanitized()` → curvature 0.3 (default), line_size 1.0, brightness 2.0, glow 0.3.
  - `of_type_and_label`: `Effect::of_type("CRT")` and `of_type("crt")` are `Some` with label `"CRT"`; `Effect::TYPES.last() == Some(&("CRT","crt"))`.
  - `hidden_crt_reads_no_backdrop`: `Effect::crt().backdrop_radius() > 0.0`; with `visible = false` it is `0.0`.
  - `junk_json_is_refused_not_fatal`: parsing `{"type":"crt","lineSize":"wide"}` is an `Err`, no panic.
- [ ] **Step 2:** Update `effect_add_takes_every_type` in `crates/engine/src/effects_tests.rs`: expected labels gain `"CRT"` before the final `"Glass"`; the undo check expects 8.
- [ ] **Step 3:** Run `cargo test -p figcraft-doc crt` → FAIL (no `Crt`).
- [ ] **Step 4:** Implement `crt.rs` and the `props.rs` hook; `backdrop_radius` arm: `EffectKind::Crt(c) => { let c = c.sanitized(); c.glow_radius() + c.max_split() / 1.5 }`. Add the no-output arms elsewhere until `cargo check --workspace --all-targets` passes (`render/draw.rs` reach match: treat like Glass, `e.backdrop_radius() * 2.0`).
- [ ] **Step 5:** Run `cargo test -p figcraft-doc crt` and `cargo test -p figcraft-engine effects_tests` → PASS; `cargo test -p figcraft-engine junk` → PASS.
- [ ] **Step 6:** Commit `G9.1: CRT effect data`.

### Task 2: Draw the CRT backdrop

**Files:**
- Create: `crates/render/src/crt.rs` (with `#[cfg(test)] #[path = "crt_tests.rs"] mod tests;`), `crates/render/src/crt_tests.rs`
- Modify: `crates/render/src/lib.rs` (`mod crt;`, and the line-321 backdrop filter also drops `Crt`), `crates/render/src/backdrop.rs` (`has_effects` counts a visible Crt; `effects` arm `EffectKind::Crt(c) => crate::crt::crt(c, frame)?`), `crates/render/src/effect_cache.rs` (Crt counts as backdrop)

**Interfaces:**
- Consumes: `Crt`, `CrtDirection` (Task 1); `backdrop::EffectFrame` (existing: `shape, back, pad, size, xf, scale`).
- Produces: `pub(crate) fn crt(c: &Crt, frame: backdrop::EffectFrame<'_>) -> Option<Pixmap>`.

- [ ] **Step 1: Write failing tests** in `crt_tests.rs`, with a scene helper `crt_scene(c: Crt, backdrop: &str) -> (Document, frame, pane)`: a 240×160 frame filled with the given hex colour (or horizontal 1-unit stripes when `"stripes"`), a fill-less 120×80 rectangle at (60,40) carrying only the CRT; render with `render_node(&d, frame, scale)`. A base `lines_only()` Crt has every field 0 except `line_size 4, thickness 0.5, line_opacity 1.0, bleed 0, brightness 1.0`.
  - `lines_darken_every_period`: grey `808080` backdrop, `lines_only()`: inside the pane, rows whose local y mod 4 ∈ [0.5, 1.5] are ≤ 10, rows with y mod 4 ∈ [2.5, 3.5] are 0x80 ± 3; outside the pane unchanged.
  - `bleed_keeps_white_lines_white`: white backdrop, `lines_only()` with `bleed 1.0` → every pane pixel ≥ 0xF0.
  - `vertical_lines_run_down`: `direction: Vertical` → columns alternate, rows within a column equal.
  - `full_curvature_blackens_corners_keeps_centre`: grey backdrop, only `curvature 1.0, brightness 1.0`: pane corner pixel (61,41) ≤ 10; centre (120,80) == 0x80 ± 3.
  - `rgb_split_parts_colours_at_the_edge_only`: stripes backdrop in white/black vertical bars, `rgb_split 1.0, brightness 1.0`: near the left edge red ≠ blue on a bar boundary; at the centre red == blue ± 3.
  - `rounded_shape_leaves_outside_pixels`: pane with corner radius 30: pixel (61,41) equals the frame render without the pane.
  - `zoomed_out_lines_fade_evenly` (Review Focus 1): `lines_only()` at scale 0.25 → all pane pixels within ±12 of each other.
  - `band_cut_does_not_shift_lines` (Review Focus 2): render the frame with the view's origin offset by 3 pixels and compare the pane's pixels to the plain render shifted by 3 → max diff ≤ 2.
  - `degenerate_layers_draw_nothing` (Review Focus 3): pane resized to 0×80 and 1×1 → render returns without panic and matches the render without the pane.
  - `hidden_crt_changes_nothing` (Review Focus 4): `visible: false` → identical to the render without the effect.
- [ ] **Step 2:** Run `cargo test -p figcraft-render crt` → FAIL.
- [ ] **Step 3:** Implement `crt()` per the spec's "Rendering" steps 1–6. Decisions the tests don't fix:
  - Local position: `inv = xf.inverse()`; for pixel centre `(x+0.5, y+0.5)` the local point `p = inv * pt`; `uv = p / (n.width, n.height)` — pass the layer size in through `frame.shape`'s bounds (`Shape::bounding_box`).
  - Curvature `k = curvature * 0.25`; `r² = (u−0.5)² + (v−0.5)²` scaled so a corner has r² = 0.5.
  - Back sampling: local → layer pixels via `xf`, then `+pad`; bilinear on premultiplied RGBA; outside `back` → transparent black.
  - RGB split offset (document units) `= max_split() * r / 0.7071` along `(uv − 0.5)` normalised; red outwards, blue inwards.
  - Glow: `glow_radius()` blur of a copy of `back` via `crate::blur::apply` (uniform `Blur`), added as `glow * max(0, luma(blurred) − 0.5) * 2`.
  - Scanline anti-aliasing: line coverage = box-filtered over the pixel's footprint in document units (`1/scale`), so a period under 2 device pixels averages to `thickness * opacity` darkening.
  - Vignette: `1 − vignette * smoothstep(0.25, 0.75, r²·2)`.
  - Mask by `spread_alpha(std::slice::from_ref(shape), 0.0, xf, w, h)` like the background blur arm.
- [ ] **Step 4:** Run `cargo test -p figcraft-render crt` → PASS; `cargo test -p figcraft-render` → no new failures.
- [ ] **Step 5:** Commit `G9.2: draw the CRT effect`.

### Task 3: CRT controls in the Effects panel

**Files:**
- Modify: `crates/ui-egui/src/panels/right/effects.rs` (replace the temporary arm; add `Crt, CrtDirection` to the `use super::{…}` list and to the parent module's re-exports where `Glass` is)
- Test: `crates/ui-egui/src/game/tests.rs` (our test file; panel driven through the harness the existing panel tests use)

**Interfaces:**
- Consumes: `Crt`, `CrtDirection`, `Effect::crt()` (Task 1).

- [ ] **Step 1: Write failing test** `crt_effect_fields_edit_the_effect`: select a rectangle, run `effect.add {type:"crt"}`, open its settings, set the "Line size" field to 8 and pick "Vertical" in the direction dropdown → the node's effect is `EffectKind::Crt` with `line_size == 8.0`, `direction == Vertical`.
- [ ] **Step 2:** Run it → FAIL.
- [ ] **Step 3:** Implement the arm in the Glass arm's style (`with`, `pct`, `unit`, `fx_field_in`, `edit`), rows of two at `half` width: [Curvature | Vignette], [Line size | Thickness], [Line opacity | Bleed], [Direction dropdown (like Noise mode) | RGB split], [Phosphor | Glow], [Brightness]. Field keys = the serde names; Brightness shown as % (50–200); Line size clamped 1–64.
- [ ] **Step 4:** Run the test → PASS; `cargo test -p figcraft-ui-egui --lib` → PASS.
- [ ] **Step 5:** Commit `G9.3: CRT controls in the Effects panel`.

### Task 4: Check in the app, record, gates

**Files:**
- Modify: `docs/game/STATUS.md` (Done entry; every file from Tasks 1–3 outside our own files under "Upstream hooks"), `docs/game/PLAN.md` (G9 line "CRT effect" ticked)

- [ ] **Step 1:** Run the app with `--control`, add an image, a rectangle over it with CRT; screenshot on the canvas, zoomed out to 25 %, and in Play; read the PNGs: lines visible, corners curved, no banding when zoomed out.
- [ ] **Step 2:** `cargo xtask ci` (known: fmt on `apps/figcraft-web` web files and 3 upstream test failures — run the remaining steps separately if it stops there).
- [ ] **Step 3:** Commit `G9.4: CRT effect recorded`.
