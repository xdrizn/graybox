//! Closing, new versions, signers and signatures (G13.4) in the model.

use crate::docs::{BlockKind, DocError, DocId, Documents, Place, Run};
use crate::docs_version::{Mark, Signature, Signer, canonical, short_code};

fn doc_with_text(d: &mut Documents, words: &str) -> DocId {
    let id = d.create("Spec", None, None).unwrap();
    d.insert_block(id, None, Place::default(), BlockKind::Paragraph, vec![Run::plain(words.to_string())]).unwrap();
    id
}

fn typed(slot: usize, name: &str) -> Signature {
    Signature {
        slot,
        name: name.into(),
        mark: Mark::Typed { text: name.into(), style: 1 },
        signed_at: 1_700_000_000,
        via: "graybox".into(),
        ..Signature::default()
    }
}

#[test]
fn a_closed_version_refuses_edits_and_a_new_version_copies_it_with_the_same_block_ids() {
    let mut d = Documents::default();
    let id = doc_with_text(&mut d, "First words.");
    assert_eq!(d.close_version(id, "ab12", "2026-11-13 10:00 UTC").unwrap(), 1);
    assert!(matches!(d.close_version(id, "x", "y"), Err(DocError::Closed(1))));
    let v1 = d.get(id).unwrap().version(1).unwrap().clone();
    assert!(v1.is_closed());
    assert_eq!(v1.fingerprint.as_deref(), Some("ab12"));
    assert!(d.insert_block(id, None, Place::default(), BlockKind::Paragraph, vec![]).is_err());
    assert_eq!(d.new_version(id).unwrap(), 2);
    let v2 = d.get(id).unwrap().version(2).unwrap();
    assert!(!v2.is_closed());
    assert_eq!(v2.blocks, v1.blocks, "a copy, ids and all");
    // A second draft waits for this one to be closed.
    assert!(d.new_version(id).is_err());
}

#[test]
fn the_fingerprint_covers_blocks_and_signers_but_not_signatures() {
    let mut d = Documents::default();
    let id = doc_with_text(&mut d, "Words.");
    d.set_for_signature(id, None, true).unwrap();
    let before = canonical(d.get(id).unwrap().current().unwrap());
    d.set_signers(id, None, vec![Signer { name: "Ana".into(), role: "Designer".into(), email: String::new() }]).unwrap();
    let after = canonical(d.get(id).unwrap().current().unwrap());
    assert_ne!(before, after);
    d.close_version(id, "ff", "t").unwrap();
    d.sign(id, None, typed(0, "Ana")).unwrap();
    assert_eq!(canonical(d.get(id).unwrap().current().unwrap()), after, "signing changes nothing it covers");
}

#[test]
fn for_signature_gives_the_designer_and_the_client_slots() {
    let mut d = Documents::default();
    let id = doc_with_text(&mut d, "Words.");
    d.set_for_signature(id, None, true).unwrap();
    let v = d.get(id).unwrap().current().unwrap();
    assert!(v.for_signature);
    assert_eq!(v.signers.iter().map(|s| s.role.as_str()).collect::<Vec<_>>(), ["Designer", "Client"]);
    // Off and on again keeps the names typed in.
    d.set_signers(id, None, vec![Signer { name: "Ana".into(), role: "Designer".into(), email: String::new() }]).unwrap();
    d.set_for_signature(id, None, false).unwrap();
    d.set_for_signature(id, None, true).unwrap();
    assert_eq!(d.get(id).unwrap().current().unwrap().signers[0].name, "Ana");
}

#[test]
fn signing_needs_a_closed_version_a_free_slot_a_name_and_a_mark() {
    let mut d = Documents::default();
    let id = doc_with_text(&mut d, "Words.");
    d.set_for_signature(id, None, true).unwrap();
    assert!(d.sign(id, None, typed(0, "Ana")).is_err(), "a draft");
    d.close_version(id, "c0ffee", "t").unwrap();
    assert!(d.sign(id, None, typed(5, "Ana")).is_err(), "no such slot");
    assert!(d.sign(id, None, typed(0, "")).is_err(), "no name");
    let blank = Signature { mark: Mark::Drawn { strokes: vec![] }, ..typed(0, "Ana") };
    assert!(d.sign(id, None, blank).is_err(), "no mark");
    d.sign(id, None, typed(0, "Ana")).unwrap();
    assert!(d.sign(id, None, typed(0, "Bea")).is_err(), "signed already");
    let v = d.get(id).unwrap().current().unwrap();
    assert_eq!(v.signatures.len(), 1);
    assert_eq!(v.signatures[0].fingerprint, "c0ffee", "it records what it agreed to");
}

