# Pixel art in Graybox (G16) — design

> **Dropped on 2026-11-13 (G16.8)**: the "Pixel button" starter and pixel fonts (the glyph sheet
> and its `pixel.font.*` commands). The owner never asked for them and does not want them in the
> product. Buttons are still made by hand from 9-sliced pixel art with a Pixel text label (D3, D4),
> and "Pixel text" on ordinary text layers (D5) stays.

Owner's request (2026-11-12): a **pixel art tool inside Graybox**, built for making game UI. It
should be easy, smart and intuitive, and impressive. In detail:
- draw pixel art and make **components** from it;
- **resize** and have the pixels made automatically from the adjacent ones;
- a pixel-art button: draw the borders, turn on **9-slice**, put **text** in the centre, and the
  button grows with the text (auto layout plus 9-slice);
- **custom fonts**;
- the common **Aseprite-style** features in this first version: pencil, brush size, eraser,
  selection, mirror with a chosen axis, a tileset or tile test, and colour replacement;
- custom **export** with a focus on **Unity**, designed so other engines fit later;
- basic **animation** if possible;
- full integration with what Graybox already has (selection colours, styles and variables,
  components, auto layout, 9-slice, the Unity export and Live Link, Claude).

He left the decisions to the orchestrator. "Not too advanced, but the standard features."

## Clean-room and assets
- Aseprite is a reference **for behaviour only**: its public documentation and how the running app
  behaves. Never read, copy or port its source code, icons, palettes, brushes or any of its files.
  The tool's icons are new, drawn in code (`icons.rs` style). Default palettes are our own or
  public-domain (for example PICO-8-like palettes must not be copied unless the licence allows it;
  prefer palettes we design, or CC0 ones with an ATTRIBUTION row).
- `.aseprite` import (stretch goal) may follow only the **published file-format document**, never
  the program's source.
- Same Graybox rules: no panics, wasm keeps building, Rust only, upstream touched only at named
  hooks, every hook listed in STATUS.md.

## The idea in one paragraph
A **pixel layer** is an ordinary Graybox layer that holds a small sprite (for example 32×32 art
pixels) and shows it at a whole-number **pixel scale** (each art pixel is 1, 2, 3, 4… canvas pixels),
always sharp (nearest-neighbour, never blurred). It moves, nests, goes into auto layout and
components, and exports like any layer. Double-click it, or pick **Pixel** in the bottom mode
switcher, and the canvas becomes a pixel editor *in place*: the rest of the design stays visible
around it. That keeps the pixel art inside the UI work instead of in a separate app.

