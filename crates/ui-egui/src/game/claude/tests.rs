//! The chat button and panel, driven through egui headless.

use egui::accesskit::{Node, Role};
use egui::{Event, PointerButton, pos2};
use figcraft_engine::Session;
use serde_json::json;

use super::state::Message;
use super::{is_open, with_chat};
use crate::layers_drag_tests::Ui;

fn nodes(u: &mut Ui) -> Vec<Node> {
    u.ctx.enable_accesskit();
    u.frame(vec![]);
    let out = u.frame_output(vec![]);
    out.platform_output.accesskit_update.map(|a| a.nodes.into_iter().map(|(_, n)| n).collect()).unwrap_or_default()
}

fn button(u: &mut Ui, label: &str) -> Option<Node> {
    nodes(u).into_iter().find(|n| n.role() == Role::Button && n.label() == Some(label))
}

fn click(u: &mut Ui, node: &Node) {
    let b = node.bounds().expect("the button has a place");
    let at = pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32);
    u.frame(vec![Event::PointerMoved(at)]);
    u.frame(vec![Event::PointerButton { pos: at, button: PointerButton::Primary, pressed: true, modifiers: Default::default() }]);
    u.frame(vec![Event::PointerButton { pos: at, button: PointerButton::Primary, pressed: false, modifiers: Default::default() }]);
    u.frame(vec![]);
}

fn app() -> Ui {
    let mut s = Session::new();
    s.execute("frame.create", &json!({"x": 0, "y": 0, "width": 100, "height": 100})).unwrap();
    Ui::new(s)
}

#[test]
fn the_toolbar_button_opens_and_closes_the_panel() {
    let mut u = app();
    assert!(!is_open());
    let chat = button(&mut u, "Chat with Claude").expect("a Chat with Claude button beside Play");
    click(&mut u, &chat);
    assert!(is_open(), "the button opens the panel");
    let close = button(&mut u, "Close Claude chat").expect("the panel has a close button");
    click(&mut u, &close);
    assert!(!is_open(), "the panel's × closes it");
}

#[test]
fn the_toggle_command_opens_the_panel_from_anywhere() {
    let mut u = app();
    u.app.run("game.claude.toggle", json!({}));
    assert!(is_open());
    u.app.run("game.claude.toggle", json!({}));
    assert!(!is_open());
}

#[test]
fn the_panel_draws_every_kind_of_message_and_a_missing_claude_is_told() {
    let mut u = app();
    u.app.run("game.claude.toggle", json!({}));
    u.frame(vec![]);
    let uid = u.app.session.docs.get(u.app.session.active).map(|d| d.uid).unwrap();
    with_chat(|chat| {
        let c = chat.convs.entry(uid).or_default();
        c.messages = vec![
            Message::User("make a frame".into()),
            Message::Claude("Done: **Menu** is a `frame`.\n\n- one\n- two\n\n```\ncode\n```".into()),
            Message::Note("Stopped.".into()),
            Message::Error("Claude Code isn't installed on this PC.".into()),
        ];
        c.busy = true;
        c.trace = Some("Drawing a frame…".into());
    });
    for _ in 0..3 {
        u.frame(vec![]);
    }
    // Stop is offered while Claude works, and ends a turn that has not started.
    assert!(button(&mut u, "Stop").is_some(), "Stop is offered while Claude works");
    with_chat(|chat| {
        let c = chat.convs.entry(uid).or_default();
        c.reset();
        assert!(!c.busy && c.messages.is_empty());
    });
    u.frame(vec![]);
}

#[test]
fn each_file_has_its_own_conversation() {
    let mut u = app();
    let uid = u.app.session.docs.get(u.app.session.active).map(|d| d.uid).unwrap();
    with_chat(|chat| {
        chat.convs.entry(uid).or_default().messages.push(Message::User("hi".into()));
        chat.convs.entry(uid + 1).or_default();
    });
    with_chat(|chat| {
        assert_eq!(chat.convs.get(&uid).map(|c| c.messages.len()), Some(1));
        assert_eq!(chat.convs.get(&(uid + 1)).map(|c| c.messages.len()), Some(0));
    });
    u.frame(vec![]);
}

/// Graybox (G9): the panel lines up: the close box's right edge, the input's and Send's are one
/// column, and New chat sits left of the close box on the title's line.
#[test]
fn the_panel_lines_up_its_header_input_and_send() {
    let mut u = app();
    u.app.run("game.claude.toggle", json!({}));
    let all = nodes(&mut u);
    let find = |role: Role, label: &str| all.iter().find(|n| n.role() == role && n.label() == Some(label)).and_then(Node::bounds);
    let close = find(Role::Button, "Close Claude chat").expect("close");
    let send = find(Role::Button, "Send").expect("send");
    let new = find(Role::Button, "New chat").expect("new chat");
    assert!((close.x1 - send.x1).abs() <= 1.0, "close {close:?} and Send {send:?} share a right edge");
    assert!(new.x1 < close.x0 && (new.y0 + new.y1 - close.y0 - close.y1).abs() <= 1.0, "New chat {new:?} beside close {close:?}");
}