#[test]
fn drawn_marks_and_signers_from_a_hand_edited_file_are_kept_in_range() {
    let mut m = Mark::Drawn { strokes: vec![vec![[f32::NAN, 0.5], [2.0, -1.0]], vec![]] };
    m.sanitize();
    assert_eq!(m, Mark::Drawn { strokes: vec![vec![[1.0, 0.0]]] });
    let mut big = Mark::Drawn { strokes: vec![vec![[0.5, 0.5]; 30_000]] };
    big.sanitize();
    assert!(matches!(&big, Mark::Drawn { strokes } if strokes[0].len() == crate::docs_version::MAX_POINTS));
    let mut t = Mark::Typed { text: "A\u{0}na".into(), style: 9 };
    t.sanitize();
    assert_eq!(t, Mark::Typed { text: "Ana".into(), style: 2 });
}

#[test]
fn a_file_with_signatures_on_a_draft_or_twice_in_a_slot_is_repaired() {
    let d: Documents = serde_json::from_value(serde_json::json!({
        "docs": [{"id": 1, "title": "a", "versions": [
            {"number": 1, "state": "closed", "fingerprint": "ab", "signers": [{"name": "A"}],
             "signatures": [{"slot": 0, "name": "A", "mark": {"type": "typed", "text": "A"}}, {"slot": 0, "name": "B"}, {"slot": 9, "name": "C"}]},
            {"number": 2, "signatures": [{"slot": 0, "name": "D"}]}
        ]}]
    }))
    .unwrap();
    let doc = d.get(DocId(1)).unwrap();
    assert_eq!(doc.versions[0].signatures.len(), 1);
    assert!(doc.versions[1].signatures.is_empty(), "a draft has no signatures");
}

#[test]
fn block_ids_may_repeat_across_versions_after_loading() {
    let mut d = Documents::default();
    let id = doc_with_text(&mut d, "Words.");
    d.close_version(id, "ab", "t").unwrap();
    d.new_version(id).unwrap();
    let json = serde_json::to_value(&d).unwrap();
    let back: Documents = serde_json::from_value(json).unwrap();
    let doc = back.get(id).unwrap();
    assert_eq!(doc.versions[0].blocks[0].id, doc.versions[1].blocks[0].id);
}

#[test]
fn the_short_code_reads_like_gbx_and_two_groups_of_four() {
    let code = short_code("7f3a9c00112233445566778899aabbccddeeff00112233445566778899aabbcc").unwrap();
    assert_eq!(code.len(), 13);
    assert!(code.starts_with("GBX-") && code.as_bytes()[8] == b'-');
    assert!(!code.contains(['I', 'L', 'O', 'U']));
    // Worked by hand: 0x7f3a9c0011 = 01111 11100 11101 01001 11000 00000 00000 10001.
    assert_eq!(code, "GBX-FWX9-R00H");
    assert_eq!(short_code("zz"), None);
}

#[test]
fn what_the_fingerprint_covers_never_changes_shape() {
    // A closed version's fingerprint is checked every time the file opens: if the text below
    // changed, every version closed by an older Graybox would read "changed outside Graybox".
    // A field added to blocks or signers must be left out when it has its default value.
    let mut d = Documents::default();
    let id = doc_with_text(&mut d, "Words.");
    d.set_for_signature(id, None, true).unwrap();
    let v = d.get(id).unwrap().current().unwrap();
    let pinned = r#"{"blocks":[{"id":1,"kind":{"type":"paragraph"},"text":[{"text":"Words."}]}],"forSignature":true,"signers":[{"name":"","role":"Designer"},{"name":"","role":"Client"}]}"#;
    assert_eq!(canonical(v), pinned);
}
