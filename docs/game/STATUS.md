# Game layer — status

**Current milestone:** G0 — Setup

## Done
- G0.1 (2026-11-03): `crates/game` (`figcraft-game`, pure simulation, L2) with the game's fixed
  60 Hz clock (`Game::advance`, `FixedClock`); `crates/ui-egui/src/game/` with `Play`, which
  drives the game from egui's frame time. Not shown in the app yet (Play mode is G1).
- Branding (2026-11-03): the app shows "Graybox by INDIRECT®" (window title, Windows file
  properties, Home sidebar wordmark, Help ▸ About Graybox). About credits FigCraft and its licence.
  Code in `crates/ui-egui/src/game/brand.rs`.

## Decisions
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

## Known issues (not ours)
- `figcraft-engine` test `board_tables_tests::a_double_click_types_in_the_cell_and_its_row_grows_with_the_text`
  fails on this machine at upstream commit `db3cb317` too, before any game change (checked on a
  clean worktree). Probably text measurement without `../craft-fonts`. Everything else in
  `cargo xtask ci` passes. `collab_live_tests::edits_sync_and_undo_takes_back_only_your_own`
  failed once and passed on the rerun (flaky).
- Dev machine (Arch): the web target comes from `pacman -S rust-wasm` (no rustup).

## Next
- G0.3: auto-save on by default; checkpoint and recovery copy every 2 minutes (also for files not
  saved yet).
- G1: Play mode and the FPS test scene.
