//! Versions, signatures, diff and the decision log (G13.4) through the commands, and what the pages
//! show of them.

use serde_json::{Value, json};

use crate::Session;

fn ex(s: &mut Session, command: &str, params: Value) -> Value {
    s.execute(command, &params).unwrap()
}

fn refused(s: &mut Session, command: &str, params: Value) -> String {
    s.execute(command, &params).unwrap_err().to_string()
}

fn doc_with(s: &mut Session, paragraphs: &[&str]) -> u64 {
    let id = ex(s, "doc.create", json!({"title": "Spec"}))["id"].as_u64().unwrap();
    for p in paragraphs {
        ex(s, "doc.block.insert", json!({"doc": id, "text": p}));
    }
    id
}

fn has_fonts() -> bool {
    figcraft_text::find_face("Inter", "Regular").is_some()
}

/// Every text on the pages of the last version's layout.
fn page_texts(s: &Session, doc: u64) -> Vec<Vec<String>> {
    let d = s.doc().unwrap();
    let doc = d.docs.get(figcraft_doc::docs::DocId(doc)).unwrap();
    let v = doc.current().unwrap();
    let code = crate::docs_render::code_of(v);
    let info = crate::docs_page::Info {
        title: &doc.title,
        number: v.number,
        closed: v.is_closed(),
        code: code.as_deref(),
        fingerprint: v.fingerprint.as_deref(),
        closed_at: v.closed_at.as_deref(),
        for_signature: v.for_signature,
        signers: &v.signers,
        signatures: &v.signatures,
        ..crate::docs_page::Info::default()
    };
    crate::docs_page::layout(&info, &v.blocks).pages.iter().map(|p| p.iter().map(|i| i.text().to_string()).collect()).collect()
}

#[test]
fn closing_records_a_fingerprint_and_a_short_code_and_refuses_edits() {
    let mut s = Session::new();
    let doc = doc_with(&mut s, &["The menu opens with Start."]);
    let r = ex(&mut s, "doc.version.close", json!({"doc": doc}));
    assert_eq!(r["version"], 1);
    let hash = r["fingerprint"].as_str().unwrap();
    assert_eq!(hash.len(), 64);
    let code = r["code"].as_str().unwrap();
    assert!(code.starts_with("GBX-") && code.len() == 13);
    assert!(r["closedAt"].as_str().unwrap().ends_with(" UTC"));
    assert!(refused(&mut s, "doc.block.insert", json!({"doc": doc, "text": "late"})).contains("closed"));
    assert!(refused(&mut s, "doc.version.close", json!({"doc": doc})).contains("closed"));
    let v = &ex(&mut s, "doc.versions", json!({"doc": doc}))[0];
    assert_eq!(v["state"], "closed");
    assert_eq!(v["intact"], true);
    assert_eq!(v["code"], code);
    // One undo opens it again.
    s.undo().unwrap();
    assert_eq!(ex(&mut s, "doc.versions", json!({"doc": doc}))[0]["state"], "draft");
}

#[test]
fn the_same_words_give_the_same_fingerprint() {
    let mut s = Session::new();
    let a = doc_with(&mut s, &["Same words."]);
    let fa = ex(&mut s, "doc.version.close", json!({"doc": a}))["fingerprint"].clone();
    let mut t = Session::new();
    let b = doc_with(&mut t, &["Same words."]);
    assert_eq!(ex(&mut t, "doc.version.close", json!({"doc": b}))["fingerprint"], fa);
    let mut u = Session::new();
    let c = doc_with(&mut u, &["Other words."]);
    assert_ne!(ex(&mut u, "doc.version.close", json!({"doc": c}))["fingerprint"], fa);
}

#[test]
fn a_version_changed_outside_graybox_is_no_longer_intact_and_cannot_be_signed() {
    let mut s = Session::new();
    let doc = doc_with(&mut s, &["Price: 100."]);
    ex(&mut s, "doc.version.close", json!({"doc": doc}));
    // Someone edits the saved file by hand.
    let mut file = serde_json::to_value(&s.doc().unwrap().docs).unwrap();
    file["docs"][0]["versions"][0]["blocks"][0]["text"][0]["text"] = json!("Price: 10.");
    s.doc_mut().unwrap().docs = serde_json::from_value(file).unwrap();
    assert_eq!(ex(&mut s, "doc.versions", json!({"doc": doc}))[0]["intact"], false);
    assert!(refused(&mut s, "doc.version.sign", json!({"doc": doc, "name": "Ana"})).contains("outside Graybox"));
}

