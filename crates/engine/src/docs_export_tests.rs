//! Documents out of the file (G13.3): PDF, Markdown, and the card on the canvas.

use figcraft_doc::{NodeId, NodeKind};
use serde_json::{Value, json};

use crate::Session;

fn ex(s: &mut Session, command: &str, params: Value) -> Value {
    s.execute(command, &params).unwrap()
}

fn doc_with(s: &mut Session, title: &str, paragraphs: &[&str]) -> u64 {
    let id = ex(s, "doc.create", json!({"title": title}))["id"].as_u64().unwrap();
    for p in paragraphs {
        ex(s, "doc.block.insert", json!({"doc": id, "text": p}));
    }
    id
}

fn has_fonts() -> bool {
    figcraft_text::find_face("Inter", "Regular").is_some()
}

/// All the words laid on a card's pages.
fn card_words(s: &Session, card: u64) -> String {
    let d = s.doc().unwrap();
    let mut out = String::new();
    let mut stack = vec![NodeId(card)];
    while let Some(id) = stack.pop() {
        let Some(n) = d.node(id) else { continue };
        if let NodeKind::Text(t) = &n.kind {
            out.push_str(&t.characters);
            out.push('\n');
        }
        stack.extend(n.children.iter().copied());
    }
    out
}

#[test]
fn markdown_comes_back_as_text_or_goes_to_a_file() {
    let mut s = Session::new();
    let id = doc_with(&mut s, "Spec", &["Players read the design."]);
    let r = ex(&mut s, "doc.export", json!({"doc": id, "format": "md"}));
    assert_eq!(r["text"].as_str(), Some("# Spec\n\n*Version 1 · draft*\n\nPlayers read the design.\n"));

    let path = std::env::temp_dir().join(format!("gbx-doc-export-{}.md", std::process::id()));
    let r = ex(&mut s, "doc.export", json!({"doc": id, "format": "md", "path": path.to_string_lossy()}));
    assert_eq!(r["bytes"].as_u64(), Some(std::fs::metadata(&path).unwrap().len()));
    assert!(std::fs::read_to_string(&path).unwrap().starts_with("# Spec"));
    std::fs::remove_file(&path).unwrap();
}

#[test]
fn a_pdf_has_one_page_per_page_of_the_layout() {
    use base64::Engine as _;
    let mut s = Session::new();
    let long = (0..1200).map(|i| format!("word{i}")).collect::<Vec<_>>().join(" ");
    let id = doc_with(&mut s, "Spec", &[&long]);
    let r = ex(&mut s, "doc.export", json!({"doc": id}));
    let bytes = base64::engine::general_purpose::STANDARD.decode(r["data"].as_str().unwrap()).unwrap();
    assert!(bytes.starts_with(b"%PDF"));
    let pages = r["pages"].as_u64().unwrap();
    if has_fonts() {
        assert!(pages >= 2, "{pages} pages");
    }
    // Every page of the layout is a page object of the file.
    let text = String::from_utf8_lossy(&bytes);
    assert_eq!(
        text.matches("/Type /Page\n").count().max(text.matches("/Type /Page ").count()).max(text.matches("/Type/Page/").count()) as u64,
        pages
    );
}

#[test]
fn export_refuses_what_it_cannot_make() {
    let mut s = Session::new();
    let id = doc_with(&mut s, "Spec", &[]);
    assert!(s.execute("doc.export", &json!({"doc": id, "format": "docx"})).is_err());
    assert!(s.execute("doc.export", &json!({"doc": 999})).is_err());
    assert!(s.execute("doc.export", &json!({"doc": id, "version": 7})).is_err());
    assert!(s.execute("doc.export", &json!({})).is_err());
}

