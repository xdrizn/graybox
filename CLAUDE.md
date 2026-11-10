# Game UI prototyper (INDIRECT fork of FigCraft) — instructions for agents

This repository is **a fork of FigCraft** (MIT OR Apache-2.0), owned by Adriel (INDIRECT, a game
UI/UX studio). FigCraft is a clean-room, pure-Rust (egui) reimplementation of the Figma design
workflow. **This fork adds a game layer**: a playable, low-poly test scene that runs under the
prototype in a fullscreen Play mode, so game UI designed here can be played with keyboard, mouse
and controller, with sound and animations, and shown to clients.

## Talking to the owner
- Adriel is a UX/UI designer, not a Rust developer. Reply in **Brazilian Portuguese**, in plain
  words, short. Explain decisions by what he will see or do, not by internals.
- Code, comments, commit messages and docs inside the repo stay in English (upstream convention).

## Start every session here
1. Read [`docs/game/PLAN.md`](docs/game/PLAN.md) (what we are building, in milestones) and
   [`docs/game/STATUS.md`](docs/game/STATUS.md) (what is done, decisions taken, what is next).
   Unless Adriel gives you a task, take the next unchecked item of the current milestone.
2. **Do not work on upstream FigCraft's own backlog** (`ROADMAP.md`, `docs/gaps.md`, the
   `*-parity.md` files). They describe the original project; read them only to understand code.
3. Orient → plan → implement + test → verify (screenshot) → record (`docs/game/STATUS.md`) → commit.

## Staying mergeable with upstream
- We keep pulling FigCraft's updates. Put our work in **our own files**: the `crates/game` crate,
  `crates/ui-egui/src/game/` and `docs/game/`. Touch upstream files only at small, named hook
  points, and list each hook in `docs/game/STATUS.md` ("Upstream hooks").
- Never reformat, rename or reorganise upstream code you don't need to change.

`plan/` is gitignored (local only).

## Non-negotiables

### Clean-room
- Behaviour comes from public documentation and black-box observation of the running incumbent app
  with synthetic documents only. Never read, disassemble or copy anything inside the incumbent's
  application bundle, web bundle or network traffic (names and public listings only).
- Never copy GPL/AGPL code.
- Reference screenshots of the incumbent stay local under the gitignored `plan/figma/screenshots/`
  and are never committed, published or used as assets.

### Assets: no third-party product iconography or images — ever (absolute rule from the project owner)
- **Never add, copy, trace, redraw-from, embed or ship any icon, image, illustration, cursor, logo,
  template, UI kit, preset library, font file or screenshot from Figma, Adobe, Avid, Autodesk or any
  other product, or from any source whose licence doesn't allow it.** This includes "lookalike"
  redraws of their icons. Breaking this rule is the worst thing you can do in this repo.
- Every image, icon, font or other asset must be one of:
  - original work created for FigCraft by a contributor (licensed MIT OR Apache-2.0);
  - OSI open source;
  - public domain / CC0;
  - Creative Commons with redistribution allowed.
- **Every asset file must have a row in [`ATTRIBUTION.md`](ATTRIBUTION.md)** (path, author, source,
  licence, notes). `cargo xtask assets` (part of `cargo xtask ci`) fails on any unlisted file under
  `assets/`, `docs/brand/` or `docs/images/`.
- FigCraft's UI icons are drawn in code (`crates/ui-egui/src/icons.rs`) from simple primitives and
  are original. Keep it that way: new icons are new code, designed from scratch.
- The app icon is the owner's chameleon drawing, vectorised (`assets/app-icon/README.md`); `cargo xtask icon`
  renders the PNGs and `.ico` from `assets/app-icon/figcraft.svg`.
