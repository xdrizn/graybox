---
name: graybox-commit
description: Use when a Graybox task is finished and needs committing, updating PLAN/STATUS, or when Adriel says "commita"; also before staging in a folder other sessions share.
---

# Commit a Graybox task

One task per commit, milestone id first, English message. Never push unless Adriel asks.

1. **Gates**: `cargo xtask ci` (fmt, clippy -D warnings, tests, layers, assets, wasm). For small
   edits at least `cargo fmt --all -- --check`, clippy and the tests you touched. Say what you did
   *not* run. Known failing tests: `graybox-egui-tests`.
2. **Docs**: tick the item in `docs/game/PLAN.md`; add to `docs/game/STATUS.md` what changed,
   decisions taken, and new upstream hooks (`graybox-hooks`).
3. **Stage only your own paths**, by name: `git add <file> <file>`. Never `git add -A` or `.`.
   Check `git status --short` first: files you did not write belong to another session
   (e.g. `docs/game/specs/2026-11-06-stroke-side-paints-design.md`) and stay untouched and unstaged.
   `.claude/agents/` belongs to whoever owns it.
4. **Message**: `G9: <what Adriel gets, imperative or noun phrase>` (milestone id `G<n>` as in
   `PLAN.md`; `G9.2:` style for sub-items). Body optional: why, hooks changed. End with the
   attribution line the session gives, e.g.
   `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>` (use the exact line from the session).
5. Commit with a heredoc (`git commit -m "$(cat <<'EOF' … EOF)"`). If a hook fails, fix it; never `--no-verify`.
6. `git log -1 --stat` to confirm only your files are in. Do not amend others' commits. Do not push.
7. Tell Adriel in Portuguese, short: what he can now see/do, what was not verified.

Upstream merges are their own commit (`graybox-upstream-merge`).
