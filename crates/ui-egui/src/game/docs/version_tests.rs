//! Versions and signatures in the editor (G13.4, G13.7), driven with raw input like a person would.

use egui::{Event, Modifiers, PointerButton, Pos2, vec2};
use figcraft_doc::docs::DocId;
use figcraft_doc::docs_version::Mark;
use figcraft_engine::Session;
use serde_json::json;

use crate::LeftTab;
use crate::layers_drag_tests::Ui;
use crate::live_tests::{flat, has_text, text_center};

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

fn click_text(u: &mut Ui, s: &str) {
    let at = text_center(&shapes(u), s);
    click(u, at);
}

/// A document "Deal" with one paragraph, open in the editor.
fn open_deal(setup: impl FnOnce(&mut Session, u64)) -> (Ui, u64) {
    let mut s = Session::new();
    let doc = s.execute("doc.create", &json!({"title": "Deal"})).unwrap()["id"].as_u64().unwrap();
    s.execute("doc.block.insert", &json!({"doc": doc, "text": "Scope."})).unwrap();
    setup(&mut s, doc);
    let mut u = Ui::new(s);
    u.app.ui.left_tab = LeftTab::Documents;
    u.app.ui.open_left_panel();
    super::open(&mut u.app, doc);
    for _ in 0..4 {
        u.frame(vec![]);
    }
    (u, doc)
}

fn version(u: &Ui, doc: u64) -> figcraft_doc::docs::DocVersion {
    u.app.session.doc().unwrap().docs.get(DocId(doc)).unwrap().current().unwrap().clone()
}

#[test]
fn closing_asks_first_then_the_version_shows_its_code_and_offers_a_new_version() {
    let (mut u, doc) = open_deal(|_, _| {});
    assert!(has_text(&shapes(&mut u), "Version 1 · draft"));
    click_text(&mut u, "Close version…");
    assert!(has_text(&shapes(&mut u), "Close version 1?"), "it asks first");
    assert!(!version(&u, doc).is_closed());
    click_text(&mut u, "Close version");
    let v = version(&u, doc);
    assert!(v.is_closed());
    let sh = shapes(&mut u);
    let code = figcraft_engine::doc_versions::code_of(&v).unwrap();
    assert!(has_text(&sh, &code), "the code shows");
    assert!(has_text(&sh, "This version is closed"));
    // New version: a draft to write in again.
    click_text(&mut u, "New version");
    assert_eq!(version(&u, doc).number, 2);
    assert!(has_text(&shapes(&mut u), "Version 2 · draft"));
}

#[test]
fn the_for_signature_switch_adds_the_designer_and_client_slots() {
    let (mut u, doc) = open_deal(|_, _| {});
    assert!(!has_text(&shapes(&mut u), "Signatures"));
    click_text(&mut u, "For signature");
    assert!(version(&u, doc).for_signature);
    let sh = shapes(&mut u);
    assert!(has_text(&sh, "Signatures"));
    assert!(has_text(&sh, "Designer") && has_text(&sh, "Client"));
    assert!(has_text(&sh, "Signed once the version is closed"));
}

fn for_signature_and_closed(s: &mut Session, doc: u64) {
    s.execute("doc.version.for_signature", &json!({"doc": doc})).unwrap();
    s.execute("doc.version.signers", &json!({"doc": doc, "signers": [{"name": "Ana Lima", "role": "Designer"}, {"name": "", "role": "Client"}]}))
        .unwrap();
    s.execute("doc.version.close", &json!({"doc": doc})).unwrap();
}

#[test]
fn signing_with_the_name_typed_fills_the_designer_slot() {
    let (mut u, doc) = open_deal(for_signature_and_closed);
    // Only the slots still free can be signed: both, here.
    click_text(&mut u, "Sign…");
    let sh = shapes(&mut u);
    assert!(has_text(&sh, "Sign version 1 as Designer"));
    click_text(&mut u, "Sign");
    let v = version(&u, doc);
    assert_eq!(v.signatures.len(), 1);
    let g = &v.signatures[0];
    assert_eq!((g.slot, g.name.as_str(), g.role.as_str()), (0, "Ana Lima", "Designer"));
    assert!(matches!(&g.mark, Mark::Typed { text, style: 0 } if text == "Ana Lima"));
    assert_eq!(g.fingerprint, v.fingerprint.clone().unwrap());
    assert!(!has_text(&shapes(&mut u), "Sign version 1 as Designer"), "the dialog closed");
}

