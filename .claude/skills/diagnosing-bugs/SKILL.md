---
name: diagnosing-bugs
description: Use when Adriel reports something broken, wrong, crashing or slow in Graybox ("bugou", "não funciona", "travou", "quebrou"), or a test fails and the cause is not obvious.
---

# Diagnosing bugs

**The skill is the feedback loop.** Get one command that goes red on *this* bug before you
theorise. Reading code to build a theory first is the failure this prevents.

## 1. Build the loop (pick the first that fits)
1. A failing `cargo test -p <crate> <name>` at the engine/command seam (most bugs reach a command).
2. The running app: use `graybox-app-check` (`engine.execute` + `ui.input` + `ui.screenshot`) to
   reproduce on a scratch copy of the file. Assert on `document.inspect`/`node.get`, not on looks.
3. A saved `.figcraft` / `.svg` that triggers it, opened by `figcraft-cli info|export`.
4. `git bisect run` when it worked before (`git log --oneline` helps pick the range).
Make it fast, deterministic, and sharp: it must assert Adriel's exact symptom, not "no crash".
Cannot build a loop? Say what you tried and ask Adriel for the file or the exact steps.

## 2. Reproduce and shrink
Run it, watch it fail with the symptom Adriel described (not a nearby one). Remove inputs, layers
and steps one at a time until every remaining piece is needed.

## 3. Hypotheses
Write 3–5 falsifiable ones: "if X is the cause, then changing Y makes it vanish". Test one at a time.

## 4. Instrument
Prefer asserts and a debugger-free `dbg!`/`log::debug!` at boundaries. Tag every temporary line
`[DEBUG-xxxx]` so one `grep` removes them all.

## 5. Fix with a regression test
Turn the shrunk repro into a test at the real seam, watch it fail, fix, watch it pass, re-run the
original loop. CLAUDE.md requires a regression test for every crash fix. Crashes: no `unwrap`/`panic`
in shipped code; return `Result`/`Option`.

## 6. Clean up
`grep -rn "DEBUG-"` is empty, scratch files and test app processes are gone, and the commit message
names the cause (`graybox-commit`).

---
Adapted from `diagnosing-bugs` in mattpocock/skills (MIT, © 2026 Matt Pocock). See `../THIRD-PARTY.md`.