#[test]
fn a_new_version_copies_the_last_and_the_diff_shows_what_changed() {
    let mut s = Session::new();
    let doc = doc_with(&mut s, &["Kept.", "Changed.", "Removed."]);
    ex(&mut s, "doc.version.close", json!({"doc": doc}));
    assert_eq!(ex(&mut s, "doc.version.new", json!({"doc": doc}))["version"], 2);
    assert!(refused(&mut s, "doc.version.new", json!({"doc": doc})).contains("draft"));
    let blocks = ex(&mut s, "doc.get", json!({"id": doc}))["blocks"].clone();
    let id_of = |text: &str| blocks.as_array().unwrap().iter().find(|b| b["text"][0]["text"] == text).unwrap()["id"].as_u64().unwrap();
    ex(&mut s, "doc.block.set", json!({"doc": doc, "id": id_of("Changed."), "text": "Changed a lot."}));
    ex(&mut s, "doc.block.delete", json!({"doc": doc, "id": id_of("Removed.")}));
    ex(&mut s, "doc.block.insert", json!({"doc": doc, "text": "Added."}));
    let d = ex(&mut s, "doc.diff", json!({"doc": doc}));
    assert_eq!((d["from"].as_u64(), d["to"].as_u64()), (Some(1), Some(2)));
    assert_eq!((d["added"].as_u64(), d["removed"].as_u64(), d["changed"].as_u64()), (Some(1), Some(1), Some(1)));
    let rows: Vec<(String, String)> = d["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| (b["change"].as_str().unwrap().to_string(), b["text"].as_str().unwrap().to_string()))
        .collect();
    let want = [("same", "Kept."), ("changed", "Changed a lot."), ("removed", "Removed."), ("added", "Added.")];
    assert_eq!(rows, want.map(|(a, b)| (a.to_string(), b.to_string())));
    assert_eq!(d["blocks"][1]["before"], "Changed.");
    // Version 1 is untouched.
    assert_eq!(ex(&mut s, "doc.get", json!({"id": doc, "version": 1}))["blocks"].as_array().unwrap().len(), 3);
}

#[test]
fn for_signature_adds_the_signature_section_with_the_designer_and_the_client() {
    let mut s = Session::new();
    let doc = doc_with(&mut s, &["Scope."]);
    ex(&mut s, "doc.version.for_signature", json!({"doc": doc}));
    ex(
        &mut s,
        "doc.version.signers",
        json!({"doc": doc, "signers": [{"name": "Ana Lima", "role": "Designer"}, {"name": "Bruno", "role": "Producer"}]}),
    );
    let v = &ex(&mut s, "doc.versions", json!({"doc": doc}))[0];
    assert_eq!(v["forSignature"], true);
    assert_eq!(v["signers"][1]["role"], "Producer");
    if has_fonts() {
        let words: Vec<String> = page_texts(&s, doc).concat();
        for w in ["Signatures", "Ana Lima", "Designer", "Bruno", "Producer"] {
            assert!(words.iter().any(|t| t == w), "{w} on the page: {words:?}");
        }
        assert!(words.iter().filter(|t| t.starts_with("Date:")).count() == 2, "a date line per slot");
    }
    // Off: the section goes.
    ex(&mut s, "doc.version.for_signature", json!({"doc": doc, "on": false}));
    assert!(!page_texts(&s, doc).concat().iter().any(|t| t == "Signatures"));
}

#[test]
fn signing_fills_the_slot_and_the_pdf_ends_with_a_certificate_page() {
    let mut s = Session::new();
    let doc = doc_with(&mut s, &["Scope."]);
    ex(&mut s, "doc.version.for_signature", json!({"doc": doc}));
    ex(&mut s, "doc.version.signers", json!({"doc": doc, "signers": [{"name": "Ana Lima", "role": "Designer"}, {"name": "", "role": "Client"}]}));
    assert!(refused(&mut s, "doc.version.sign", json!({"doc": doc, "name": "Ana Lima"})).contains("closed"));
    let code = ex(&mut s, "doc.version.close", json!({"doc": doc}))["code"].as_str().unwrap().to_string();
    let drawn = json!({"type": "drawn", "strokes": [[[0.1, 0.8], [0.3, 0.2], [0.5, 0.7], [0.9, 0.3]]]});
    ex(&mut s, "doc.version.sign", json!({"doc": doc, "name": "Ana Lima", "mark": drawn}));
    assert!(refused(&mut s, "doc.version.sign", json!({"doc": doc, "name": "Ana Lima"})).contains("signed already"));
    let v = &ex(&mut s, "doc.versions", json!({"doc": doc}))[0];
    assert_eq!(v["signatures"][0]["name"], "Ana Lima");
    assert_eq!(v["signatures"][0]["role"], "Designer");
    assert!(v["signatures"][0]["signedAt"].as_str().unwrap().ends_with("UTC"));
    if has_fonts() {
        let pages = page_texts(&s, doc);
        let last: &Vec<String> = pages.last().unwrap();
        assert!(last.iter().any(|t| t == "Signature certificate"), "{last:?}");
        assert!(last.iter().any(|t| t == &code));
        assert!(last.iter().any(|t| t.contains("gov.br")));
        assert!(last.iter().any(|t| t == "Not signed yet"), "the client's slot waits");
        assert!(pages.iter().all(|p| p.iter().any(|t| t == &code)), "the code in every footer");
    }
    // The signature is a drawn mark on the page.
    let d = s.doc().unwrap();
    let docv = d.docs.get(figcraft_doc::docs::DocId(doc)).unwrap().current().unwrap().clone();
    let info =
        crate::docs_page::Info { closed: true, for_signature: true, signers: &docv.signers, signatures: &docv.signatures, ..Default::default() };
    let l = crate::docs_page::layout(&info, &docv.blocks);
    assert!(l.pages.iter().flatten().any(|i| matches!(i, crate::docs_page::Item::Ink { strokes, .. } if strokes.len() == 1)));
}

#[test]
fn the_markdown_of_a_version_for_signature_ends_with_its_signers_and_who_signed() {
    let mut s = Session::new();
    let doc = doc_with(&mut s, &["Scope."]);
    ex(&mut s, "doc.version.for_signature", json!({"doc": doc}));
    ex(&mut s, "doc.version.signers", json!({"doc": doc, "signers": [{"name": "Ana", "role": "Designer"}, {"name": "", "role": "Client"}]}));
    ex(&mut s, "doc.version.close", json!({"doc": doc}));
    ex(&mut s, "doc.version.sign", json!({"doc": doc, "name": "Ana"}));
    // The signing time is now; pin it to a known one.
    let mut file = serde_json::to_value(&s.doc().unwrap().docs).unwrap();
    file["docs"][0]["versions"][0]["signatures"][0]["signed_at"] = json!(1_700_000_000u64);
    s.doc_mut().unwrap().docs = serde_json::from_value(file).unwrap();
    let out = ex(&mut s, "doc.export", json!({"doc": doc, "format": "md"}))["text"].as_str().unwrap().to_string();
    assert!(out.ends_with("## Signatures\n\n- Ana, Designer: signed 2023-11-14 22:13 UTC\n- Client: ______________________\n"), "{out}");
}

#[test]
fn a_typed_signature_uses_a_handwriting_font_or_a_clean_italic() {
    for style in 0..3 {
        let (family, real) = crate::docs_page::hand_font(style);
        assert!(!family.is_empty());
        if !real {
            assert_eq!(family, "Inter", "the fallback is the body font in italic");
        }
    }
}

#[test]
fn the_decision_log_lists_every_decision_with_where_it_is() {
    let mut s = Session::new();
    let a = doc_with(&mut s, &["Intro."]);
    let b = ex(&mut s, "doc.create", json!({"title": "HUD"}))["id"].as_u64().unwrap();
    ex(
        &mut s,
        "doc.block.insert",
        json!({"doc": a, "kind": {"type": "decision", "who": "Ana", "when": "2026-11-13", "why": "cheaper"}, "text": "Use 9-slice"}),
    );
    ex(&mut s, "doc.block.insert", json!({"doc": b, "kind": "decision", "text": "Health on the left"}));
    let log = ex(&mut s, "doc.decisions", json!({}));
    let log = log.as_array().unwrap();
    assert_eq!(log.len(), 2);
    assert_eq!(log[0]["text"], "Use 9-slice");
    assert_eq!(log[0]["who"], "Ana");
    assert_eq!(log[0]["doc"].as_u64(), Some(a));
    assert_eq!(log[1]["title"], "HUD");
}

#[test]
fn junk_parameters_are_refused_not_panicked_on() {
    let mut s = Session::new();
    let doc = doc_with(&mut s, &["x"]);
    for (cmd, p) in [
        ("doc.version.close", json!({"doc": "x"})),
        ("doc.version.new", json!({"doc": -1})),
        ("doc.version.for_signature", json!({"doc": doc, "on": "yes"})),
        ("doc.version.signers", json!({"doc": doc, "signers": "Ana"})),
        ("doc.version.signers", json!({"doc": doc, "signers": vec![json!({}); 50]})),
        ("doc.version.sign", json!({"doc": doc, "name": 5})),
        ("doc.version.sign", json!({"doc": doc, "name": "A", "mark": {"type": "drawn", "strokes": "x"}})),
        ("doc.diff", json!({"doc": doc, "from": 99})),
        ("doc.versions", json!({"doc": 99})),
    ] {
        assert!(s.execute(cmd, &p).is_err(), "{cmd} {p}");
    }
    // A drawn mark far out of range is kept inside its box, not refused.
    ex(&mut s, "doc.version.close", json!({"doc": doc}));
    ex(&mut s, "doc.version.sign", json!({"doc": doc, "name": "A", "mark": {"type": "drawn", "strokes": [[[1e30, -5.0], [0.5, 0.5]]]}}));
}
