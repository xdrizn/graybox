//! Graybox: a chat with Claude inside the editor (G10.1). A button beside Play opens a panel; what
//! the person types goes to the Claude Code CLI installed on the PC (his own plan), which edits
//! the open file through the `graybox` MCP tools (`figcraft-cli mcp --attach <port>`).
//!
//! Desktop only: the browser build shows the button and says the chat needs the desktop app.
//! Spec: `docs/game/specs/2026-11-10-claude-chat-design.md`.

#![cfg_attr(target_arch = "wasm32", allow(dead_code))]

use std::cell::RefCell;

use crate::FigApp;

#[cfg(not(target_arch = "wasm32"))]
pub mod control;
#[cfg(not(target_arch = "wasm32"))]
mod discover;
pub mod events;
mod guide;
mod markdown;
mod panel;
#[cfg(not(target_arch = "wasm32"))]
pub mod runner;
pub mod state;

#[cfg(test)]
mod tests;

use state::{Chat, Conversation, Queued};

thread_local! {
    /// The chat lives with the UI thread, outside `FigApp`, so upstream state stays untouched.
    static CHAT: RefCell<Chat> = RefCell::new(Chat::default());
}

/// Run `f` on the chat state (taken out while it runs, so `f` may use the whole app).
fn with_chat<R>(f: impl FnOnce(&mut Chat) -> R) -> R {
    let mut chat = CHAT.with(|c| std::mem::take(&mut *c.borrow_mut()));
    let r = f(&mut chat);
    CHAT.with(|c| *c.borrow_mut() = chat);
    r
}

/// Whether the panel is open.
pub fn is_open() -> bool {
    CHAT.with(|c| c.borrow().open)
}

/// Open or close the panel (the toolbar button, `game.claude.toggle`).
pub fn toggle() {
    CHAT.with(|c| {
        let mut c = c.borrow_mut();
        c.open = !c.open;
        c.focus = c.open;
    });
}

/// The uid of the file on screen.
fn active_uid(app: &FigApp) -> Option<u64> {
    app.session.docs.get(app.session.active).map(|d| d.uid)
}

/// Send what is typed in `conv` (desktop). Finds the programs first so a missing one is told at once.
#[cfg(not(target_arch = "wasm32"))]
pub fn send(conv: &mut Conversation, images: Vec<std::path::PathBuf>, now: f64) {
    let text = conv.input.trim().to_string();
    if text.is_empty() || conv.busy {
        return;
    }
    conv.input.clear();
    conv.begin_turn(&text);
    if discover::find_claude().is_none() {
        conv.fail(
            "Claude Code isn't installed on this PC. Install it from claude.com/claude-code, open a terminal, type claude auth login to sign in, then ask again here.",
        );
        return;
    }
    if discover::find_cli().is_none() {
        conv.fail(
            "Graybox can't find figcraft-cli next to the app. It should sit in the same folder as the Graybox program; update or reinstall Graybox.",
        );
        return;
    }
    conv.queued = Some(Queued { text, images, since: now });
    control::ask();
}

/// Start queued messages once the control channel is open, and take the events of running turns.
/// Every frame, for every file's conversation (a turn goes on when the person switches files).
pub fn tick(app: &mut FigApp, ctx: &egui::Context) {
    let _ = app;
    with_chat(|chat| {
        for conv in chat.convs.values_mut() {
            tick_conversation(conv, ctx);
        }
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn tick_conversation(conv: &mut Conversation, ctx: &egui::Context) {
    if let Some(run) = &conv.run {
        let events = run.poll();
        for e in events {
            conv.apply(e);
        }
    }
    let Some(q) = conv.queued.clone() else { return };
    let now = ctx.input(|i| i.time);
    let (Some(port), Some(claude), Some(cli)) = (control::port(), discover::find_claude(), discover::find_cli()) else {
        if now - q.since > 15.0 {
            conv.fail("Graybox couldn't open the connection Claude needs to edit the file. Close and reopen Graybox, then try again.");
        } else {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        return;
    };
    conv.queued = None;
    let turn = runner::Turn { prompt: q.text, images: q.images, session: conv.session.clone(), claude, cli, port };
    let wake = ctx.clone();
    match runner::Run::start(&turn, move || wake.request_repaint()) {
        Ok(run) => conv.run = Some(run),
        Err(e) => conv.fail(e),
    }
}

#[cfg(target_arch = "wasm32")]
fn tick_conversation(_conv: &mut Conversation, _ctx: &egui::Context) {}

/// Draw the panel when it is open (after the editor, before the toasts).
pub fn show(app: &mut FigApp, ctx: &egui::Context) {
    if !is_open() {
        return;
    }
    let Some(uid) = active_uid(app) else { return };
    with_chat(|chat| panel::show(app, ctx, chat, uid));
}
