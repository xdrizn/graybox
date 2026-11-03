# Game layer — status

**Current milestone:** G0 — Setup

## Done
- G0.1 (2026-11-03): `crates/game` (`figcraft-game`, pure simulation, L2) with the game's fixed
  60 Hz clock (`Game::advance`, `FixedClock`); `crates/ui-egui/src/game/` with `Play`, which
  drives the game from egui's frame time. Not shown in the app yet (Play mode is G1).

## Decisions
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