#[test]
fn signing_by_drawing_records_the_strokes() {
    let (mut u, doc) = open_deal(for_signature_and_closed);
    click_text(&mut u, "Sign…");
    click_text(&mut u, "Draw");
    let sh = shapes(&mut u);
    let middle = text_center(&sh, "Sign here with the mouse or a pen");
    // One stroke across the pad.
    let from = middle - vec2(120.0, 10.0);
    press(&mut u, from, true);
    for k in 1..=8 {
        let p = from + vec2(30.0 * k as f32, if k % 2 == 0 { -14.0 } else { 14.0 });
        u.frame(vec![Event::PointerMoved(p)]);
    }
    press(&mut u, from + vec2(240.0, 0.0), false);
    u.frame(vec![]);
    assert!(!has_text(&shapes(&mut u), "Sign here with the mouse or a pen"), "the hint goes once something is drawn");
    click_text(&mut u, "Sign");
    let v = version(&u, doc);
    let Some(Mark::Drawn { strokes }) = v.signatures.first().map(|g| g.mark.clone()) else { panic!("a drawn mark: {:?}", v.signatures) };
    assert_eq!(strokes.len(), 1);
    assert!(strokes[0].len() >= 5, "{strokes:?}");
    assert!(strokes[0].iter().all(|q| (0.0..=1.0).contains(&q[0]) && (0.0..=1.0).contains(&q[1])));
}

#[test]
fn a_version_changed_outside_graybox_says_so_and_offers_no_signing() {
    let (mut u, doc) = open_deal(for_signature_and_closed);
    let mut file = serde_json::to_value(&u.app.session.doc().unwrap().docs).unwrap();
    file["docs"][0]["versions"][0]["blocks"][0]["text"][0]["text"] = json!("Scope, and more.");
    u.app.session.doc_mut().unwrap().docs = serde_json::from_value(file).unwrap();
    assert!(version(&u, doc).is_closed());
    // It is checked again within a second.
    u.advance_clock(1.5);
    u.frame(vec![]);
    let sh = shapes(&mut u);
    assert!(has_text(&sh, "Changed outside Graybox"));
    assert!(!has_text(&sh, "Sign…"));
}

#[test]
fn the_version_menu_compares_with_the_version_before() {
    let (mut u, doc) = open_deal(|s, doc| {
        s.execute("doc.version.close", &json!({"doc": doc})).unwrap();
        s.execute("doc.version.new", &json!({"doc": doc})).unwrap();
        s.execute("doc.block.insert", &json!({"doc": doc, "text": "Price: 100."})).unwrap();
    });
    assert_eq!(version(&u, doc).number, 2);
    click_text(&mut u, "Version 2 · draft");
    click_text(&mut u, "Compare with the version before");
    let sh = shapes(&mut u);
    assert!(has_text(&sh, "Version 2 against version 1"));
    assert!(has_text(&sh, "1 added · 0 removed · 0 changed"));
    assert!(has_text(&sh, "ADDED") && has_text(&sh, "Price: 100."));
    click_text(&mut u, "Back to the document");
    assert!(!has_text(&shapes(&mut u), "Version 2 against version 1"));
}

#[test]
fn an_earlier_version_can_be_read_but_not_written() {
    let (mut u, doc) = open_deal(|s, doc| {
        s.execute("doc.version.close", &json!({"doc": doc})).unwrap();
        s.execute("doc.version.new", &json!({"doc": doc})).unwrap();
    });
    click_text(&mut u, "Version 2 · draft");
    let v1 = u.app.session.doc().unwrap().docs.get(DocId(doc)).unwrap().version(1).unwrap().clone();
    let code = figcraft_engine::doc_versions::code_of(&v1).unwrap();
    // Menu rows of a check list start with the room for the check mark.
    click_text(&mut u, &format!("    Version 1 · closed · {code}"));
    let sh = shapes(&mut u);
    assert!(has_text(&sh, "An earlier version"));
    assert!(!has_text(&sh, "Close version…"));
    click_text(&mut u, "Go to version 2");
    assert!(has_text(&shapes(&mut u), "Close version…"));
    assert_eq!(version(&u, doc).number, 2);
}
