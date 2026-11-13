//! Several blocks at once (G13.2): Escape selects a block, Shift+arrows and Shift+click grow the
//! selection, Delete removes it in one step, and copied blocks paste as blocks.

use super::blocks_tests::{blocks, click, key, key_with, open, shapes};
use crate::layers_drag_tests::Ui;
use crate::live_tests::text_center;
use egui::{Event, Key, Modifiers, PointerButton, Pos2};

fn words(u: &Ui, doc: u64) -> Vec<String> {
    blocks(u, doc).iter().map(figcraft_doc::docs::Block::plain_text).collect()
}

fn undo_len(u: &Ui) -> usize {
    u.app.session.ds().unwrap().history.undo.len()
}

/// The blocks selected now.
fn picked(u: &Ui) -> usize {
    let uid = super::state::uid(&u.app).unwrap();
    let st = super::state::take(uid);
    let n = st.picked.len();
    super::state::put(uid, st);
    n
}

fn shift_click(u: &mut Ui, at: Pos2) {
    let m = Modifiers::SHIFT;
    u.frame_with(vec![Event::PointerMoved(at)], m);
    u.frame_with(vec![Event::PointerButton { pos: at, button: PointerButton::Primary, pressed: true, modifiers: m }], m);
    u.frame_with(vec![Event::PointerButton { pos: at, button: PointerButton::Primary, pressed: false, modifiers: m }], m);
    u.frame(vec![]);
}

#[test]
fn escape_selects_the_block_shift_down_grows_it_and_delete_removes_both_in_one_step() {
    let (mut u, doc) = open(&[("paragraph", "alpha"), ("paragraph", "beta"), ("paragraph", "gamma")]);
    let at = text_center(&shapes(&mut u), "alpha");
    click(&mut u, at);
    key(&mut u, Key::Escape);
    assert_eq!(picked(&u), 1);
    key_with(&mut u, Key::ArrowDown, Modifiers::SHIFT);
    assert_eq!(picked(&u), 2);
    let undo = undo_len(&u);
    key(&mut u, Key::Delete);
    assert_eq!(words(&u, doc), ["gamma"]);
    assert_eq!(undo_len(&u), undo + 1, "one step");
    assert_eq!(picked(&u), 0);
}

#[test]
fn keys_with_nothing_written_in_leave_the_hidden_canvas_alone_but_undo_still_works() {
    let (mut u, doc) = open(&[("paragraph", "alpha")]);
    u.app.run("shape.rectangle", serde_json::json!({"x": 0, "y": 0, "width": 40, "height": 40}));
    let layers = u.app.session.doc().unwrap().node_count();
    u.frame(vec![]);
    // Delete, a tool key and a copy: the selected rectangle behind the document stays.
    key(&mut u, Key::Delete);
    key(&mut u, Key::R);
    u.frame(vec![Event::Copy]);
    assert_eq!(u.app.session.doc().unwrap().node_count(), layers, "the canvas took no key");
    // Undo still undoes (here: the rectangle).
    key_with(&mut u, Key::Z, Modifiers::COMMAND | Modifiers::CTRL);
    assert_eq!(u.app.session.doc().unwrap().node_count(), layers - 1);
    assert_eq!(words(&u, doc), ["alpha"]);
}

#[test]
fn shift_click_selects_from_the_block_written_in_to_the_one_clicked() {
    let (mut u, _) = open(&[("paragraph", "alpha"), ("paragraph", "beta"), ("paragraph", "gamma")]);
    let s = shapes(&mut u);
    let (a, c) = (text_center(&s, "alpha"), text_center(&s, "gamma"));
    click(&mut u, a);
    shift_click(&mut u, c);
    assert_eq!(picked(&u), 3);
    key(&mut u, Key::Escape);
    assert_eq!(picked(&u), 0, "Escape lets go");
}

#[test]
fn copied_blocks_paste_as_blocks_after_the_block_written_in() {
    let (mut u, doc) = open(&[("heading2", "Goals"), ("checklist", "Ship"), ("paragraph", "")]);
    let at = text_center(&shapes(&mut u), "Goals");
    click(&mut u, at);
    key(&mut u, Key::Escape);
    key_with(&mut u, Key::ArrowDown, Modifiers::SHIFT);
    let out = u.frame_output(vec![Event::Copy]);
    let copied = out
        .platform_output
        .commands
        .iter()
        .find_map(|c| match c {
            egui::OutputCommand::CopyText(t) => Some(t.clone()),
            _ => None,
        })
        .expect("Markdown on the clipboard");
    assert!(copied.contains("## Goals") && copied.contains("- [ ] Ship"), "{copied}");
    // In the empty paragraph at the end: the blocks come back as blocks, in its place.
    let last = blocks(&u, doc)[2].id;
    let uid = super::state::uid(&u.app).unwrap();
    let mut st = super::state::take(uid);
    st.focus = Some(super::state::Target::Block(last));
    super::state::put(uid, st);
    u.frame(vec![]);
    u.frame(vec![]);
    u.frame(vec![Event::Paste(copied)]);
    u.frame(vec![]);
    let b = blocks(&u, doc);
    assert_eq!(words(&u, doc), ["Goals", "Ship", "Goals", "Ship"]);
    assert!(matches!(b[3].kind, figcraft_doc::docs::BlockKind::Checklist { .. }), "kept their kind");
}
