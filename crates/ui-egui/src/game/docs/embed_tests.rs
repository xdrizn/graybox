//! Layers shown in documents (G13.2): `/` "Frame or component" asks for a layer from a list, the
//! layer shows live, the handle menu pins it, and a deleted layer says so.

use egui::{Key, Shape};
use figcraft_doc::docs::BlockKind;
use figcraft_engine::Session;
use serde_json::json;

use super::blocks_tests::{blocks, click, key, shapes, type_text};
use crate::LeftTab;
use crate::layers_drag_tests::Ui;
use crate::live_tests::{has_text, text_center};

/// A file with a frame named `HUD` and a document with one empty paragraph, open in the editor.
fn with_frame() -> (Ui, u64, u64) {
    let mut s = Session::new();
    let frame = s.execute("frame.create", &json!({"x": 0, "y": 0, "width": 320, "height": 180, "name": "HUD"})).unwrap()["id"].as_u64().unwrap();
    let doc = s.execute("doc.create", &json!({"title": "Notes"})).unwrap()["id"].as_u64().unwrap();
    s.execute("doc.block.insert", &json!({"doc": doc, "kind": "paragraph"})).unwrap();
    let mut u = Ui::new(s);
    u.app.ui.left_tab = LeftTab::Documents;
    u.app.ui.open_left_panel();
    super::open(&mut u.app, doc);
    for _ in 0..3 {
        u.frame(vec![]);
    }
    (u, doc, frame)
}

/// The first block's layer, whether it is live, and its pinned picture.
fn embed(u: &Ui, doc: u64) -> (u64, bool, Option<String>) {
    match blocks(u, doc).into_iter().next().map(|b| b.kind) {
        Some(BlockKind::Embed { node, live, snapshot }) => (node.0, live, snapshot),
        k => panic!("not an embed: {k:?}"),
    }
}

fn pictures(s: &[Shape]) -> usize {
    s.iter().filter(|s| matches!(s, Shape::Mesh(m) if m.texture_id != egui::TextureId::default())).count()
}

#[test]
fn slash_frame_asks_for_a_layer_from_a_list_and_shows_it_live() {
    let (mut u, doc, frame) = with_frame();
    let at = text_center(&shapes(&mut u), "Type “/” for blocks");
    click(&mut u, at);
    type_text(&mut u, "/frame");
    key(&mut u, Key::Enter);
    assert_eq!(embed(&u, doc).0, 0, "no layer yet");
    u.frame(vec![]);
    let s = shapes(&mut u);
    assert!(has_text(&s, "Show a frame or component of this file"));
    let item = text_center(&s, "HUD");
    click(&mut u, item);
    assert_eq!(embed(&u, doc), (frame, true, None));
    let s = shapes(&mut u);
    assert_eq!(pictures(&s), 1, "the layer is drawn");
    assert!(has_text(&s, "Live") && has_text(&s, "HUD"));
}

#[test]
fn the_handle_menu_pins_the_layer_and_a_deleted_layer_says_so() {
    let (mut u, doc, frame) = with_frame();
    let para = blocks(&u, doc)[0].id.0;
    u.app.run("doc.block.delete", json!({"doc": doc, "id": para}));
    u.app.run("doc.block.embed", json!({"doc": doc, "node": frame}));
    let s = shapes(&mut u);
    let at = text_center(&s, "Live");
    u.frame(vec![egui::Event::PointerMoved(at)]);
    u.frame(vec![]);
    let handle = super::blocks::last_handle().expect("a handle");
    click(&mut u, handle.center());
    let item = text_center(&shapes(&mut u), "✓ Follow the layer");
    click(&mut u, item);
    let (node, live, shot) = embed(&u, doc);
    assert_eq!((node, live), (frame, false));
    assert!(shot.is_some(), "pinned as it is now");
    let s = shapes(&mut u);
    assert!(has_text(&s, "Pinned") && pictures(&s) == 1);
    // Following again, then the layer is deleted.
    u.app.run("doc.block.embed", json!({"doc": doc, "id": embed_id(&u, doc), "live": true}));
    u.app.run("edit.delete", json!({"ids": [frame]}));
    let s = shapes(&mut u);
    assert!(has_text(&s, "This layer was deleted"));
    assert!(has_text(&s, "Choose a layer"));
}

fn embed_id(u: &Ui, doc: u64) -> u64 {
    blocks(u, doc)[0].id.0
}
