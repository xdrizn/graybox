---
name: tdd
description: Use when adding a Graybox feature or fixing a bug test-first, or when Adriel asks for tests; also before writing an engine command, parser or layout rule.
---

# Test-driven development (Graybox)

Red, then green, one slice at a time.

- **Test behaviour through the public seam**: an engine command (`s.execute("shape.rectangle", &json!({...}))`
  on a `Session`) and the document it leaves, or a pure function in `crates/game`. Not private helpers.
  Name tests as specs: `tests::scaled_instance_keeps_its_size_override`.
- **One slice per cycle**: one failing test → minimum code to pass → next test. Never write all
  tests first.
- **Expected values come from outside the code** (a literal, a worked example, the spec), never
  recomputed with the same formula.
- **Seams**: decide them yourself and state them in one line (Adriel is not a developer). Engine
  commands: `crates/engine/src/*tests.rs`. Pure game logic: tests next to the module in `crates/game`.
  UI: see `graybox-egui-tests`. Real look: `graybox-app-check`.
- **Run narrow, then wide**: `cargo test -p figcraft-engine <name>`, then `cargo test --workspace --no-fail-fast`
  before commit (known failures: see `graybox-egui-tests`).
- New commands also extend the junk-parameter sweep (`graybox-hooks`). Tests may `unwrap()`; shipped code may not.
- Refactor after green, in a separate step, without changing test expectations.

---
Adapted from `tdd` in mattpocock/skills (MIT, © 2026 Matt Pocock). See `../THIRD-PARTY.md`.