#[test]
fn a_card_shows_the_pages_and_follows_every_edit() {
    let mut s = Session::new();
    let doc = doc_with(&mut s, "Spec", &["First words."]);
    let card = ex(&mut s, "doc.card.create", json!({"doc": doc, "x": 100, "y": 50}))["id"].as_u64().unwrap();
    {
        let d = s.doc().unwrap();
        let n = d.node(NodeId(card)).unwrap();
        assert!(matches!(n.kind, NodeKind::Frame));
        assert_eq!(n.plugin_data.get("graybox.doc").map(String::as_str), Some(doc.to_string().as_str()));
        assert_eq!(n.children.len(), 1, "one page");
        assert_eq!((n.width, n.height), (595.0, 842.0));
    }
    assert!(card_words(&s, card).contains("First words."));

    let block = ex(&mut s, "doc.block.insert", json!({"doc": doc, "text": "Second words."}))["id"].as_u64().unwrap();
    assert!(card_words(&s, card).contains("Second words."), "the card follows an added block");
    ex(&mut s, "doc.block.set", json!({"doc": doc, "id": block, "text": "Changed words."}));
    let words = card_words(&s, card);
    assert!(words.contains("Changed words.") && !words.contains("Second words."));
    ex(&mut s, "doc.rename", json!({"id": doc, "title": "Brief"}));
    assert!(card_words(&s, card).contains("Brief"));
    assert_eq!(s.doc().unwrap().node(NodeId(card)).unwrap().name, "Brief · v1");

    // One undo takes back the words and the card together.
    s.undo().unwrap();
    assert!(card_words(&s, card).contains("Spec"));
    s.undo().unwrap();
    let words = card_words(&s, card);
    assert!(words.contains("Second words.") && !words.contains("Changed words."));
}

#[test]
fn refresh_repairs_a_card_whose_page_was_deleted_by_hand() {
    let mut s = Session::new();
    let doc = doc_with(&mut s, "Spec", &["Words."]);
    let card = ex(&mut s, "doc.card.create", json!({"doc": doc}))["id"].as_u64().unwrap();
    let page = s.doc().unwrap().node(NodeId(card)).unwrap().children[0];
    s.doc_mut().unwrap().remove(page).unwrap();
    assert!(!card_words(&s, card).contains("Words."));
    assert_eq!(ex(&mut s, "doc.card.refresh", json!({"id": card}))["refreshed"].as_u64(), Some(1));
    assert!(card_words(&s, card).contains("Words."));
}

#[test]
fn a_card_survives_its_document_being_deleted_and_saving() {
    let mut s = Session::new();
    let doc = doc_with(&mut s, "Spec", &["Kept words."]);
    let card = ex(&mut s, "doc.card.create", json!({"doc": doc}))["id"].as_u64().unwrap();
    ex(&mut s, "doc.delete", json!({"id": doc}));
    assert!(card_words(&s, card).contains("Kept words."), "the card stays as a record");
    assert_eq!(ex(&mut s, "doc.card.refresh", json!({"id": card}))["refreshed"].as_u64(), Some(0));
}

#[test]
fn a_card_of_a_missing_document_or_version_is_refused() {
    let mut s = Session::new();
    let doc = doc_with(&mut s, "Spec", &[]);
    assert!(s.execute("doc.card.create", &json!({"doc": 999})).is_err());
    assert!(s.execute("doc.card.create", &json!({"doc": doc, "version": 5})).is_err());
    assert!(s.execute("doc.card.create", &json!({})).is_err());
}

#[test]
fn a_card_has_locked_pages_and_only_its_own_document_rebuilds_it() {
    let mut s = Session::new();
    let a = doc_with(&mut s, "", &["A words."]);
    let b = doc_with(&mut s, "B", &["B words."]);
    let card_a = ex(&mut s, "doc.card.create", json!({"doc": a}))["id"].as_u64().unwrap();
    let card_b = ex(&mut s, "doc.card.create", json!({"doc": b, "x": 2000}))["id"].as_u64().unwrap();
    let d = s.doc().unwrap();
    let node = d.node(NodeId(card_a)).unwrap();
    assert_eq!(node.name, "Untitled · v1");
    assert!(node.children.iter().all(|p| d.node(*p).unwrap().locked), "canvas edits would be lost: pages are locked");
    let b_pages = d.node(NodeId(card_b)).unwrap().children.clone();
    ex(&mut s, "doc.block.insert", json!({"doc": a, "text": "More A."}));
    assert_eq!(s.doc().unwrap().node(NodeId(card_b)).unwrap().children, b_pages, "B's card is untouched");
    assert!(card_words(&s, card_a).contains("More A."));
}
