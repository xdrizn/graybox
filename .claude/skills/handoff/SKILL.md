---
name: handoff
description: Use when a session is getting long or ends mid-task and Adriel asks for a handoff ("handoff", "passa pro próximo", "resume pra outra sessão").
argument-hint: "What will the next session be used for?"
disable-model-invocation: true
---

Write a handoff note so a fresh agent can continue.

- Save it under the system temp folder (`$TMPDIR`, else `%TEMP%`), not in the repo. If the work is
  worth keeping in the repo, it belongs in `docs/game/STATUS.md` instead.
- Say: goal, what is done (commit hashes), what is half-done (files, uncommitted state), decisions
  Adriel already took, what is next, and what is broken or unverified.
- Do not copy what is already in `PLAN.md`, `STATUS.md`, specs, commits or diffs: link them by path.
- Suggest which project skills the next agent should load (`graybox-app-check`, `graybox-commit`, …).
- List files other sessions own and must not be touched or staged.
- Redact secrets and personal data.
- If an argument was given, tailor the note to that next task.

---
Adapted from `handoff` in mattpocock/skills (MIT, © 2026 Matt Pocock). See `../THIRD-PARTY.md`.
