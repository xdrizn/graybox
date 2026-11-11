# Claude in Graybox: chat panel and icons from sketches

Owner's request (2026-11-10): a button where the old Game settings button sat (bottom toolbar,
beside Play) opens a chat with Claude, who edits the open file by itself. Alongside it, icons: the
owner gives references, draws a rough sketch with the brush, and Claude turns it into a real icon.

Decision (owner): Claude runs through the **Claude Code CLI already installed on the PC**
(`claude`, his own subscription). No API key, nothing billed apart from his plan. Desktop only;
the browser build shows the button and says the chat works in the desktop app.

## Part 1: chat (G10.1)

### What he sees
- A chat button (speech-bubble icon, drawn in code in `icons.rs` style) right after Play in the
  bottom toolbar. It toggles a **Claude panel**: a floating window anchored bottom-right of the
  canvas, resizable, themed by `theme::Tokens`.
- The panel: the conversation (his messages, Claude's replies as plain text with simple Markdown:
  bold, lists, code), a one-line trace of what Claude is doing ("Creating a frame…", from tool
  calls), an input box (Enter sends, Shift+Enter new line), **Send**, **Stop**, **New chat**.
- While Claude works, its avatar and outlines show on the canvas (already built: attached agents,
  see `docs/mcp.md`). Each edit is an ordinary undo step.
- Errors are told in plain words: Claude Code not found (with where to install it), not signed in
  (run `claude` once in a terminal), the process failed (last lines of its output).

### How it works
- `crates/ui-egui/src/game/claude/` (desktop parts behind `#[cfg(not(target_arch = "wasm32"))]`):
  - **Control channel**: the chat needs the app's control channel. If the app was not started with
    `--control`, open one on a free 127.0.0.1 port at the first message (reuse `control.rs`'s
    listener; small hook if it only starts from the command line).
  - **MCP server**: `figcraft-cli mcp --attach <port>`. Find `figcraft-cli(.exe)` next to the
    running exe, else on PATH; the installed copy ships it beside `graybox.exe` (update the
    `graybox-update-stable` skill and the install steps to copy it).
  - **Runner**: spawns `claude -p <message> --output-format stream-json --verbose --mcp-config
    <temp json> --allowedTools "mcp__graybox" --append-system-prompt <Graybox guide>` with a
    scratch working folder (never the repo or his projects folder), and `--resume <session id>`
    for the next messages of the same chat. Reads stdout lines on a thread, sends parsed events
    (text, tool use, result, error) through a channel, `ctx.request_repaint()`. Stop kills only that
    child process. Find `claude` on PATH, then `%USERPROFILE%\.local\bin\claude.exe` (and the
    macOS/Linux equivalents).
  - **Graybox guide** (system prompt addition, a const string): what Graybox is, that the open file
    is reached through the `graybox` MCP tools, to inspect before editing, to look at results with
    the screenshot/export tool, to keep changes small and named, and to answer in the user's
    language.
- The chat lives in memory per open file (not saved in the `.figcraft` file).
- UI-only state; a command id `game.claude.toggle` (Session::request_ui) so menus, shortcuts and
  the Actions palette can open it too.
- Upstream hooks: `toolbar.rs` (the button and `main_width`), the panel draw call in `app.rs` or
  `canvas.rs`. List them in `docs/game/STATUS.md`.

### Done when
In the real app: open a scratch file, click the button, ask "create a 400×300 frame named Menu
with a centred title", watch it appear, then a follow-up ("make the title orange") edits the same
title. Stop interrupts. Missing `claude` gives the plain message. `cargo xtask ci` passes (known
failures aside), wasm builds.

## Part 2: icons from sketches (G10.2, after part 1)

### What he sees
- **References**: right-click any layers (images, icons, frames) → "Use as icon reference". The
  file keeps the list (`GameSettings::icon_refs`, serde default, skip if empty); the Claude panel
  shows them as small thumbnails with a remove ×.
- **Make icon**: with a sketch selected (brush strokes, any layers), the Claude panel shows a
  "Make icon from selection" row: size (16, 24, 32, 48, 64; default 24), style (Same as references,
  Outline, Filled), name. Clicking it sends a chat turn with the sketch and references as images.
- The result: a frame named `Icon / <name>` at that size, placed to the right of the sketch, with
  the icon as clean vectors (pixel-grid aligned, consistent stroke, few points), coloured like the
  references. The sketch stays. Claude looks at its result and fixes it once if it is off. He can
  then say in the chat "thicker", "rounder", "make 3 variants", and Claude edits it.

### How it works
- The sketch and references are rendered to PNGs (`render_area`) in the chat's scratch folder; the
  turn's prompt names those files so Claude Code reads them as images.
- Claude writes an SVG and places it through an engine command that imports SVG text into a new
  frame of the chosen size at a given position (reuse the SVG importer; add the command if
  `edit.paste` can't take SVG text and a target box). Junk-sweep test for any new command.
- The prompt asks for an original icon in the references' style; it must not reproduce a
  third-party logo or product icon.

### Done when
In the real app: a scratch file with two simple reference icons and a rough brush sketch of a
heart gives a clean 24 px heart icon frame in the references' style, next to the sketch.