## Decisions taken (orchestrator, 2026-11-12)
| # | Decision |
|---|---|
| D1 | **Pixel mode** in the bottom mode switcher: Draw · **Pixel** · Design · Prototype · Inspect. Its pill shows the pixel tools. Double-clicking a pixel layer enters it; Esc or Done returns to Design. With no pixel layer selected, dragging on the canvas makes a new sprite of that size, snapped to the pixel scale; a small "New sprite" popover sets width × height and scale. |
| D2 | **Pixel layer** = a node that carries a sprite (Graybox-owned data) and draws it with nearest-neighbour sampling at an integer scale. Its position snaps to whole canvas pixels, and its size to whole art pixels. |
| D3 | **Smart resize**: resizing a pixel layer in Design mode never blurs. Without 9-slice it repeats or extends the nearest edge pixels outward (Grow) or crops (Shrink), never scaling the art; Alt+drag scales by whole multiples. With 9-slice on (insets in **art pixels**), corners stay, edges and centre are **tiled** (default) or **stretched by whole pixels**. That gives the button that grows with its text. |
| D4 | **Pixel 9-slice + auto layout + text**: a pixel layer used as a frame's background (or a frame with a pixel fill) gives its 9-slice insets as the auto-layout padding (an option, on by default). A text child in the centre makes the frame hug the text, and the art regrows (D3). |
| D5 | **Text that matches pixel art**: text layers get a "Pixel text" option: no anti-aliasing, glyphs snapped to the art-pixel grid at the parent's pixel scale. Any font the file can use works (installed fonts, fonts inside the file, the font store). |
| D6 | **Palette = Graybox colours**: the palette panel shows the file's colour styles and variables, the colours used in the sprite, and a local palette (add, sort, load and save `.gpl` and `.hex`). **Selection colours** (right panel) lists a pixel layer's colours like any frame's, and changing one replaces it in the pixels (all frames and layers, an option for exact or similar matches). A palette swap can become a **component variant** ("Red team"). |
| D7 | **Storage**: sprites live in the `.figcraft` file as Graybox-owned data: size, layers, frames, cels (RGBA, compressed), tags, palette, slices, with serde defaults. Old files are unchanged. The engine model and operations live in a new pure crate `crates/pixel` (L2, no egui), deterministic and fully unit-tested. |
| D8 | **Everything is a command** (`pixel.create`, `pixel.stroke`, `pixel.fill`, `pixel.select`, `pixel.transform`, `pixel.replace_color`, `pixel.layer.*`, `pixel.frame.*`, `pixel.export`…). One stroke is one undo step. Commands are in the junk sweep. Claude, the CLI and MCP reach them ("draw a 16×16 heart"). |
| D9 | **Export** goes through an exporter trait (`PixelExporter`) with Unity first, plus generic outputs; Godot and others are new impls later. |
| D10 | Delivery: phases G16.1–G16.6, each committed, reviewed with the `code-review` skill (`high`, repeated until clean), checked in the real app with screenshots. The installed app is updated at the end. |

## Tools (Pixel mode pill and shortcuts)
Shortcuts follow what pixel artists expect (B, E, G, I, L, U, M, Q, W, V, X), and are rebindable:
- **Pencil** (B): pixel-perfect line option (removes L-corners), size 1–64, square or circle,
  `[` and `]` to change size.
- **Eraser** (E), and right-click draws with the secondary colour.
- **Bucket** (G): contiguous or global, tolerance.
- **Eyedropper** (I, or Alt held with any tool).
- **Line** (L), **Rectangle** and **Ellipse** (U), outline or filled, Shift for square or circle.
- **Marquee** (M) and **Lasso** (Q, freehand and polygon), **Magic wand** (W), with add, subtract and
  intersect via Shift and Alt. **Move** (V) for the selection or the layer. Cut, copy and paste, also
  to and from other Graybox layers as images.
- **Swap colours** (X), and a **Replace colour** tool (paint over one colour with another).
- **Mirror**: horizontal, vertical or both, with an **axis you drag** on the canvas (shown as a
  line), snapped to pixel or half-pixel.
- Transform: flip horizontally or vertically, rotate 90°, nearest scale by whole multiples, **outline**
  (inside or outside, colour).
- **Pixel grid** and a custom grid (for example 8×8 tiles), with zoom shown as ×N.
- **Tile mode** (tile test): the sprite shows repeated 3×3 around itself and drawing **wraps across
  the edges**, so a seamless tile can be checked while drawing. A tileset preview uses one tile to fill
  an area.
- **Shade** mode: lighten or darken along the palette ramp, as a stretch goal in G16.2.

## Animation (G16.4, basic)
- A **timeline** strip at the bottom of the canvas in Pixel mode: layers × frames, a duration per frame
  (ms), add, duplicate, delete and drag frames.
- **Onion skin** (previous and next, opacity), playback, loop, ping-pong.
- **Tags** (named frame ranges such as `idle` and `hover`) with direction.
- In **Prototype** and **Play**, a pixel layer plays its tag. Interactions can switch tags (hover →
  `hover` tag), which ties into the existing prototype triggers.

