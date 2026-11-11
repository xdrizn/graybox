# Game layer — status

**Current milestone:** G7 — Share (and G0.3, G2.3 leftovers)

## Done
- G9 (2026-11-11): an opaque inside stroke the colour of what lies behind a shape (a dark ring on a dark
  frame) no longer shows a thin light line along its outer edge: the fill stops a pixel inside the
  stroke instead of on the same curve (rectangles, rounded rectangles, ellipses; `render/covered.rs`
  `fill_under_stroke`, tests in `render/stroke_edge_tests.rs`). Centre and outside strokes never had it.
  Dashed, per-side, translucent, blended, gradient strokes and other shapes draw as before.
- G0.1 (2026-11-03): `crates/game` (`figcraft-game`, pure simulation, L2) with the game's fixed
  60 Hz clock (`Game::advance`, `FixedClock`); `crates/ui-egui/src/game/` with `Play`, which
  drives the game from egui's frame time. Not shown in the app yet (Play mode is G1).
- Branding (2026-11-03): the app shows "Graybox by INDIRECT®" (window title and Windows file
  properties; "Graybox" in the Home wordmark and Help ▸ About Graybox). About credits FigCraft.
  Code in `crates/ui-egui/src/game/brand.rs`.
- Audit (2026-11-03): the big "feat: add graybox" commit (32b0bc22, another agent) brought most of
  G1–G6; checked in the app (Play scene, HUD bound to variables) and ticked in PLAN.md.
- G4 rework (2026-11-03): no screen roles. Any frame is a game screen; opening, pausing and
  freeing the cursor are ordinary prototype interactions. Pause game / cursor actions run with an
  overlay are undone when it closes (`game::Play::settle`). Game settings (controls, events,
  sounds, share, example screens) moved from a floating window to the Prototype tab with nothing
  selected. The starter kit's HUD, Inventory, Pause, Map and Death are examples built from plain
  interactions (`cmd::game::example_interactions`); `examples/game-starter.figcraft` regenerated.
- G5 (2026-11-03): sound checked end to end on desktop (import, Play sound from a key trigger in
  Play, no errors). Added the player's own Volume slider in Play, bottom right while the mouse is
  free; it scales every sound on top of the file's master, group and sound volumes and is kept
  for the app run. `cargo xtask wasm` passes.
