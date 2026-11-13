//! The Documents tab and the editor, driven with raw input like a person would.

use egui::{Event, Modifiers, PointerButton};
use figcraft_engine::Session;
use serde_json::json;

use crate::LeftTab;
use crate::layers_drag_tests::Ui;
use crate::live_tests::{flat, has_text, text_center};

fn session_with_documents() -> (Session, u64, u64) {
    let mut s = Session::new();
    let a = s.execute("doc.create", &json!({"title": "Spec"})).unwrap()["id"].as_u64().unwrap();
    let b = s.execute("doc.create", &json!({"title": "Notes"})).unwrap()["id"].as_u64().unwrap();
    (s, a, b)
}

fn shapes(u: &mut Ui) -> Vec<egui::Shape> {
    flat(&u.frame_output(vec![]).shapes)
}

fn click(u: &mut Ui, at: egui::Pos2) {
    for pressed in [true, false] {
        u.frame(vec![Event::PointerMoved(at), Event::PointerButton { pos: at, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE }]);
    }
    u.frame(vec![]);
}

fn documents_tab(s: Session) -> Ui {
    let mut u = Ui::new(s);
    u.app.ui.left_tab = LeftTab::Documents;
    u.app.ui.open_left_panel();
    for _ in 0..3 {
        u.frame(vec![]);
    }
    u
}

#[test]
fn the_tab_lists_the_documents_and_clicking_one_opens_it_in_place_of_the_canvas() {
    let (s, a, _) = session_with_documents();
    let mut u = documents_tab(s);
    let sh = shapes(&mut u);
    assert!(has_text(&sh, "Spec") && has_text(&sh, "Notes"));
    assert_eq!(super::open_document(&u.app), None);
    let at = text_center(&sh, "Spec");
    click(&mut u, at);
    assert_eq!(super::open_document(&u.app), Some(a));
    // The editor shows the title and the way back.
    let sh = shapes(&mut u);
    assert!(has_text(&sh, "Back to canvas"));
    // Back to the canvas.
    let back = text_center(&sh, "Back to canvas");
    click(&mut u, back);
    assert_eq!(super::open_document(&u.app), None);
    assert!(!has_text(&shapes(&mut u), "Back to canvas"));
}

#[test]
fn the_new_document_row_makes_a_document_and_opens_it() {
    let mut u = documents_tab(Session::new());
    assert!(has_text(&shapes(&mut u), "No documents yet"));
    let at = text_center(&shapes(&mut u), "New document");
    click(&mut u, at);
    let docs = &u.app.session.doc().unwrap().docs;
    assert_eq!(docs.len(), 1);
    assert_eq!(super::open_document(&u.app), docs.walk().first().map(|d| d.0));
}

#[test]
fn leaving_the_documents_tab_shows_the_canvas_again_and_coming_back_resumes() {
    let (s, a, _) = session_with_documents();
    let mut u = documents_tab(s);
    super::open(&mut u.app, a);
    u.frame(vec![]);
    assert!(has_text(&shapes(&mut u), "Back to canvas"));
    u.app.ui.left_tab = LeftTab::File;
    u.frame(vec![]);
    assert!(!has_text(&shapes(&mut u), "Back to canvas"));
    u.app.ui.left_tab = LeftTab::Documents;
    u.frame(vec![]);
    assert!(has_text(&shapes(&mut u), "Back to canvas"));
    assert_eq!(super::open_document(&u.app), Some(a));
}

#[test]
fn deleting_the_open_document_closes_the_editor() {
    let (s, a, _) = session_with_documents();
    let mut u = documents_tab(s);
    super::open(&mut u.app, a);
    u.frame(vec![]);
    u.app.run("doc.delete", json!({"id": a}));
    u.frame(vec![]);
    u.frame(vec![]);
    assert_eq!(super::open_document(&u.app), None);
    assert!(!has_text(&shapes(&mut u), "Back to canvas"));
}

