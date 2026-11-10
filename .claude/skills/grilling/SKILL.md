---
name: grilling
description: Use when Adriel gives a short or vague request that could mean several different things (a UI behaviour, a prototype flow, a game feature), or says "grill", "me questiona", "vamos alinhar" before building.
---

# Grilling (interview before building)

Adriel's requests are short Portuguese lines. A wrong guess costs a whole build, so settle the
open decisions first. Ask in **Brazilian Portuguese, plain words**, about what he will **see or do**,
never about internals.

1. Look up facts yourself first (code, `docs/game/PLAN.md`, `STATUS.md`, the running app). Never ask
   Adriel something you can read.
2. Map the decisions as a tree. The **frontier** is every decision whose prerequisites are settled.
   Ask the whole frontier in one round, at most ~5 questions.
3. Format each question like this, so "sim" accepts your pick:

```
❓ **P1 - <short title>**: <the question, with the choices if any>
➡️ <your recommended answer>
```

4. Wait for the answers. They unblock the next round. Repeat until the frontier is empty.
5. Do not start building until Adriel confirms the shared understanding. Then write the decisions
   into `docs/game/STATUS.md` (decisions taken) so the next session does not ask again.

Skip this skill when the request is already clear, or is a small tweak you can undo in one step.

---
Adapted from `grilling` in mattpocock/skills (MIT, © 2026 Matt Pocock). See `../THIRD-PARTY.md`.