- G6 (2026-11-03): components carry their behaviour. Instances already played their main's
  interactions (sounds, game events); "Play animation" now aims at the instance's own copy of
  the layer (`cmd::game::aim_animations_at_instances`, via each layer's `source`), so a button
  that pulses its glow pulses the hovered button, not the main component.

- G4 navigable menus (2026-11-03): any auto layout frame (list, row, grid) can be switched on as
  "Navigable menu" in the Prototype tab (`game.menu.set`, stored in `GameSettings::menus`). In
  Play, focus moves with arrows/WASD/D-pad/left stick and the mouse, holding repeats (0.4 s, then
  every 0.12 s), Confirm (Enter/Space/A) runs the item's On click (or On press), Back (Escape/B) closes the
  overlay. A wrapping vertical layout navigates like a grid (↑↓ in a column, ←→ across).
  Items that are instances of a set with a `State` property show `Focused`/`Pressed`;
  others get an orange outline. Navigation math in `figcraft_game::menu`, focus in
  `ui-egui/src/game/menu.rs`, applied by `present/game.rs`. Starter kit: Button, Tab and Grid
  cell are State sets; the Inventory grid, Pause buttons and library List/Grid are menus. Checked
  in the app (Pause list with ↓, Inventory grid with the D-pad). Spec and plan in `docs/game/specs/`.
  Fix (2026-11-04): the mouse finds menu items through layers drawn over the menu (a monitor
  bezel image covered the whole screen and hid the items from the pointer).
- G9 CRT effect (2026-11-06): new effect type `crt` (`figcraft_doc::Crt`, our `doc/src/crt.rs`),
  a backdrop effect drawn like glass by our `render/src/crt.rs`: bulge, red/blue split, glow,
  box-filtered scanlines with bleed, phosphor columns, vignette, brightness, all in the layer's own
  coordinates. A render with a visible CRT in view on its page is drawn in one piece (no bands),
  canvas patches redraw CRT layers whole, and its backdrop reads clamp at a cut view's edge; the CRT clears its shape on the surface below as far as its picture
  will not cover it, so in Play (transparent frame) the unbent layers never show. In Play it bends the UI layers only, not the 3D scene.
  Figma and Vector Drawable exports leave it out with a warning. Checked in the app: canvas,
  zoomed to 30 % and 300 %, Play. Spec and plan in `docs/game/specs/2026-11-06-crt-effect-*`.
- Play never waits for a heavy screen (2026-11-06): a full-screen CRT took ~460 ms a redraw and
  froze the game on every HUD change (a shot with ammo, a hotbar slot). The CRT now shades rows
  in parallel, blurs its glow at half/quarter size and reads one channel per split (~140 ms), and
  Play redraws a changed screen off the UI thread (`game/frames.rs`): the last image shows until
  the new one is ready, newest request wins. The browser draws at once, as before.
- G9 Unity Live Link run in a real Unity (2022.3.39f1, Personal licence, 2026-11-10): a synthetic
  screen (9-sliced panel with stroke and radius, gradient shape, texts left/centre/right/middle,
  constraints left/right/centre/stretch/scale, three levels of nested frames, one translucent
  layer) exported by `figcraft-cli run`, imported in batch mode with the new
  `GrayboxLiveLink.ImportBatch` (`-executeMethod`, `-grayboxFolder`, optional `-grayboxScene`), and
  the Canvas rendered to PNG through a camera: it matches Graybox's render (only the font differs:
  Unity has no Inter) both at the design size and with the screen made larger (anchors act as
  Graybox's constraints). Both text paths (TextMeshPro, legacy Text) import clean; a second export
  after edits (move, recolour, text, rename, delete, add, fill removed) updates in place with no
  duplicates. Found and fixed: Watch never fired (Mono's `FileSystemWatcher` filter `layout.json`
  does not match files in sub-folders: now watches all files and picks `layout.json` by name);
  opacity was applied twice (sprite and Image colour: now the sprite is exported opaque and the
  layout's opacity becomes a `CanvasGroup`, which the layers inside inherit); auto-width texts
  wrapped in Unity's wider font (the export now carries `text.autoResize`; auto width stays on one
  line); a layer that lost its fill kept its old Image; the package had no `.meta` files (a
  `GrayboxNode` in a scene is referenced by its script GUID, so those are committed). Not tried:
  the window's buttons by hand (the editor opens the window and its Watch runs, headless).
- G9 Live Link prepared for the Unity 6 (6000.3) project of a collaborator, and for pixel art: a
  "Pixel art (no smoothing)" switch in the window (`GrayboxLiveLink.PixelArt`: Point filter, no
  compression); `FindFirstObjectByType` under `UNITY_2023_1_OR_NEWER`. Compiled against 2022.3.39f1
  (TextMeshPro, legacy Text, and the 2023+ branch); NOT run in Unity 6 (not installed here), and
  `enableWordWrapping` may warn there (see the package README).

## Decisions
- Play redraws its whole screen when a variant changes on hover. Costly unchanged layers (layer blur,
  big image fills) are kept between renders instead (owner's "essa aqui" screen: ~450 to ~120 ms per
  hover, identical pixels). Going further would mean redrawing only the changed area.
- Fonts (owner, 2026-11-04): Google Fonts / Noto families download to this computer only when picked
  (font picker's "Google Fonts" rows), never into the repo or the program. Sharing (`game.share`)
  puts the font store fonts the text uses into the file (`game.fonts`, base64, capped); whoever
  opens it loads them (`game::fonts::load_shared`, desktop and browser). System fonts never travel.
- State looks without special names (owner, 2026-11-04): hotbars and navigable menus show selected /
  focused / pressed / disabled with a variant the designer picks from the items' own variants
  ("When selected: Property 1 = hover"), or Auto (found by names like hover, selected, focused,
  pressed, disabled), or an orange outline. "Create a variant for it" adds one. Warnings say when
  an item is the original component or not a component copy. Older State=Focused files work on Auto.
- Hotbars (owner, 2026-11-04): an auto layout row marked as a hotbar keeps one selected slot in Play:
  keys 1-9 pick, the wheel (unless a map is open) and LB/RB step, without stopping the player. The
  selected slot shows its State=Focused variant.
- Wireframe hotbars (owner, 2026-11-06, experimental): when no slot is a component copy and the
  first slot is drawn differently from the second, the first slot is the selected look and the
  second the plain one. Picking slot 4 gives slot 4 the first one's look and the first the
  second's, at once, no animation: size, scale and rotation, fills, strokes, effects, corners and
  opacity move; what each slot holds and its place stay (auto layout reflows). Play only, the file
  is untouched. Identical slots keep the orange outline (`game::hotbar::base_looks`).
- Any monitor (owner, 2026-11-06): in Play the screen frame as designed is the reference size. It is
  scaled until its width or height meets the monitor's (nothing cut, no bars) and stretched along
  the other side to the monitor's shape; the layers' Constraints (Design tab) anchor them: Right +
  Top keeps a minimap in that corner, Center the crosshair in the middle, Left & Right stretches.
  A 16:9 design on a 16:9 monitor looks as before; 4K is the same layout twice as big. Overlays keep
  their designed size. The presenter's Fit / Fill options don't apply in Play (`game::screen_fit`).
- Named inputs (owner, 2026-11-04): the designer names keys/buttons (Prototype, nothing selected →
  Game inputs); a press fires the Game event of that name on the screen on top only, so one key
  can open a screen and close it from that screen. Every file starts with `pegar-item` on E /
  D-pad right. Keys can't clash with gameplay actions or other inputs.
- Maps (owner, 2026-11-04): any frame can be a map (Prototype tab). In Play, while the mouse is
  free, its content pans (mouse drag, left stick) and zooms (wheel; right stick or R2/R1 as chosen),
  between the chosen limits. The view is Play state only.
- Game behind a screen (owner, 2026-11-04): per top-level frame, Game running (default) / Game
  paused / No game, decided by the screen on top (overlay, else the frame). Paused and No game
  freeze the simulation and free the mouse. Game bars: a layer follows vida, municao or reserva as
  a share of its maximum (health max, magazine, starting reserve) and keeps its drawn width as
  the full mark; the binding of a variable to W stays as upstream has it.
- Component panel like Figma (owner, 2026-11-04): a set lists each property on one compact row
  that opens its editor; a variant shows only "Current variant" (its own values, changing one
  renames the variant). Not yet in the compact panel: drag to reorder properties, instance-swap
  default and preferred components (upstream's matrix had them). Upstream's panel tests run with
  the compact panel off (`game::components::COMPACT`).
- Navigable menus (2026-11-03): menu keys live in `menu_bindings`, separate from gameplay keys
  (WASD may move the player and the menu); a menu owns input only while the open screen has one,
  and the player stops moving meanwhile. A key/controller interaction on the open screen for the
  same input wins over the menu (Escape → Close overlay closes one overlay, not two). The first
  menu in layer order owns focus (tab bars switching menus are phase 2). A focused Disabled item
  keeps its look and gets the outline.
- Game screens are free-form (owner, 2026-11-03): no predefined screen types. The kit's screens
  only show what interactions can do. Files with old `screens` or inventory/pause/map bindings
  still open; those are dropped on load.
- Escape in Play frees the mouse only when no screen is open and the frame has no Escape
  interaction of its own (so a pause menu on Escape works). F10 always leaves Play.
- G9 (2026-11-06): a click on a top-level frame's background selects the frame (its hover outline
  already suggested it); a drag there still draws a marquee. Selection colors folds past three
  colours. Auto layout glides last 0.18 s (cubic ease in and out) and only on the canvas. A frame
  fill hidden by a covering layer isn't drawn, which removes the light line around screenshot
  frames. "Padding 0 stays 10" not reproduced (frame, instance and main all apply 0): tests pin it;
  waiting for the owner's steps.
- G9 anchoring (2026-11-07): a layer in a frame without auto layout takes the constraints of the
  third (each way) holding most of it; two thirds or more, or first third into last, stretches. Only
  when its middle is inside the frame (scroll content keeps its own), not inside instances, and not
  when the frame itself was resized (its layers move by their constraints then). Set by hand →
  `GameSettings::anchored` (not plugin data: `.fig` export warns about plugin data). Older layers
  change only when moved or with "Anchor layers by position".
- G9 corner styles (2026-11-07): `Corners::style` (`figcraft_doc::CornerStyle`, our
  `doc/src/corner_style.rs`): Round (upstream's, the only one smoothing shapes), Chamfer (straight
  cut, CSS `bevel`), Scoop (concave quarter circle around the corner, CSS `scoop`), Notch (square
  step, CSS `notch`), V-notch (a V opening half the size along each side, its tip at the full size
  on the diagonal). Every style fits the same size × size square, clamped like round corners, so
  the radius field grows and shrinks it. Round is not written to files (older files load as they
  were). Strokes offset each cut parallel to itself (mitred); other joins take the generic stroke
  outline. `node.set` `cornerStyle` (aliases bevel, inverse round, inset, inverted chamfer; null =
  round). The picker shows only when the selection has a radius or a style. `.fig` export warns
  (Figma draws round corners); code export (CSS, SwiftUI…) still writes plain border radius.
- Branding stops at what the user sees. Internal names stay upstream's: crates, `figcraft.exe`,
  the `.figcraft` extension, the settings folder (`FigCraft`, so settings are kept) and command ids.
  Messages that mention FigCraft in passing (errors, sharing) still do; renaming them would mean
  editing every language catalog.
- The game advances in fixed 1/60 s steps, at most 5 per frame; a late frame (hidden window,
  stall) drops the backlog instead of fast-forwarding. Same game on 30, 60 and 144 Hz displays.
- `crates/game` has no dependencies yet; `crates/ui-egui/src/game/` is where wgpu, input and
  audio will come in (it is the only place that may know about egui).

## Upstream hooks
Every place where we changed an upstream FigCraft file, so merges stay easy:
- `crates/ui-egui/src/present/layout.rs` `layout`: in Play the frame is sized and scaled by
  `game::screen_fit::fit` (anchoring to any monitor).
- `Cargo.toml` `[workspace.dependencies]`: `figcraft-game` path entry.
- `crates/ui-egui/Cargo.toml`: depends on `figcraft-game`.
- `crates/ui-egui/src/lib.rs`: `pub mod game;`.
- `xtask/src/layers.rs` `TABLE`: `("game", 2)`.
- `crates/ui-egui/src/theme.rs` `paint_wordmark`: calls `game::brand::paint_wordmark`.
- `crates/ui-egui/src/windows.rs` `title_of`: calls `game::brand::window_title`.
- `crates/ui-egui/src/dialogs.rs`: About window title and `about_body` text (`game::brand`).
- `crates/ui-egui/src/toolbar.rs` Help menu: `game::brand::ABOUT`.
- `apps/figcraft/src/main.rs` and `startup.rs`: window / error box title `game::brand::FULL_NAME`.
- `apps/figcraft/build.rs`: Windows `ProductName`, `FileDescription`, `CompanyName`.
- `crates/ui-egui/src/panels/proto_panel.rs` `prototype`: calls `game::prototype_section` after the
  prototype settings.
- `crates/ui-egui/src/app/commands.rs` `game.settings`: selects nothing and shows the Prototype tab.
- `crates/ui-egui/src/present/chrome.rs` `finish`: sounds scaled by `Play::volume`.
- `crates/engine/src/proto_run.rs` `play_interactions`: calls `cmd::game::aim_animations_at_instances`.
- `crates/ui-egui/src/panels/component_panel.rs` `instance_section`: a set or main component goes to
  `game::components::section` (compact Properties / Current variant) before upstream's matrix.
- `crates/ui-egui/src/canvas/proto_canvas.rs` `draw` / `input`: a connection dropped on another variant
  of the source's set becomes Change to (`game::proto_targets::variant_at`).
- `crates/ui-egui/src/canvas/proto_canvas.rs` `noodles`: with a selection, only the selected screens'
  connections (`game::proto_targets::shows_connection`).
- `crates/ui-egui/src/panels/proto_panel.rs` `prototype`: `game::screen_ui::section` (game behind the screen,
  game bar) after the menu section.
- `crates/ui-egui/src/present/navigate.rs` `display`: maps show their content moved and zoomed
  (`game::map::apply`).
- `crates/engine/src/cmd/arrange.rs` `scale_node`: an instance scaled (itself or inside a scaled frame)
  gets its "size" override, so a variant change keeps its size (upstream only marked layers inside
  instances).
- `crates/ui-egui/src/present/actions.rs` Change to: `cmd::game::keep_instance_size` before the swap.
- `apps/figcraft/src/main.rs`: with nothing to open, start in the most recent file that still exists.
- `crates/ui-egui/src/services.rs` `Services::cursor_screen_pixels` (desktop Windows: `mouse_position`
  crate) and `canvas/input.rs` `import_dropped`: dropped files land under the mouse.
- `crates/engine/src/fonts/download.rs` `catalog_families` (+ wasm stub in `fonts.rs`); `typography/picker.rs`: the
  font store's families not installed listed under "Google Fonts: downloads when picked";
  `typography/fields.rs`: a pick runs `game::fonts::fetch`.
- `crates/engine/src/cmd/game_share.rs` `share`: `cmd::game::embed_shared_fonts` before publishing.
- `crates/ui-egui/src/app.rs` `frame`: `game::fonts::load_shared`.
- `crates/ui-egui/src/theme.rs` `install_fonts`: Inter stays a fallback behind Space Grotesk (shortcut glyphs).
- `crates/render/src/effect_cache.rs` (new, `mod` in `lib.rs`) and `draw.rs` `draw_layer`: a layer blur and a
  plain image layer reuse their pixels while unchanged (same node `Arc`, place, size, scale, colour
  profile, image bytes, GIF frame); drop shadow casters only for layers with drop shadows.
- `crates/engine/src/canvas/tools/draw.rs` `up` (+ `board_click`): a click without a drag on the
  background of a top-level frame selects it (upstream: deselects); `selection_behaviour_tests`
  row changed to match.
- `crates/engine/src/selection/selection_colors.rs` `collect`: colours group by how they read
  (8-bit channels, whole percent), so near-equal greys are one row.
- `crates/ui-egui/src/panels/selection_colors.rs` `section`: shows by `game::color_fold::shown` (one
  colour too, when it comes from more than the one selected layer); header from `game::color_fold` (folds
  past three colours to a preview and "+N").
- `crates/render/src/covered.rs` (new, `mod` in `lib.rs`) and `draw.rs` `draw_content`: a frame fill
  a layer inside covers completely (opaque, axis-aligned, whole box) is not drawn (no light seam).
  Same file: `draw.rs` `inset_outline` (the ring builder of `stroke_area`, pulled out as a function) and the
  fill call in `draw_content` use `covered::fill_under_stroke` (fill inset under an opaque inside stroke).
- `crates/ui-egui/src/canvas/render.rs` `request_render` and `canvas.rs`: the drawn document goes
  through `game::layout_ease::view`; the canvas repaints while layers glide.
- `crates/ui-egui/src/panels/typography/spacing.rs` `spacing_input`: Alt and a drag anywhere on a
  spacing field scrubs it (`game::alt_scrub`). Number fields do it upstream since v0.17 (merged
  2026-11-10: our `widgets.rs` Alt-scrub was dropped for theirs).
- `apps/figcraft/src/main.rs` (`set_startup_update_check(false)`), `crates/ui-egui/src/toolbar.rs` (Help
  menu row removed), `menus.rs` (`help.checkForUpdates` does nothing) and `dialogs/updates_ui.rs`
  (`check` kept, unused): no FigCraft update check or prompt; its menu string is kept in `game/brand.rs`.
- `crates/ui-egui/src/lib.rs` (`font_watch` field) and `app.rs` `frame`: `game::font_watch::tick`
  looks at the font folders in the background when the window comes back to the front and walks them
  again only when a font file came or went (no refit: a walk reports most families as changed).
- `crates/engine/src/boolean_box.rs` (new, `mod` in `lib.rs`) and `derived.rs` `normalize` step 3: a
  boolean group's box is its result's bounds (after `fit_to_children`).
- `crates/ui-egui/src/canvas/handles.rs` `al_zones`: no padding bands (no padding handles on the
  canvas; gap handles stay).
- `crates/engine/src/anchors.rs` (new, `pub(crate) mod` in `lib.rs`), `derived.rs` `normalize` (after step
  3) and `cmd/props.rs` `"constraints"`: layers placed or moved in a frame without auto layout anchor
  by position (9-slice thirds) until set by hand (`GameSettings::anchored`); `game.anchors.auto`.
- `crates/ui-egui/src/panels/right.rs` `position_fields`: `game::anchor_ui` (Figma-like constraints
  with a clickable box; "Anchor layers by position" for a frame); `constraints_row`/`pretty` kept unused.
- Upstream tests changed for position anchoring: `history_behaviour_tests` scene and
  `canvas_fixes_tests::a_selected_layer_shows_its_constraint_guides_in_its_frame` anchor by hand.
- `crates/engine/src/shift_select.rs` (new, `mod` in `lib.rs`) and `canvas/tools/drag.rs` press: Shift-click
  adds the layer at the selected layers' depth (same top-level frame), not the plain click's level.
- `crates/engine/src/rect_to_frame.rs` (new, `mod` in `lib.rs`), `cmd/structure.rs` `wrap` and
  `cmd/component.rs` (passes `wrapOnly`): "Frame selection" on one rectangle makes it the frame.
- `crates/ui-egui/src/menus.rs` canvas context menu: "Set as frame background" (`game.image.toBackground`,
  in `cmd/game.rs`) after "Crop image" for one image layer in a frame.
- `crates/engine/src/canvas/tools/drag.rs` `resize_drag`: Ctrl crops images inside auto layout too
  (not a layer with its own auto layout, nor one with a min/max size in an auto layout parent).
- `crates/ui-egui/src/panels/right/paints.rs` `paints`: a Fill/Stroke linked to one paint style is one row
  (`game::style_fill`: swatch, name, detach), not the style's paints.
- `crates/ui-egui/src/widgets/colorpicker.rs` gradient "Rotate 90°": `game::gradient_turn::quarter` (a
  linear gradient turns about the layer's middle; others about their centre).
- Upstream v0.17 merge (2026-11-10): `theme.rs` dark `text_tertiary` raised to 136 (upstream's 3:1
  contrast test); `shortcut_parity_tests` pins F9 (`game.play`); `canvas_fixes_tests` pencil preview
  uses a light page (Graybox pages start dark); `crates/text/build.rs` keeps Space Grotesk and
  JetBrains Mono in web builds next to upstream's `FIGCRAFT_EMBED_SCRIPTS`/`FIGCRAFT_WEB_FONTS`.
- `crates/engine/src/canvas/vectoredit.rs` `ensure_vector`: a whole ellipse edits with four points
  (`ellipse_points.rs`, new, `mod` in `lib.rs`).
- `crates/ui-egui/src/canvas/overlays.rs` selection overlay: hidden while the Pen draws a path.
- `crates/engine/src/canvas/tools/drag.rs` pen drag: 8 screen px pull out no handle (was 3); press
  filter: a Cmd/Ctrl press always starts a marquee (not the selection-box drag either), leaving out
  the layer it began on; `draw.rs` `board_click` turns a Cmd/Ctrl click into the deep select.
  `selection_cache_tests` moves with Control (snapping off) instead of Cmd.
- `crates/engine/src/cmd/structure.rs` `join`: several layers joined have their meeting ends welded
  (`weld.rs`, new, `mod` in `lib.rs`; 2 units).
- `crates/ui-egui/src/toolbar.rs`: no Game settings button beside Play (`main_width` counts 5 cells).
- `crates/ui-egui/src/panels/layout_panel.rs` `dimensions`: an upright line shows its length as H
  (`game::line_dims`).
- 9-slice (`crates/doc/src/nine_slice.rs`, `pub mod` in doc `lib.rs`; stored as plugin data
  `graybox.nineSlice`): `crates/render/src/draw.rs` `draw_layer` (now `pub(crate)`) calls
  `nine_slice::draw` (new, `mod` in render `lib.rs`); `cmd/props.rs` `resize_node` keeps a sliced
  layer's children put; `game.nineSlice.set` in `cmd/game.rs`; UI: `panels/right.rs` (section under
  Layout), `canvas.rs` (cuts drawn), `canvas/input.rs` (cuts dragged) → `game::nine_slice_ui`.
- Pseudo-localization: `crates/engine/src/pseudo.rs` (new, `pub mod`); `canvas/render.rs` draws
  `game::pseudo_view::view(doc)`; `canvas.rs` banner; switch in `game::prototype_section`.
- Unity export: `crates/engine/src/unity_export.rs` (new), `game.unity.export` in `cmd/game.rs`
  (skipped by the junk sweep in `tests.rs`: it writes files), `GameSettings::{unity_dir,
  unity_frames}`; UI `game::unity_ui` (frame switch via `panels/proto_panel.rs`, settings part, export
  on save from `app.rs`). The Unity side is C# (`integrations/unity/`, a UPM package): the one place
  that isn't Rust, as Unity plugins can't be.
- `crates/ui-egui/src/bidi.rs` `single_line`: control height capped to the row, so rename fields edit
  on the same line as the name (theme interact size 24 > row 20).
- `crates/ui-egui/src/panels/proto_panel.rs` `prototype`: calls `game::menu_section` for one
  selected layer (after Scroll behavior).
- `crates/ui-egui/src/present/player.rs` `player`: calls `game::menu_overlay` right after `draw`.
- `crates/ui-egui/src/present/actions.rs` `texture`: in Play, a changed screen asks
  `game::frames` and shows its last image meanwhile.
- `crates/doc/src/game.rs` and `crates/engine/src/cmd/game.rs` are ours (game settings, game commands).
- CRT effect (`crates/doc/src/crt.rs`, `crates/render/src/crt.rs` are ours):
  - `doc/src/lib.rs`: `pub mod crt; pub use crt::{Crt, CrtDirection};`, `mod crt_tests`.
  - `doc/src/props.rs`: `EffectKind::Crt`, `Effect::TYPES` entry, `crt()`, `of_type`, `label`,
    `backdrop_radius` arm. `doc/src/colors.rs`: no-colour arm.
  - `engine/src/cmd/arrange.rs` (scale: line size), `engine/src/dev/devinspect.rs` (row),
    `engine/src/effects_tests.rs` (type list).
  - `format/src/fig/export/{paint,layer,assets}.rs`: `figma_effects` leaves CRT out.
  - `vectordrawable/src/{walk,warnings}.rs`: `Warn::Crt`.
  - `render/src/lib.rs` (`mod crt`, `band_overlap` none with a CRT, `render_alone` filter),
    `render/src/backdrop.rs` (`has_effects`, `effects` arm), `render/src/draw.rs` (reach arm,
    `crt::clear_under` before the layer's composite), `render/src/effect_cache.rs`,
    `render/src/retained.rs` (`sensitive`).
  - `ui-egui/src/panels/right.rs` (imports), `ui-egui/src/panels/right/effects.rs` (the CRT
    fields arm and `CrtField`).
- Corner styles (`crates/doc/src/corner_style.rs`, `ui-egui/src/game/corner_style_ui.rs` are ours):
  - `doc/src/props.rs` `Corners::style`; `doc/src/lib.rs` (`pub mod corner_style`, `CornerStyle`,
    `mod corner_style_tests`); `doc/src/node.rs` (component set corners use `..Default`);
    `doc/src/geometry.rs` `shape_path` (rect-like outlines through `corner_style::outline`).
  - `render/src/draw.rs` `side_strokes` and `stroke_area` (styled rings), `render/src/glass.rs`
    `RoundedRect::of` (round only), `svg/src/lib.rs` stroke shortcut (round only).
  - `format/src/fig/export/shape.rs` (stroke geometry), `layer.rs` (warning).
  - `engine/src/cmd/props.rs` (`cornerStyle` / `cornerShape`, help, override group "corners"),
    `automation/inspect.rs` (`cornerStyle` when not round), `cmd/edit/clipboard.rs` (copied and
    pasted with the radius), `tests.rs` (junk sweep rows), `lib.rs` (`mod corner_style_tests`).
  - `ui-egui/src/panels/right/paints.rs` `appearance`: `game::corner_style_ui::section` after the
    radius row; Corner smoothing only for round corners. `ui-egui/src/game/mod.rs`: the module.
- The G1–G6 commit 32b0bc22 changed more upstream files (present/*, toolbar, theme, services,
  prefs, proto model…) without listing them here; to list.

## Known issues (not ours)
- With craft-fonts in the build, `components_scale_tests::editing_deep_mains_keeps_every_override_and_matches_a_full_rebuild`
  fails when the whole engine suite runs in parallel and passes alone (also with the scale fix
  reverted); `board_tables_tests::a_double_click…` passes now.
- This machine now builds with `../craft-fonts` (plus the Graybox faces, see FONTS.md); the same three
  engine tests still fail (board tables row growth, .fig font names, example .fig export).
- Fail on this Windows machine before the G4 rework too: `fig_export_assets_tests::every_example_keeps_its_variables_and_styles`
  (.fig export drops the starter kit's number→text binding) and `fig_font_names_tests`
  (depends on installed fonts).
- `figcraft-engine` test `board_tables_tests::a_double_click_types_in_the_cell_and_its_row_grows_with_the_text`
  fails on this machine at upstream commit `db3cb317` too, before any game change (checked on a
  clean worktree). Probably text measurement without `../craft-fonts`. Everything else in
  `cargo xtask ci` passes. `collab_live_tests::edits_sync_and_undo_takes_back_only_your_own`
  failed once and passed on the rerun (flaky).
- Dev machine (Arch): the web target comes from `pacman -S rust-wasm` (no rustup).

## Next
- Navigable menus phase 2: tab bars (LB/RB) switching the menu below; spatial focus for layers
  without auto layout. To check: Tab sent through the control channel did not open the starter
  Inventory in Play (the controller's View did).
- G0.3: checkpoint and recovery copy every 2 minutes.
- G2.3: try controllers on Linux/macOS (gilrs code is in).
- G7: share link opening in Play (code is in, untested).