#[test]
fn typing_in_a_block_is_sent_as_one_command_after_a_pause() {
    let (mut s, a, _) = session_with_documents();
    let b = s.execute("doc.block.insert", &json!({"doc": a, "kind": "paragraph", "text": "x"})).unwrap()["id"].as_u64().unwrap();
    let mut u = documents_tab(s);
    super::open(&mut u.app, a);
    for _ in 0..3 {
        u.frame(vec![]);
    }
    let at = text_center(&shapes(&mut u), "x");
    click(&mut u, at);
    let undo_before = u.app.session.ds().unwrap().history.undo.len();
    u.frame(vec![Event::Key { key: egui::Key::End, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }]);
    for ch in ["h", "e", "l", "l", "o"] {
        u.frame(vec![Event::Text(ch.to_string())]);
    }
    // Not sent while the words are still coming.
    let block_text = |u: &Ui| u.app.session.doc().unwrap().docs.get(figcraft_doc::docs::DocId(a)).unwrap().current().unwrap().blocks[0].plain_text();
    assert_eq!(block_text(&u), "x");
    u.advance_clock(1.0);
    u.frame(vec![]);
    u.frame(vec![]);
    assert_eq!(block_text(&u), "xhello");
    assert_eq!(u.app.session.ds().unwrap().history.undo.len(), undo_before + 1, "one burst of typing is one undo step");
    let _ = b;
}

fn titles(u: &Ui, parent: Option<figcraft_doc::docs::DocId>) -> Vec<String> {
    let docs = &u.app.session.doc().unwrap().docs;
    docs.siblings(parent).iter().map(|d| docs.get(*d).unwrap().title.clone()).collect()
}

#[test]
fn dropping_a_row_before_after_or_inside_another_moves_it_there() {
    use super::tree::{Drop, apply_drop};
    use figcraft_doc::docs::DocId;
    let mut s = Session::new();
    let ids: Vec<u64> = ["A", "B", "C", "D"].iter().map(|t| s.execute("doc.create", &json!({"title": t})).unwrap()["id"].as_u64().unwrap()).collect();
    let mut u = documents_tab(s);
    let uid = super::state::uid(&u.app).unwrap();
    let do_drop = |u: &mut Ui, dragged: u64, to: Drop| {
        let mut st = super::state::take(uid);
        apply_drop(&mut u.app, &mut st, DocId(dragged), to);
        super::state::put(uid, st);
    };
    // D before A; then A after C (a later place: the index counts without the lifted row).
    do_drop(&mut u, ids[3], Drop::Before(DocId(ids[0])));
    assert_eq!(titles(&u, None), ["D", "A", "B", "C"]);
    do_drop(&mut u, ids[0], Drop::After(DocId(ids[2])));
    assert_eq!(titles(&u, None), ["D", "B", "C", "A"]);
    // Dropped where it already is: nothing happens (no new undo step).
    let steps = u.app.session.ds().unwrap().history.undo.len();
    do_drop(&mut u, ids[1], Drop::After(DocId(ids[3])));
    assert_eq!(u.app.session.ds().unwrap().history.undo.len(), steps);
    // Inside another row nests it as the last child.
    do_drop(&mut u, ids[1], Drop::Inside(DocId(ids[0])));
    assert_eq!(titles(&u, None), ["D", "C", "A"]);
    assert_eq!(titles(&u, Some(DocId(ids[0]))), ["B"]);
}

#[test]
fn enter_in_a_nested_block_adds_its_sibling_inside_the_same_parent() {
    use figcraft_doc::docs::DocId;
    let (mut s, a, _) = session_with_documents();
    let top = s.execute("doc.block.insert", &json!({"doc": a, "text": "top"})).unwrap()["id"].as_u64().unwrap();
    s.execute("doc.block.insert", &json!({"doc": a, "text": "kid", "parent": top})).unwrap();
    s.execute("doc.block.insert", &json!({"doc": a, "text": "last"})).unwrap();
    let mut u = documents_tab(s);
    super::open(&mut u.app, a);
    for _ in 0..3 {
        u.frame(vec![]);
    }
    let at = text_center(&shapes(&mut u), "kid");
    click(&mut u, at);
    u.frame(vec![Event::Key { key: egui::Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }]);
    u.frame(vec![]);
    let v = u.app.session.doc().unwrap().docs.get(DocId(a)).unwrap().current().unwrap().clone();
    // Two top-level blocks as before; the parent now holds the old child and the new block after it.
    assert_eq!(v.blocks.len(), 2, "the new block must not land at the top");
    assert_eq!(v.blocks[0].children.len(), 2);
    assert_eq!(v.blocks[0].children[0].plain_text(), "kid");
    assert_eq!(v.blocks[0].children[1].plain_text(), "");
}

