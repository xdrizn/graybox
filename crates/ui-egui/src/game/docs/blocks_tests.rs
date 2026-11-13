//! The block editor's keys and marks (G13.2), driven with raw input like a person would: Markdown
//! shortcuts, Enter and Backspace as in Notion, Tab to nest, the list marks, the checkbox, the toggle
//! and the handle menu.

use egui::{Event, Key, Modifiers, PointerButton, Pos2, pos2};
use figcraft_doc::docs::{Block, BlockKind, DocId};
use figcraft_engine::Session;
use serde_json::json;

use crate::LeftTab;
use crate::layers_drag_tests::Ui;
use crate::live_tests::{flat, has_text, text_center, texts};

fn shapes(u: &mut Ui) -> Vec<egui::Shape> {
    flat(&u.frame_output(vec![]).shapes)
}

fn press(u: &mut Ui, at: Pos2, pressed: bool) {
    u.frame(vec![Event::PointerMoved(at), Event::PointerButton { pos: at, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE }]);
}

fn click(u: &mut Ui, at: Pos2) {
    press(u, at, true);
    press(u, at, false);
    u.frame(vec![]);
}

fn key_with(u: &mut Ui, key: Key, modifiers: Modifiers) {
    u.frame_with(vec![Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers }], modifiers);
    u.frame_with(vec![Event::Key { key, physical_key: None, pressed: false, repeat: false, modifiers }], modifiers);
    u.frame(vec![]);
}

fn key(u: &mut Ui, key: Key) {
    key_with(u, key, Modifiers::NONE);
}

fn type_text(u: &mut Ui, s: &str) {
    for ch in s.chars() {
        u.frame(vec![Event::Text(ch.to_string())]);
    }
    u.frame(vec![]);
}

/// The top-left corner and size of the text `s` on screen.
fn text_rect(u: &mut Ui, s: &str) -> egui::Rect {
    let found = texts(&shapes(u)).into_iter().find(|(t, _, _)| t == s);
    let (_, at, size) = found.unwrap_or_else(|| panic!("no text {s:?} on screen"));
    egui::Rect::from_min_size(at, size)
}

/// Put the keyboard in the block showing `s`, with the cursor `from_end` characters before its end.
fn caret_in(u: &mut Ui, s: &str, from_end: usize) {
    let at = text_center(&shapes(u), s);
    click(u, at);
    key(u, Key::End);
    for _ in 0..from_end {
        key(u, Key::ArrowLeft);
    }
}

fn open(blocks: &[(&str, &str)]) -> (Ui, u64) {
    let mut s = Session::new();
    let doc = s.execute("doc.create", &json!({"title": "Notes"})).unwrap()["id"].as_u64().unwrap();
    for (kind, text) in blocks {
        s.execute("doc.block.insert", &json!({"doc": doc, "kind": kind, "text": text})).unwrap();
    }
    let mut u = Ui::new(s);
    u.app.ui.left_tab = LeftTab::Documents;
    u.app.ui.open_left_panel();
    super::open(&mut u.app, doc);
    for _ in 0..3 {
        u.frame(vec![]);
    }
    (u, doc)
}

/// Let typed words go out as a command.
fn settle(u: &mut Ui) {
    // The clock is set, not moved: jump well past every frame so far (each is 1/60 s).
    thread_local! {
        static LATER: std::cell::Cell<f64> = const { std::cell::Cell::new(0.0) };
    }
    let later = LATER.with(|c| {
        c.set(c.get() + 1_000.0);
        c.get()
    });
    u.advance_clock(later);
    u.frame(vec![]);
    u.frame(vec![]);
}

fn blocks(u: &Ui, doc: u64) -> Vec<Block> {
    u.app.session.doc().unwrap().docs.get(DocId(doc)).unwrap().current().unwrap().blocks.clone()
}

fn words(u: &Ui, doc: u64) -> Vec<String> {
    blocks(u, doc).iter().map(Block::plain_text).collect()
}

fn undo_len(u: &Ui) -> usize {
    u.app.session.ds().unwrap().history.undo.len()
}

#[test]
fn typing_a_markdown_mark_at_the_start_turns_the_block_and_the_mark_goes() {
    for (mark, kind) in [
        ("## ", BlockKind::Heading { level: 2 }),
        ("- ", BlockKind::Bulleted),
        ("[] ", BlockKind::Checklist { checked: false }),
        ("> ", BlockKind::Quote),
        ("1. ", BlockKind::Numbered),
    ] {
        let (mut u, doc) = open(&[("paragraph", "x")]);
        caret_in(&mut u, "x", 1);
        type_text(&mut u, mark);
        settle(&mut u);
        let b = blocks(&u, doc);
        assert_eq!(b[0].kind, kind, "after typing {mark:?}");
        assert_eq!(b[0].plain_text(), "x", "the mark {mark:?} goes, the words stay");
    }
}

