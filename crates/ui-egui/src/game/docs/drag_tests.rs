//! Moving blocks (G13.2): drag a block by its handle, or Ctrl+Shift+arrows.

use egui::{Event, Key, Modifiers, PointerButton, Pos2, pos2};
use figcraft_doc::docs::Block;
use serde_json::json;

use super::blocks_tests::{blocks, caret_in, key_with, open, text_rect, words};
use crate::layers_drag_tests::Ui;

/// The handle of the block showing `s`, once the pointer is on the block.
fn handle_of(u: &mut Ui, s: &str) -> Pos2 {
    let r = text_rect(u, s);
    u.frame(vec![Event::PointerMoved(r.center())]);
    u.frame(vec![]);
    super::blocks::last_handle().expect("a handle while the pointer is on the block").center()
}

/// Press at `from`, move to `to` in steps, release there.
fn drag(u: &mut Ui, from: Pos2, to: Pos2) {
    let button = |pos, pressed| Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
    u.frame(vec![Event::PointerMoved(from), button(from, true)]);
    for k in 1..=6 {
        let t = k as f32 / 6.0;
        u.frame(vec![Event::PointerMoved(from + (to - from) * t)]);
    }
    u.frame(vec![button(to, false)]);
    u.frame(vec![]);
}

#[test]
fn dragging_a_handle_above_the_first_block_moves_the_block_there() {
    let (mut u, doc) = open(&[("paragraph", "alpha"), ("paragraph", "beta"), ("paragraph", "gamma")]);
    let from = handle_of(&mut u, "gamma");
    let top = text_rect(&mut u, "alpha").min.y - 2.0;
    drag(&mut u, from, pos2(from.x, top));
    assert_eq!(words(&u, doc), ["gamma", "alpha", "beta"]);
}

#[test]
fn dragging_below_the_last_block_moves_it_to_the_end() {
    let (mut u, doc) = open(&[("paragraph", "alpha"), ("paragraph", "beta"), ("paragraph", "gamma")]);
    let from = handle_of(&mut u, "alpha");
    let below = text_rect(&mut u, "gamma").max.y + 12.0;
    drag(&mut u, from, pos2(from.x, below));
    assert_eq!(words(&u, doc), ["beta", "gamma", "alpha"]);
}

#[test]
fn a_block_dropped_into_its_own_content_stays_put() {
    let mut s = figcraft_engine::Session::new();
    let doc = s.execute("doc.create", &json!({"title": "Notes"})).unwrap()["id"].as_u64().unwrap();
    let t = s.execute("doc.block.insert", &json!({"doc": doc, "kind": {"type": "toggle", "open": true}, "text": "Group"})).unwrap()["id"]
        .as_u64()
        .unwrap();
    s.execute("doc.block.insert", &json!({"doc": doc, "parent": t, "text": "inside one"})).unwrap();
    s.execute("doc.block.insert", &json!({"doc": doc, "parent": t, "text": "inside two"})).unwrap();
    let mut u = Ui::new(s);
    u.app.ui.left_tab = crate::LeftTab::Documents;
    u.app.ui.open_left_panel();
    super::open(&mut u.app, doc);
    for _ in 0..3 {
        u.frame(vec![]);
    }
    let before = blocks(&u, doc);
    let from = handle_of(&mut u, "Group");
    let between = text_rect(&mut u, "inside two").min.y - 2.0;
    drag(&mut u, from, pos2(from.x, between));
    assert_eq!(blocks(&u, doc), before);
}

#[test]
fn ctrl_shift_arrows_move_the_block_among_its_neighbours() {
    let (mut u, doc) = open(&[("paragraph", "alpha"), ("paragraph", "beta"), ("paragraph", "gamma")]);
    caret_in(&mut u, "beta", 0);
    key_with(&mut u, Key::ArrowUp, Modifiers::COMMAND | Modifiers::SHIFT);
    assert_eq!(words(&u, doc), ["beta", "alpha", "gamma"]);
    key_with(&mut u, Key::ArrowDown, Modifiers::COMMAND | Modifiers::SHIFT);
    key_with(&mut u, Key::ArrowDown, Modifiers::COMMAND | Modifiers::SHIFT);
    assert_eq!(words(&u, doc), ["alpha", "gamma", "beta"]);
    // At the end it stays.
    key_with(&mut u, Key::ArrowDown, Modifiers::COMMAND | Modifiers::SHIFT);
    assert_eq!(blocks(&u, doc).last().map(Block::plain_text).as_deref(), Some("beta"));
}

#[test]
fn escape_during_a_drag_cancels_it() {
    let (mut u, doc) = open(&[("paragraph", "alpha"), ("paragraph", "beta"), ("paragraph", "gamma")]);
    let from = handle_of(&mut u, "gamma");
    let to = pos2(from.x, text_rect(&mut u, "alpha").min.y - 2.0);
    let button = |pos, pressed| Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
    u.frame(vec![Event::PointerMoved(from), button(from, true)]);
    for k in 1..=6 {
        u.frame(vec![Event::PointerMoved(from + (to - from) * (k as f32 / 6.0))]);
    }
    u.frame(vec![Event::Key { key: Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }]);
    u.frame(vec![button(to, false)]);
    u.frame(vec![]);
    assert_eq!(words(&u, doc), ["alpha", "beta", "gamma"]);
}