#[test]
fn deleting_a_row_asks_first_and_then_deletes_with_its_children() {
    use figcraft_doc::docs::DocId;
    let (mut s, a, _) = session_with_documents();
    s.execute("doc.create", &json!({"title": "Inner", "parent": a})).unwrap();
    let mut u = documents_tab(s);
    let uid = super::state::uid(&u.app).unwrap();
    let mut st = super::state::take(uid);
    st.confirm_delete = Some(DocId(a));
    super::state::put(uid, st);
    u.frame(vec![]);
    let sh = shapes(&mut u);
    assert!(has_text(&sh, "Delete “Spec”?"));
    assert!(has_text(&sh, "It also deletes the 1 document inside it. You can undo this."));
    // Nothing is deleted until the person says so; Cancel keeps everything.
    assert_eq!(u.app.session.doc().unwrap().docs.len(), 3);
    let cancel = text_center(&sh, "Cancel");
    click(&mut u, cancel);
    assert_eq!(u.app.session.doc().unwrap().docs.len(), 3);
    assert!(!has_text(&shapes(&mut u), "Delete “Spec”?"));
    // Ask again and confirm.
    let mut st = super::state::take(uid);
    st.confirm_delete = Some(DocId(a));
    super::state::put(uid, st);
    u.frame(vec![]);
    let del = text_center(&shapes(&mut u), "Delete");
    click(&mut u, del);
    assert_eq!(titles(&u, None), ["Notes"]);
    assert_eq!(u.app.session.doc().unwrap().docs.len(), 1);
}

#[test]
fn another_command_sends_the_pending_words_first() {
    let (mut s, a, _) = session_with_documents();
    s.execute("doc.block.insert", &json!({"doc": a, "text": "x"})).unwrap();
    let mut u = documents_tab(s);
    super::open(&mut u.app, a);
    for _ in 0..3 {
        u.frame(vec![]);
    }
    let at = text_center(&shapes(&mut u), "x");
    click(&mut u, at);
    u.frame(vec![Event::Text("y".into())]);
    // A save (or any command) right after typing must not lose the last words.
    u.app.run("view.zoom100", json!({}));
    let text = u.app.session.doc().unwrap().docs.get(figcraft_doc::docs::DocId(a)).unwrap().current().unwrap().blocks[0].plain_text();
    assert!(text.contains('y'), "got {text:?}");
}

#[test]
fn the_dots_button_on_a_row_opens_its_menu() {
    // The row's buttons sit on top of it: the pointer going onto them must not make them vanish.
    let (s, _, _) = session_with_documents();
    let mut u = documents_tab(s);
    let row = text_center(&shapes(&mut u), "Spec");
    let dots = egui::Pos2::new(273.0, row.y);
    for p in [row, dots, dots] {
        u.frame(vec![Event::PointerMoved(p)]);
    }
    click(&mut u, dots);
    assert!(has_text(&shapes(&mut u), "Move to the top level") || has_text(&shapes(&mut u), "Add a document inside"), "the menu is open");
}

#[test]
fn putting_a_document_on_the_canvas_makes_a_selected_card_and_shows_the_canvas() {
    use figcraft_doc::docs::DocId;
    let (mut s, a, _) = session_with_documents();
    s.execute("doc.block.insert", &json!({"doc": a, "text": "Hello"})).unwrap();
    let mut u = documents_tab(s);
    super::open(&mut u.app, a);
    u.frame(vec![]);
    let uid = super::state::uid(&u.app).unwrap();
    let mut st = super::state::take(uid);
    super::export::put_on_canvas(&mut u.app, &mut st, DocId(a), None);
    super::state::put(uid, st);
    assert_eq!(super::open_document(&u.app), None, "the editor closes so the card shows");
    let sel = u.app.session.selection();
    assert_eq!(sel.len(), 1);
    let d = u.app.session.doc().unwrap();
    assert_eq!(d.node(sel[0]).unwrap().plugin_data.get("graybox.doc").map(String::as_str), Some(a.to_string().as_str()));
}

#[test]
fn the_editor_has_an_export_menu() {
    let (s, a, _) = session_with_documents();
    let mut u = documents_tab(s);
    super::open(&mut u.app, a);
    u.frame(vec![]);
    // The right panel has an "Export" section too: ours is the one in the editor's top row.
    let at =
        crate::live_tests::texts(&shapes(&mut u)).into_iter().find(|(t, p, _)| t == "Export" && p.y < 100.0).map(|(_, p, s)| p + s / 2.0).unwrap();
    click(&mut u, at);
    let sh = shapes(&mut u);
    assert!(has_text(&sh, "Export as PDF…") && has_text(&sh, "Export as Markdown…") && has_text(&sh, "Put on the canvas"));
}

#[test]
fn file_names_never_hold_characters_a_file_system_refuses() {
    assert_eq!(super::export::file_stem("a/b:c?"), "a-b-c-");
    assert_eq!(super::export::file_stem("  "), "Untitled");
    assert_eq!(super::export::file_stem("..."), "Untitled");
}
