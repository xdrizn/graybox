---
name: graybox-hooks
description: Use when a Graybox change needs code in an upstream FigCraft file, or when editing any file outside crates/game, crates/ui-egui/src/game/ and docs/game/.
---

# Changing upstream code safely

We keep merging FigCraft (`graybox-upstream-merge`), so upstream files must stay almost untouched.

## Where code goes
1. **Our files first**: `crates/game`, `crates/ui-egui/src/game/`, `docs/game/`, and Graybox-only
   modules in engine/render (e.g. `crates/engine/src/cmd/game*.rs`). Put the logic there.
2. **Upstream file = a hook**: one or two lines that call into our module. Comment it
   `// Graybox: <what and why>` and add a row to `docs/game/STATUS.md` → "Upstream hooks"
   (file, function, what it calls). No reformatting, renaming or reordering of upstream code.
3. A new `mod` in an upstream `lib.rs` is a hook too (and a new crate needs `[lints] workspace = true`).

## Pitfalls that cost time
- **CRLF files**: many upstream files use CRLF. `perl -0pi` and multi-line `sed` silently match
  nothing. Use the **Edit tool**, then prove it landed: `grep -n "Graybox:" <file>`. Keep the file's
  line endings (`file <path>` / `git diff --stat` should show a few lines, not the whole file).
- **`egui::Id::new` is deprecated** (clippy `-D warnings` fails). Use `egui::Id::unique(...)`.
- **Strings in our modules are plain English literals**. Do not pass them through `tr()`: the i18n
  test rejects unknown keys and rejects unused catalog keys. When you remove or replace an upstream
  string we used to translate, keep it alive with `tr_noop("…")` in `crates/ui-egui/src/game/brand.rs`.
- **New file-writing commands** (export, save, anything that creates files/folders) must be added
  to the skip list of `every_command_survives_junk_params` in `crates/engine/src/tests.rs`
  (next to `"game.unity.export"`, with a `// Graybox:` comment). Other commands must not panic on junk params.
- Layering: `crates/game` is pure simulation (no egui/wgpu/winit); `cargo xtask layers` enforces it.
- Never `unwrap`/`expect`/`panic!` in shipped code; wasm must keep building (`cargo xtask wasm`).
- Assets: no third-party icons or images; every asset file needs a row in `ATTRIBUTION.md`.

## Before you finish
`grep -rn "Graybox:" <touched files>`, compare with the STATUS "Upstream hooks" list, then
`cargo clippy --workspace --all-targets -- -D warnings` and the narrow tests for what you changed.
