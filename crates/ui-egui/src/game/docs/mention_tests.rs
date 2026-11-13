//! Mentions in documents (G13.2): `@` lists the file's documents and frames, Enter puts the one
//! picked into the words, and Ctrl+click on it goes to it.

use egui::{Event, Key, Modifiers, PointerButton, Pos2};
use figcraft_doc::docs::DocId;
use figcraft_engine::Session;
use serde_json::json;

use super::blocks_tests::{blocks, click, key, settle, shapes, type_text};
use crate::LeftTab;
use crate::layers_drag_tests::Ui;
use crate::live_tests::{has_text, text_center};

/// A file with a frame `HUD`, a document `Combat brief`, and a document `Notes` with one empty
/// paragraph, open in the editor.
fn with_file() -> (Ui, u64, u64, u64) {
    let mut s = Session::new();
    let frame = s.execute("frame.create", &json!({"x": 0, "y": 0, "width": 320, "height": 180, "name": "HUD"})).unwrap()["id"].as_u64().unwrap();
    let other = s.execute("doc.create", &json!({"title": "Combat brief"})).unwrap()["id"].as_u64().unwrap();
    let doc = s.execute("doc.create", &json!({"title": "Notes"})).unwrap()["id"].as_u64().unwrap();
    s.execute("doc.block.insert", &json!({"doc": doc, "kind": "paragraph"})).unwrap();
    let mut u = Ui::new(s);
    u.app.ui.left_tab = LeftTab::Documents;
    u.app.ui.open_left_panel();
    super::open(&mut u.app, doc);
    for _ in 0..3 {
        u.frame(vec![]);
    }
    (u, doc, frame, other)
}

/// The document open in the editor.
fn open_doc(u: &Ui) -> Option<DocId> {
    let uid = super::state::uid(&u.app)?;
    let st = super::state::take(uid);
    let open = st.open;
    super::state::put(uid, st);
    open
}

fn ctrl_click(u: &mut Ui, at: Pos2) {
    let m = Modifiers::CTRL | Modifiers::COMMAND;
    u.frame_with(vec![Event::PointerMoved(at)], m);
    u.frame_with(vec![Event::PointerButton { pos: at, button: PointerButton::Primary, pressed: true, modifiers: m }], m);
    u.frame_with(vec![Event::PointerButton { pos: at, button: PointerButton::Primary, pressed: false, modifiers: m }], m);
    u.frame(vec![]);
}

#[test]
fn at_lists_documents_and_frames_and_enter_mentions_the_one_found() {
    let (mut u, doc, frame, _) = with_file();
    let at = text_center(&shapes(&mut u), "Type “/” for blocks");
    click(&mut u, at);
    // The documents tree lists the documents too: the menu adds one more of each.
    let s = shapes(&mut u);
    let (notes, brief) = (count(&s, "Notes"), count(&s, "Combat brief"));
    type_text(&mut u, "see @");
    let s = shapes(&mut u);
    assert_eq!(count(&s, "Combat brief"), brief + 1, "the other document is listed");
    assert!(has_text(&s, "HUD"), "the frame is listed");
    assert_eq!(count(&s, "Notes"), notes, "not the document itself");
    type_text(&mut u, "hu");
    let s = shapes(&mut u);
    assert!(has_text(&s, "HUD") && count(&s, "Combat brief") == brief, "the letters filter");
    key(&mut u, Key::Enter);
    type_text(&mut u, "today");
    settle(&mut u);
    let b = &blocks(&u, doc)[0];
    assert_eq!(b.plain_text(), "see @HUD today");
    assert_eq!(b.text.iter().find_map(|r| r.mention), Some(figcraft_doc::docs::Mention::Layer { node: figcraft_doc::NodeId(frame) }));
    assert_eq!(blocks(&u, doc).len(), 1, "Enter picked, it did not split the block");
}

#[test]
fn ctrl_click_on_a_mention_opens_the_document_or_shows_the_layer() {
    let (mut u, doc, frame, other) = with_file();
    let para = blocks(&u, doc)[0].id.0;
    u.app.run("doc.block.set", json!({"doc": doc, "id": para, "text": "a"}));
    u.app.run("doc.block.mention", json!({"doc": doc, "id": para, "from": 0, "to": 1, "document": other}));
    let next = u.app.run("doc.block.insert", json!({"doc": doc, "text": "b"})).unwrap()["id"].as_u64().unwrap();
    u.app.run("doc.block.mention", json!({"doc": doc, "id": next, "from": 0, "to": 1, "layer": frame}));
    let at = start_of(&shapes(&mut u), "@Combat brief ");
    ctrl_click(&mut u, at);
    assert_eq!(open_doc(&u), Some(DocId(other)));
    super::open(&mut u.app, doc);
    let at = start_of(&shapes(&mut u), "@HUD ");
    ctrl_click(&mut u, at);
    assert_eq!(open_doc(&u), None, "back on the canvas");
    assert_eq!(u.app.session.ds().unwrap().selection, vec![figcraft_doc::NodeId(frame)]);
}

fn count(s: &[egui::Shape], text: &str) -> usize {
    crate::live_tests::texts(s).iter().filter(|(t, _, _)| t == text).count()
}

/// A point on the first letters of the words `text`.
fn start_of(s: &[egui::Shape], text: &str) -> Pos2 {
    let (_, p, size) = crate::live_tests::texts(s).into_iter().find(|(t, _, _)| t == text).unwrap_or_else(|| panic!("no text {text:?}"));
    p + egui::vec2(14.0, size.y / 2.0)
}