#[test]
fn a_mark_typed_at_the_start_of_a_list_item_turns_it_too() {
    let (mut u, doc) = open(&[("numbered", "x")]);
    caret_in(&mut u, "x", 1);
    type_text(&mut u, "## ");
    settle(&mut u);
    assert_eq!(blocks(&u, doc)[0].kind, BlockKind::Heading { level: 2 });
    assert_eq!(words(&u, doc), ["x"]);
}

#[test]
fn three_dashes_make_a_divider_and_writing_goes_on_below_it() {
    let (mut u, doc) = open(&[]);
    let at = text_center(&shapes(&mut u), "Click here and start writing");
    click(&mut u, at);
    type_text(&mut u, "---");
    type_text(&mut u, "next");
    settle(&mut u);
    let b = blocks(&u, doc);
    assert_eq!(b[0].kind, BlockKind::Divider);
    assert_eq!(b.last().map(Block::plain_text).as_deref(), Some("next"));
}

#[test]
fn enter_in_the_middle_splits_the_block_in_one_undo_step_and_writing_goes_on_in_the_new_one() {
    let (mut u, doc) = open(&[("paragraph", "Hello world")]);
    caret_in(&mut u, "Hello world", 6);
    let before = undo_len(&u);
    key(&mut u, Key::Enter);
    assert_eq!(words(&u, doc), ["Hello", " world"]);
    assert_eq!(undo_len(&u), before + 1);
    // The cursor is at the start of the new block.
    type_text(&mut u, "big");
    settle(&mut u);
    assert_eq!(words(&u, doc), ["Hello", "big world"]);
}

#[test]
fn enter_on_an_empty_list_item_ends_the_list() {
    let (mut u, doc) = open(&[("bulleted", "one")]);
    caret_in(&mut u, "one", 0);
    key(&mut u, Key::Enter);
    assert_eq!(blocks(&u, doc)[1].kind, BlockKind::Bulleted);
    key(&mut u, Key::Enter);
    assert_eq!(blocks(&u, doc).len(), 2);
    assert_eq!(blocks(&u, doc)[1].kind, BlockKind::Paragraph);
}

#[test]
fn enter_on_an_empty_heading_starts_text_below_and_the_heading_stays() {
    let (mut u, doc) = open(&[("heading2", "")]);
    // An empty heading says which level it is.
    let at = text_center(&shapes(&mut u), "Heading 2");
    click(&mut u, at);
    // A heading is not a list to end: Enter adds a line below it.
    key(&mut u, Key::Enter);
    let b = blocks(&u, doc);
    assert_eq!(b.len(), 2);
    assert_eq!(b[0].kind, BlockKind::Heading { level: 2 });
    assert_eq!(b[1].kind, BlockKind::Paragraph);
}

#[test]
fn backspace_at_the_start_of_a_decision_keeps_it() {
    let (mut u, doc) = open(&[("paragraph", "before"), ("decision", "Ship on Friday")]);
    caret_in(&mut u, "Ship on Friday", 14);
    key(&mut u, Key::Backspace);
    let b = blocks(&u, doc);
    assert_eq!(b.len(), 2);
    assert_eq!(b[1].kind.name(), "decision");
    assert_eq!(b[1].plain_text(), "Ship on Friday");
}

#[test]
fn backspace_in_an_empty_decision_turns_it_into_text() {
    let (mut u, doc) = open(&[("paragraph", "before"), ("decision", "")]);
    let at = text_center(&shapes(&mut u), "What was decided");
    click(&mut u, at);
    key(&mut u, Key::Backspace);
    assert_eq!(blocks(&u, doc)[1].kind, BlockKind::Paragraph);
}

#[test]
fn shift_enter_breaks_the_line_inside_the_block() {
    let (mut u, doc) = open(&[("paragraph", "Hello world")]);
    caret_in(&mut u, "Hello world", 6);
    key_with(&mut u, Key::Enter, Modifiers::SHIFT);
    settle(&mut u);
    assert_eq!(words(&u, doc), ["Hello\n world"]);
}

