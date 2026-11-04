# Game layer — status

**Current milestone:** G7 — Share (and G0.3, G2.3 leftovers)

## Done
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

## Decisions
- State looks without special names (owner, 2026-11-04): hotbars and navigable menus show selected /
  focused / pressed / disabled with a variant the designer picks from the items' own variants
  ("When selected: Property 1 = hover"), or Auto (found by names like hover, selected, focused,
  pressed, disabled), or an orange outline. "Create a variant for it" adds one. Warnings say when
  an item is the original component or not a component copy. Older State=Focused files work on Auto.
- Hotbars (owner, 2026-11-04): an auto layout row marked as a hotbar keeps one selected slot in Play:
  keys 1-9 pick, the wheel (unless a map is open) and LB/RB step, without stopping the player. The
  selected slot shows its State=Focused variant.
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
- `crates/ui-egui/src/bidi.rs` `single_line`: control height capped to the row, so rename fields edit
  on the same line as the name (theme interact size 24 > row 20).
- `crates/ui-egui/src/panels/proto_panel.rs` `prototype`: calls `game::menu_section` for one
  selected layer (after Scroll behavior).
- `crates/ui-egui/src/present/player.rs` `player`: calls `game::menu_overlay` right after `draw`.
- `crates/doc/src/game.rs` and `crates/engine/src/cmd/game.rs` are ours (game settings, game commands).
- The G1–G6 commit 32b0bc22 changed more upstream files (present/*, toolbar, theme, services,
  prefs, proto model…) without listing them here; to list.

## Known issues (not ours)
- This machine now builds with `../craft-fonts` (plus the Graybox faces, see FONTS.md); the same three
  engine tests still fail (board tables row growth, .fig font names, example .fig export).
- Fail on this Windows machine before the G4 rework too: `fig_export_assets_tests::every_example_keeps_its_variables_and_styles`
  (.fig export drops the starter kit's number→text binding) and `fig_font_names_tests`
  (depends on installed fonts). `cargo fmt --check` flags `apps/figcraft-web/src/game_input.rs`
  and `web.rs` (from 32b0bc22).
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
