# Game layer — status

**Current milestone:** G6 — Animations in component variants

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

## Decisions
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
- The G1–G6 commit 32b0bc22 changed more upstream files (present/*, toolbar, theme, services,
  prefs, proto model…) without listing them here; to list.

## Known issues (not ours)
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
- G6: interactions inside component variants carry sounds/animations/game events everywhere.
- G0.3: checkpoint and recovery copy every 2 minutes.
- G2.3: try controllers on Linux/macOS (gilrs code is in).
- G7: share link opening in Play (code is in, untested).
