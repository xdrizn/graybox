# CRT effect — design

Owner request (2026-11-06): a **CRT** effect in the Effects section, with every usual CRT setting
(curvature, line size, line opacity, how the lines affect what is behind…), applied statically.

Owner decisions:
- It is a **backdrop effect**, like Background blur and Glass: the designer draws a layer over the
  screen area (any shape, rounded corners welcome) and every layer behind it, inside its shape,
  gets the CRT look. The layer's own fills draw over the result as usual (a 0% fill shows only the
  effect).
- **Static**: nothing animates (no flicker, no rolling bar). Grain comes from stacking the existing
  Noise effect.
- No variable binding for its fields yet.

## What the designer sees

Effects → "+" → type dropdown lists **CRT** after Glass. Its settings (rows of two fields, like
Glass):

| Field (panel label) | Range, default | Meaning |
|---|---|---|
| Curvature | 0–100 %, 30 % | Barrel bulge of the backdrop; samples past the edge come out black |
| Vignette | 0–100 %, 40 % | Darkens towards the edges |
| Line size | 1–64 (document units), 4 | Period of the scanlines: one line every N units |
| Thickness | 0–100 %, 50 % | Share of each period that is dark |
| Line opacity | 0–100 %, 50 % | How dark the dark part is |
| Bleed | 0–100 %, 50 % | Bright backdrop pixels push through the lines (0: lines darken everything equally) |
| Direction | Horizontal / Vertical, Horizontal | Which way the lines run |
| RGB split | 0–100 %, 20 % | Red and blue sampled apart (chromatic aberration), growing towards the edges |
| Phosphor | 0–100 %, 20 % | Red/green/blue column mask |
| Glow | 0–100 %, 30 % | Soft halo of bright areas |
| Brightness | 50–200 %, 120 % | Overall gain, to win back what the lines take |

Everything scales with the document (zooming in shows bigger lines), so the canvas, Present, Play
and exports look the same.

## Data (doc crate)

- New file `crates/doc/src/crt.rs` (ours): `pub struct Crt { curvature, vignette, line_size,
  thickness, line_opacity, bleed, direction: CrtDirection, rgb_split, phosphor, glow, brightness }`
  with `#[serde(default)]`, camelCase aliases, `Default` (table above) and `sanitized()` (finite,
  clamped to the ranges above; non-finite → default). `CrtDirection { Horizontal, Vertical }`.
- Hook in `props.rs`: `EffectKind::Crt(Crt)` (serialised `"type": "crt"`), entry `("CRT", "crt")`
  in `Effect::TYPES`, `Effect::crt()`, `of_type("crt")`, `label()`, and `backdrop_radius()` =
  the glow radius plus the largest RGB split offset (so the backdrop read is padded enough).

## Rendering (render crate)

- New file `crates/render/src/crt.rs` (ours): `pub fn crt(c: &Crt, frame: EffectFrame<'_>) ->
  Option<Pixmap>`, the same contract as `glass::glass`: read `frame.back` (the backdrop padded by
  `pad`), return a `size` pixmap masked by the layer's shape coverage.
- Per output pixel, in the layer's unit box `uv` (from the inverse of `xf`, so lines follow the
  document, not the screen):
  1. Curvature: `uv' = 0.5 + (uv − 0.5)·(1 + k·r²)` with `r` the distance from the centre and `k`
     from Curvature; outside 0..1 → black.
  2. Sample the backdrop bilinearly at `uv'`; with RGB split, red at `uv'` pushed outwards and blue
     inwards by an offset that grows with `r`.
  3. Glow: add a blurred copy of the backdrop (computed once per render with `blur::apply`),
     weighted by Glow, bright parts only.
  4. Scanlines: position along the line axis in document units modulo Line size; inside the dark
     share (Thickness, with a 1-pixel soft edge) multiply by `1 − opacity·(1 − bleed·luma)`.
  5. Phosphor: multiply by a red/green/blue column triad (column width = Line size / 3, at least
     one device pixel) mixed in by Phosphor.
  6. Vignette and Brightness, clamp, premultiply.
- Hooks: `backdrop.rs` (`has_effects`, the `effects` match arm), `draw.rs` reach match,
  `effect_cache.rs` and `lib.rs` (treat CRT like Glass wherever "backdrop effect" is listed).
- Budget: one pass over the layer's pixels plus one blur, the same order of cost as Glass. Bands
  render in parallel where Glass already does.

## Panel (ui-egui)

`panels/right/effects.rs`: a `EffectKind::Crt` arm with the rows above (`fx_field_in` number
fields, percentages shown as 0–100, a dropdown for Direction), plus the type icon/preview arm next
to Glass. The rest of the panel (add, remove, reorder, hide, copy/paste, styles) works unchanged.

## Other exhaustive matches (one line each, no CRT output)

`doc/colors.rs`, `engine/assets/style_vars.rs`, `engine/cmd/arrange.rs`, `engine/cmd/styles.rs`,
`engine/dev/codegen/css.rs`, `engine/dev/devinspect.rs`, `format/fig/export/paint.rs` (Figma has
no CRT: the effect is left out), `format/rest/props.rs`, `pdf/draw.rs`, `svg/lib.rs`,
`render/retained.rs`, `vectordrawable/walk.rs`. Each is listed in STATUS.md "Upstream hooks".

## Errors and limits

- Fields are sanitised before drawing; junk JSON in `effect.add`/`node.set` either fails to parse
  or lands clamped (the engine junk sweep covers the new type).
- A shape with no area, a zero-size layer or a missing backdrop returns `None` (nothing drawn).
- No `unwrap`; data-derived indexes through `.get()`.

## Testing

- doc: serde round trip, aliases, `sanitized()` with NaN/∞/out-of-range, `of_type("crt")`.
- render: on a backdrop of known stripes, a CRT layer with only lines darkens every Nth row by the
  opacity; Bleed 100 % keeps white rows white; Curvature 100 % turns the corners black and keeps
  the centre; RGB split moves red and blue apart at the edge but not at the centre; a CRT on a
  rounded rectangle leaves pixels outside the shape untouched.
- engine: `effect.add {type: "crt"}` adds it with the defaults; the junk sweep runs.
- panel: adding CRT from the type dropdown and editing Line size changes the stored effect.
- In the app: a rectangle over an image with CRT, screenshot on the canvas and in Play.
