---
name: graybox-upstream-merge
description: Use when Adriel asks to pull FigCraft's updates ("atualiza do upstream", "merge do figcraft", new upstream release) into Graybox.
---

# Merge upstream FigCraft

Remote `upstream` = `https://github.com/storytold/figcraft.git`. Merge is its **own commit**, never
mixed with features. Start from a clean tree (`git status`); ask before touching other sessions' files.

1. `git fetch upstream` then `git log --oneline HEAD..upstream/main | head -30` to see the size.
2. `git merge --no-commit upstream/main` (never rebase, never push).
3. Conflicts: open `docs/game/STATUS.md` → "Upstream hooks". For each conflicted file keep **every**
   Graybox hook (`// Graybox:` lines). If upstream now does what the hook did, **take upstream's
   version** and delete our hook and its STATUS row. Conflicts in CRLF files: use the Edit tool and
   re-grep afterwards (`graybox-hooks`). `git diff --name-only --diff-filter=U` lists what is left.
4. Check hooks still exist: `grep -rn "Graybox:" crates apps xtask | wc -l` before/after the merge
   should not drop except for hooks you deliberately removed. Also re-check `Cargo.toml`
   workspace deps, `xtask/src/layers.rs` `TABLE` (`("game", 2)`) and the `pub mod game;` line.
5. Build and lint: `cargo clippy --workspace --all-targets -- -D warnings`, then
   `cargo test --workspace --no-fail-fast` (long; run in background).
6. Fix tests that assume upstream defaults Graybox changed, **in the test, not the product**:
   pages start dark (use a light page in the test, e.g. pencil-preview tests), anchoring to any
   monitor (`present/layout.rs`), F9 = `game.play` in `shortcut_parity_tests`, dark `text_tertiary`
   contrast. Known machine-only failures are listed in `graybox-egui-tests`; do not "fix" those.
7. Run `cargo xtask ci` if time allows (fmt, layers, assets, wasm).
8. Record in `docs/game/STATUS.md`: "Upstream vX merge (date)" with changed hooks and test fixes.
9. `git add` the merge result and commit with `git commit` (default merge message plus
   `G9: merge upstream FigCraft vX.Y` style subject and the Co-Authored-By line; see `graybox-commit`).
   Features wait for a separate commit.