#[test]
fn enter_with_words_selected_replaces_them() {
    let (mut u, doc) = open(&[("paragraph", "Hello world")]);
    caret_in(&mut u, "Hello world", 6);
    key_with(&mut u, Key::End, Modifiers::SHIFT);
    key(&mut u, Key::Enter);
    settle(&mut u);
    assert_eq!(words(&u, doc), ["Hello", ""]);
}

#[test]
fn backspace_at_the_start_turns_a_list_item_into_text_then_joins_it_to_the_block_above() {
    let (mut u, doc) = open(&[("paragraph", "Hello"), ("bulleted", " world")]);
    caret_in(&mut u, " world", 6);
    key(&mut u, Key::Backspace);
    assert_eq!(blocks(&u, doc)[1].kind, BlockKind::Paragraph);
    key(&mut u, Key::Backspace);
    assert_eq!(words(&u, doc), ["Hello world"]);
    // The cursor sits where the two met.
    type_text(&mut u, ",");
    settle(&mut u);
    assert_eq!(words(&u, doc), ["Hello, world"]);
}

#[test]
fn tab_nests_a_block_under_the_one_above_and_shift_tab_takes_it_out() {
    let (mut u, doc) = open(&[("bulleted", "parent"), ("bulleted", "child")]);
    // The document was opened from the panel: keyboard navigation starts there.
    u.app.ui.nav.region = crate::a11y::Region::Layers;
    caret_in(&mut u, "child", 0);
    key(&mut u, Key::Tab);
    let b = blocks(&u, doc);
    assert_eq!(b.len(), 1);
    assert_eq!(b[0].children[0].plain_text(), "child");
    // The keyboard stays in the block.
    type_text(&mut u, "!");
    settle(&mut u);
    assert_eq!(blocks(&u, doc)[0].children[0].plain_text(), "child!");
    key_with(&mut u, Key::Tab, Modifiers::SHIFT);
    assert_eq!(words(&u, doc), ["parent", "child!"]);
}

#[test]
fn numbered_items_count_and_a_block_in_between_starts_again() {
    let (mut u, _) = open(&[("numbered", "first"), ("numbered", "second"), ("paragraph", "break"), ("numbered", "again")]);
    let sh = shapes(&mut u);
    assert!(has_text(&sh, "1.") && has_text(&sh, "2."));
    let again = texts(&sh).into_iter().filter(|(t, _, _)| t == "1.").count();
    assert_eq!(again, 2);
}

#[test]
fn clicking_the_box_of_a_task_ticks_it_and_again_unticks_it() {
    let (mut u, doc) = open(&[("checklist", "Ship it")]);
    for want in [true, false] {
        let r = text_rect(&mut u, "Ship it");
        click(&mut u, pos2(r.min.x - 12.0, r.center().y));
        assert_eq!(blocks(&u, doc)[0].kind, BlockKind::Checklist { checked: want });
    }
}

#[test]
fn a_closed_toggle_hides_what_is_inside_and_its_arrow_opens_it() {
    let mut s = Session::new();
    let doc = s.execute("doc.create", &json!({"title": "Notes"})).unwrap()["id"].as_u64().unwrap();
    let t = s.execute("doc.block.insert", &json!({"doc": doc, "kind": "toggle", "text": "More"})).unwrap()["id"].as_u64().unwrap();
    s.execute("doc.block.insert", &json!({"doc": doc, "parent": t, "text": "Hidden words"})).unwrap();
    let mut u = Ui::new(s);
    u.app.ui.left_tab = LeftTab::Documents;
    u.app.ui.open_left_panel();
    super::open(&mut u.app, doc);
    for _ in 0..3 {
        u.frame(vec![]);
    }
    assert!(!has_text(&shapes(&mut u), "Hidden words"));
    let r = text_rect(&mut u, "More");
    click(&mut u, pos2(r.min.x - 12.0, r.center().y));
    assert_eq!(blocks(&u, doc)[0].kind, BlockKind::Toggle { open: true });
    assert!(has_text(&shapes(&mut u), "Hidden words"));
}

#[test]
fn the_handle_menu_duplicates_a_block() {
    let (mut u, doc) = open(&[("callout", "Remember")]);
    let r = text_rect(&mut u, "Remember");
    // Hover the block so its handle shows, then open the handle's menu.
    u.frame(vec![Event::PointerMoved(r.center())]);
    u.frame(vec![]);
    let handle = super::blocks::last_handle().expect("a handle while the pointer is on the block");
    click(&mut u, handle.center());
    let at = text_center(&shapes(&mut u), "Duplicate");
    click(&mut u, at);
    assert_eq!(words(&u, doc), ["Remember", "Remember"]);
}