## Export (G16.5)
An exporter trait `PixelExporter { fn export(&Sprite, &ExportOptions) -> Vec<OutFile> }` in
`crates/pixel` (or `crates/format`, wherever the layering allows). Options: scale ×N, which frames or
tags, which layers, trim, padding, extrude (edge bleed), and power-of-two size.
- **PNG** (single frame or each frame), and an **animated GIF** if it is cheap.
- **Sprite sheet**: grid or packed, plus **JSON** metadata (frames, durations, tags, slices with 9-slice
  borders, pivot) in a documented Graybox schema, close to common sprite-sheet JSON conventions.
- **Unity**: PNG plus `.meta`-compatible import settings applied by the **Graybox Live Link** package
  (C#, `integrations/unity/`). The Live Link side:
  - imports the sheet as Sprite (Multiple) with Point filter, no compression, the chosen Pixels Per
    Unit, and the 9-slice **borders**;
  - builds an **AnimationClip** per tag;
  - wires pixel buttons in the existing Canvas export as sliced Images.
  The `unity-livelink` agent knows that package. The existing Unity export (`game.unity.export`) picks
  up pixel layers automatically (nearest, borders).
- Designed for later: Godot (`SpriteFrames` and `.import` settings) and GameMaker or generic JSON are
  new exporter impls. Note them in the spec, don't build them now.
- Import: PNG and sprite sheets (by grid) into a pixel layer. `.aseprite` import is the stretch goal
  (published format document only).

## Phases (one session runs them in order; each ends committed and checked)
1. **G16.1 Core and drawing**:
   - `crates/pixel` (model, ops: pencil, line, rect, ellipse, fill, erase, mirror, brush shapes,
     pixel-perfect), registered at L2 in `cargo xtask layers`;
   - the pixel layer node and its sharp rendering at integer scale (canvas and export);
   - save and load;
   - Pixel mode in the switcher and the pill;
   - pencil, eraser, bucket, eyedropper, line, rectangle, ellipse, brush size and shape;
   - mirror with a draggable axis;
   - grid, zoom, undo per stroke, the commands and the junk sweep.
2. **G16.2 Selection, transform and colour**:
   - marquee, lasso, wand, move, clipboard, flip, rotate, outline;
   - the swap and replace colour tools;
   - the palette panel tied to styles and variables;
   - Selection colours on pixel layers, and replace across the sprite;
   - layers inside a sprite (add, hide, opacity, reorder, merge).
3. **G16.3 Smart resize, 9-slice and pixel text**:
   - Grow and Shrink resize;
   - pixel-unit 9-slice (tile or stretch), drawn on the canvas with the existing 9-slice handles;
   - insets as auto-layout padding;
   - Pixel text;
   - tile mode and the tileset preview;
   - components and variants from pixel layers, and palette-swap variants.
4. **G16.4 Animation**: timeline, frames, durations, onion skin, tags, playback, and playing in
   Prototype and Play.
5. **G16.5 Export and import**: the exporter trait, PNG, sheet and JSON, the Unity path with Live Link
   (C#, compile-checked; batchmode test if Unity is available), PNG and sheet import, and the export
   dialog with presets.
6. **G16.6 Polish and extras**:
   - shade mode;
   - MCP tools so Claude can draw and edit sprites;
   - a performance pass (a 256×256 sprite with 8 layers and 24 frames stays smooth);
   - a final review;
   - the user guide in `docs/game/` (pt-BR summary for the owner);
   - updating the installed app.

## Done when (whole milestone)
In the real app, on a scratch file:
1. Make a 24×24 pixel button frame with the pencil and mirror.
2. Turn on 9-slice.
3. Add text: it grows with the text, and the art stays sharp.
4. Make it a component with hover and pressed variants that play a 2-frame tag.
5. Recolour it through Selection colours.
6. Export a Unity sheet with borders and import it with Live Link.
7. Ask Claude in the chat to draw a 16×16 icon.

`cargo xtask ci` passes apart from the known failures, and wasm builds.