- **Fonts live in [storytold/craft-fonts](https://github.com/storytold/craft-fonts), never in this
  repo.** The app uses it only through the optional `CRAFT_FONTS_DIR` build input
  (`.cargo/config.toml` points it at `../craft-fonts` for local builds; `crates/text/build.rs`
  embeds it). Everything must build, test and run without it (system fonts are scanned at runtime).
- Demo and test content is synthetic or public domain; never personal photos or data.
- **Game assets follow the same rule**: low-poly models, textures, sounds and controller glyphs are
  made in code or come from CC0 / permissive sources (e.g. Kenney), each with a row in
  `ATTRIBUTION.md`. Never official PlayStation/Xbox/Nintendo button art.
- Code and original assets are MIT OR Apache-2.0 (`LICENSE-MIT`, `LICENSE-APACHE`, `NOTICE`). The
  ArtCraft logos in `docs/brand/` are trademarks, not open source, under
  `docs/brand/LICENSE-brand.txt`: never modify them or use them outside FigCraft.
- If the provenance of an asset is unclear, don't add it.

### Never crash
- No `unwrap()`, `expect()`, `panic!`, `unreachable!`, `todo!`, `unimplemented!` in shipped code
  (workspace clippy lints deny them; tests may use them). Return `Result`/`Option`; use `.get()` for
  data-derived indexes; slice strings only at char boundaries; check floats are finite; cap sizes.
- New workspace crates must include `[lints] workspace = true` in `Cargo.toml` to inherit `[workspace.lints]`. Production crate roots must declare the required crate-level lint attributes before modules or other crate items; crate-level `//!` documentation may precede them.
- `figcraft_engine::guard` turns a panicking command into `EngineError::Internal` and the document
  is rolled back; the desktop app also guards each UI frame and writes recovery copies. These are
  safety nets, not substitutes for `Result`.
- `unsafe_code = "forbid"` workspace-wide.
- Every crash fix comes with a regression test. The engine has a junk-parameter sweep over every
  command (`crates/engine/src/tests.rs`): extend fuzz/sweep tests when you add parsers or commands.

### Architecture
- **Everything is a command.** User-visible behaviour = a command in `crates/engine/src/cmd/*`
  (`spec!(id, label, params doc, undoable, run)`) + tests. The menus, shortcuts, Actions palette,
  CLI, control channel and MCP server all dispatch the same command ids. Command ids follow the
  incumbent's menu catalog in `crates/engine/src/catalog.rs` (the parity metric: `cargo xtask parity`).
- **Programmatic calls never open dialogs.** Commands only *request* UI (`Session::request_ui`); the
  control channel drops dialog requests.
- **Layering** is enforced by `cargo xtask layers`: geom/color (L0) → doc (L1) → layout/text/pathops/plugin/collab
  (L2) → render/format (L3) → svg/pdf (L4) → engine (L5) → ui-egui/mcp (L6) → apps. Nothing below
  L6 depends on egui/eframe/rfd/winit. **The UI crate is swappable.**
- **The UI is thin**: panels read engine state and act through `app.run(id, params)` /
  `app.set_props(ids, props)`. Colours come from `theme::Tokens`.
- **Time goes through `chrono`** (workspace dependency). Never hand-roll UTC/calendar conversion (days-to-date algorithms, leap-year math); a crate owns only its output format string. The one exception is range extension: past the years chrono can represent, shift the date by whole 400-year Gregorian cycles (146,097 days, after which the calendar repeats), let chrono compute the month and day, and add the cycles back to the year (splitting seconds into hours, minutes and seconds is plain arithmetic).
- **Rust only** (no handwritten JS/TS). **Never break wasm** (`cargo xtask wasm`): every game
  feature (render, input, audio) must work on desktop and in the browser build.
- **Game layering:** `crates/game` is pure simulation (no egui/eframe/wgpu/winit); register it in
  `cargo xtask layers` at L2. Rendering the scene (wgpu paint callback), input and audio output live
  in `crates/ui-egui/src/game/`.

### Quality gates (before every commit)
`cargo xtask ci` (fmt, clippy -D warnings, tests, layers, assets, wasm). One task per commit with a
milestone id in the message, game milestones prefixed `G` (`G1.2: FPS movement`). Commit after
every landed arc of work. Push only when Adriel asks.

## Running and looking at the app
- `cargo run -p figcraft -- --control <port> [file.figcraft|file.svg|image]`. Pick a free port
  (`python3 -c 'import socket;s=socket.socket();s.bind(("127.0.0.1",0));print(s.getsockname()[1])'`);
  sibling apps use the control channel too.
- Drive it with JSON lines, e.g.
  `{"id":1,"method":"engine.execute","params":{"command":"shape.rectangle","params":{"x":10,"y":10,"width":100,"height":50}}}`
  then `{"id":2,"method":"ui.screenshot","params":{"path":"/tmp/shot.png"}}`. Methods are documented
  in `crates/ui-egui/src/control.rs` and `docs/control-protocol.md`.
- **For UI work, look at the result** (take a screenshot and read the PNG). Compare with the
  incumbent side by side where the owner allows (`plan/figma/`).
- Headless: `figcraft-cli` (`info`, `export`, `run`, `commands`, `mcp`). MCP: `figcraft-cli mcp`
  (see `docs/mcp.md`).
- Parallel agents: separate `CARGO_TARGET_DIR` per agent, edit only the crates/files you own,
  write manifests atomically, delete your target dir when done (disk is limited).

## Roadmap
Our roadmap is `docs/game/PLAN.md`; progress goes in `docs/game/STATUS.md`. Upstream's `ROADMAP.md`
and parity docs are not ours to update.

## CI workflows
Leave `.github/workflows/` as upstream has it unless Adriel asks; run `cargo xtask ci` locally.

## Agents
Unity export and the Live Link package: use the `unity-livelink` agent (`.claude/agents/unity-livelink.md`).

## Project skills
Load these with the Skill tool when the task matches (`.claude/skills/<name>/SKILL.md`):
- `graybox-app-check` — verify UI in the real app over the control channel, on a scratch copy.
- `graybox-update-stable` — update Adriel's installed Graybox copy and smoke-test it.
- `graybox-upstream-merge` — merge FigCraft updates and keep every Graybox hook.
- `graybox-hooks` — how to touch upstream files (hooks, CRLF, i18n, junk-sweep pitfalls).
- `graybox-egui-tests` — egui test pitfalls and the known failing tests.
- `graybox-commit` — one-task commits, PLAN/STATUS updates, stage only your own files.
- `grilling`, `diagnosing-bugs`, `tdd`, `handoff`, `wait-what` — general workflow skills adapted from
  mattpocock/skills (MIT, see `.claude/skills/THIRD-PARTY.md`).
