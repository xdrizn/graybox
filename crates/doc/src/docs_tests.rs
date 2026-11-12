//! The document store: tree edits, blocks, closed versions, and reading hostile files.

use serde_json::json;

use crate::Document;
use crate::docs::{BlockId, BlockKind, DocError, DocId, Documents, MAX_BLOCK_DEPTH, MAX_DEPTH, Place, Run, VersionState};

fn three() -> (Documents, DocId, DocId, DocId) {
    let mut d = Documents::default();
    let a = d.create("Alpha", None, None).unwrap();
    let b = d.create("Beta", None, None).unwrap();
    let c = d.create("Gamma", None, None).unwrap();
    (d, a, b, c)
}

fn titles(d: &Documents, parent: Option<DocId>) -> Vec<String> {
    d.siblings(parent).iter().map(|i| d.get(*i).unwrap().title.clone()).collect()
}

#[test]
fn documents_are_created_in_order_and_at_a_place() {
    let (mut d, a, _, _) = three();
    assert_eq!(titles(&d, None), ["Alpha", "Beta", "Gamma"]);
    d.create("First", None, Some(0)).unwrap();
    d.create("Inside", Some(a), None).unwrap();
    assert_eq!(titles(&d, None), ["First", "Alpha", "Beta", "Gamma"]);
    assert_eq!(titles(&d, Some(a)), ["Inside"]);
    // Every document starts with an open first version.
    assert_eq!(d.get(a).unwrap().versions.len(), 1);
    assert_eq!(d.get(a).unwrap().current().unwrap().number, 1);
}

#[test]
fn moving_nests_reorders_and_refuses_cycles() {
    let (mut d, a, b, c) = three();
    d.move_to(b, Some(a), None).unwrap();
    assert_eq!(titles(&d, None), ["Alpha", "Gamma"]);
    assert_eq!(titles(&d, Some(a)), ["Beta"]);
    assert_eq!(d.get(b).unwrap().parent, Some(a));
    assert_eq!(d.depth(b), 2);
    // A document cannot go inside itself or inside what it holds.
    assert_eq!(d.move_to(a, Some(b), None), Err(DocError::Cycle));
    assert_eq!(d.move_to(a, Some(a), None), Err(DocError::Cycle));
    // The index counts the siblings left once it is lifted out.
    d.move_to(c, None, Some(0)).unwrap();
    assert_eq!(titles(&d, None), ["Gamma", "Alpha"]);
    d.move_to(b, None, Some(1)).unwrap();
    assert_eq!(titles(&d, None), ["Gamma", "Beta", "Alpha"]);
    assert!(titles(&d, Some(a)).is_empty());
}

#[test]
fn nesting_has_a_depth_cap() {
    let mut d = Documents::default();
    let mut at = d.create("0", None, None).unwrap();
    for i in 1..MAX_DEPTH {
        at = d.create(&i.to_string(), Some(at), None).unwrap();
    }
    assert_eq!(d.depth(at), MAX_DEPTH);
    assert_eq!(d.create("too deep", Some(at), None), Err(DocError::Limit("levels of documents")));
    // Moving a subtree that would pass the cap is refused too.
    let other = d.create("other", None, None).unwrap();
    let kid = d.create("kid", Some(other), None).unwrap();
    assert!(d.move_to(other, Some(at), None).is_err());
    assert_eq!(d.get(kid).unwrap().parent, Some(other));
}

#[test]
fn deleting_takes_the_documents_inside_too() {
    let (mut d, a, b, c) = three();
    let inner = d.create("Inner", Some(b), None).unwrap();
    let gone = d.delete(b).unwrap();
    assert!(gone.contains(&b) && gone.contains(&inner));
    assert!(d.get(inner).is_none());
    assert_eq!(d.siblings(None), [a, c]);
    assert_eq!(d.delete(b), Err(DocError::NoDoc(b)));
}

#[test]
fn ids_are_never_reused() {
    let (mut d, _, b, _) = three();
    d.delete(b).unwrap();
    let again = d.create("New", None, None).unwrap();
    assert!(again.0 > 3);
}

#[test]
fn titles_and_icons_are_cleaned_and_capped() {
    let (mut d, a, _, _) = three();
    d.rename(a, "Line\nbreak\u{0}").unwrap();
    assert_eq!(d.get(a).unwrap().title, "Linebreak");
    d.rename(a, &"x".repeat(1000)).unwrap();
    assert_eq!(d.get(a).unwrap().title.chars().count(), 200);
    d.set_icon(a, "📄").unwrap();
    assert_eq!(d.get(a).unwrap().icon, "📄");
    assert!(d.set_icon(a, &"y".repeat(40)).is_err());
    d.set_icon(a, "").unwrap();
    assert_eq!(d.get(a).unwrap().icon, "");
}

fn para(d: &mut Documents, doc: DocId, text: &str) -> BlockId {
    d.insert_block(doc, None, Place::default(), BlockKind::Paragraph, vec![Run::plain(text)]).unwrap()
}

#[test]
fn blocks_are_inserted_nested_edited_moved_and_deleted() {
    let (mut d, a, _, _) = three();
    let h = d.insert_block(a, None, Place::default(), BlockKind::Heading { level: 1 }, vec![Run::plain("Title")]).unwrap();
    let p = para(&mut d, a, "Body");
    let first = d.insert_block(a, None, Place { parent: None, index: Some(0) }, BlockKind::Divider, vec![]).unwrap();
    let order = |d: &Documents| d.get(a).unwrap().current().unwrap().blocks.iter().map(|b| b.id).collect::<Vec<_>>();
    assert_eq!(order(&d), [first, h, p]);
    // Inside another block.
    let t = d.insert_block(a, None, Place { parent: Some(p), index: None }, BlockKind::Toggle { open: true }, vec![]).unwrap();
    assert_eq!(d.get(a).unwrap().current().unwrap().block_count(), 4);
    d.update_block(a, None, p, |b| {
        b.set_plain_text("Edited");
        Ok(())
    })
    .unwrap();
    let v = d.get(a).unwrap().current().unwrap();
    assert_eq!(v.block(p).unwrap().plain_text(), "Edited");
    // Editing a block keeps what is inside it.
    assert_eq!(v.block(p).unwrap().children[0].id, t);
    // Move the toggle to the top; its old parent loses it.
    d.move_block(a, None, t, Place { parent: None, index: Some(1) }).unwrap();
    assert_eq!(order(&d), [first, t, h, p]);
    assert!(d.get(a).unwrap().current().unwrap().block(p).unwrap().children.is_empty());
    // A block cannot go inside itself or what it holds.
    d.move_block(a, None, p, Place { parent: Some(t), index: None }).unwrap();
    assert_eq!(d.move_block(a, None, t, Place { parent: Some(p), index: None }), Err(DocError::Cycle));
    assert_eq!(d.move_block(a, None, t, Place { parent: Some(t), index: None }), Err(DocError::Cycle));
    d.delete_block(a, None, t).unwrap();
    // The toggle took the paragraph with it.
    assert_eq!(order(&d), [first, h]);
    assert_eq!(d.delete_block(a, None, t), Err(DocError::NoBlock(t)));
}

#[test]
fn set_plain_text_keeps_the_look_of_a_single_run() {
    let mut b = crate::docs::Block::new(BlockId(1), BlockKind::Paragraph);
    b.text = vec![Run { text: "old".into(), bold: true, ..Run::default() }];
    b.set_plain_text("new");
    assert_eq!(b.text.len(), 1);
    assert!(b.text[0].bold);
    assert_eq!(b.plain_text(), "new");
    b.set_plain_text("");
    assert!(b.text.is_empty());
}

fn rich() -> crate::docs::Block {
    let mut b = crate::docs::Block::new(BlockId(1), BlockKind::Paragraph);
    b.text = vec![
        Run::plain("Goal: "),
        Run { text: "ship".into(), bold: true, ..Run::default() },
        Run { text: " the spec".into(), link: Some("https://example.test".into()), ..Run::default() },
    ];
    b
}

#[test]
fn typing_in_a_block_with_several_runs_keeps_the_looks_of_the_rest() {
    // Letters added inside the bold word stay bold; the link run is untouched.
    let mut b = rich();
    b.set_plain_text("Goal: shipped the spec");
    assert_eq!(b.plain_text(), "Goal: shipped the spec");
    assert!(b.text[1].bold && b.text[1].text == "shipped");
    assert_eq!(b.text[2].link.as_deref(), Some("https://example.test"));
    // Deleting a whole run drops it and keeps the others.
    let mut b = rich();
    b.set_plain_text("Goal:  the spec");
    assert_eq!(b.plain_text(), "Goal:  the spec");
    assert!(b.text.iter().all(|r| !r.bold));
    assert!(b.text.iter().any(|r| r.link.is_some()));
    // Setting the text it already has changes nothing.
    let mut b = rich();
    b.set_plain_text("Goal: ship the spec");
    assert_eq!(b, rich());
    // Replacing everything still works and is one plain run.
    b.set_plain_text("other");
    assert_eq!(b.plain_text(), "other");
}

#[test]
fn words_typed_after_a_mention_do_not_join_it() {
    use crate::NodeId;
    let mention = crate::docs::Mention::Layer { node: NodeId(1) };
    let mut b = crate::docs::Block::new(BlockId(1), BlockKind::Paragraph);
    b.text = vec![Run { text: "@Hero".into(), mention: Some(mention), ..Run::default() }, Run::plain(" is")];
    b.set_plain_text("@Hero is here");
    assert_eq!(b.plain_text(), "@Hero is here");
    assert!(b.text[0].mention.is_some() && b.text[0].text == "@Hero");
    // Typing right after the mention starts a plain run of its own.
    let mut b = crate::docs::Block::new(BlockId(1), BlockKind::Paragraph);
    b.text = vec![Run { text: "@Hero".into(), mention: Some(mention), ..Run::default() }, Run::plain("x")];
    b.set_plain_text("@Heroax");
    assert_eq!(b.plain_text(), "@Heroax");
    assert!(b.text[0].mention.is_some() && b.text[0].text == "@Hero");
    assert!(b.text[1].mention.is_none() && b.text[1].text == "a");
    // Editing the mention's own words makes it ordinary text.
    let mut b = crate::docs::Block::new(BlockId(1), BlockKind::Paragraph);
    b.text = vec![Run { text: "@Hero".into(), mention: Some(mention), ..Run::default() }];
    b.set_plain_text("@Hera");
    assert!(b.text[0].mention.is_none());
}

#[test]
fn locate_says_where_a_block_sits_among_its_siblings() {
    let (mut d, a, _, _) = three();
    let first = para(&mut d, a, "one");
    let second = para(&mut d, a, "two");
    let inner = d.insert_block(a, None, Place { parent: Some(first), index: None }, BlockKind::Bulleted, vec![]).unwrap();
    let inner2 = d.insert_block(a, None, Place { parent: Some(first), index: None }, BlockKind::Bulleted, vec![]).unwrap();
    let v = d.get(a).unwrap().current().unwrap();
    assert_eq!(v.locate(first), Some((None, 0)));
    assert_eq!(v.locate(second), Some((None, 1)));
    assert_eq!(v.locate(inner), Some((Some(first), 0)));
    assert_eq!(v.locate(inner2), Some((Some(first), 1)));
    assert_eq!(v.locate(BlockId(999)), None);
}

#[test]
fn a_block_kind_from_a_newer_build_is_read_not_fatal() {
    let mut d = read(json!({"docs": [
        {"id": 1, "title": "kept", "versions": [{"blocks": [
            {"id": 1, "kind": {"type": "paragraph"}},
            {"id": 2, "kind": {"type": "hologram", "depth": 3}},
            {"kind": {"type": "paragraph"}},
            {"id": 4}
        ]}]},
        {"title": "no id: dropped"}
    ]}));
    assert_eq!(d.len(), 1);
    let blocks = d.get(DocId(1)).unwrap().current().unwrap().blocks.clone();
    assert_eq!(blocks.len(), 4);
    assert_eq!(blocks[1].kind, BlockKind::Unknown);
    // A block without an id gets one; a block without a kind is a paragraph.
    assert!(blocks[2].id.0 > 0 && blocks[2].id != blocks[0].id);
    assert_eq!(blocks[3].kind, BlockKind::Paragraph);
    // The unknown block cannot be made or changed here, but it can be deleted.
    assert_eq!(BlockKind::from_value(&json!({"type": "hologram"})), None);
    assert!(d.update_block(DocId(1), None, blocks[1].id, |_| Ok(())).is_err());
    d.delete_block(DocId(1), None, blocks[1].id).unwrap();
}

#[test]
fn a_closed_version_refuses_every_block_edit() {
    let (mut d, a, _, _) = three();
    let p = para(&mut d, a, "Signed text");
    // Closing is G13.4; the store already honours the state.
    d.update_version_state_for_tests(a, VersionState::Closed);
    assert_eq!(d.insert_block(a, None, Place::default(), BlockKind::Quote, vec![]), Err(DocError::Closed(1)));
    assert_eq!(d.update_block(a, None, p, |_| Ok(())), Err(DocError::Closed(1)));
    assert_eq!(d.move_block(a, None, p, Place::default()), Err(DocError::Closed(1)));
    assert_eq!(d.delete_block(a, None, p), Err(DocError::Closed(1)));
    assert_eq!(d.get(a).unwrap().current().unwrap().block(p).unwrap().plain_text(), "Signed text");
}

#[test]
fn block_data_out_of_range_is_refused() {
    let (mut d, a, _, _) = three();
    let h = d.insert_block(a, None, Place::default(), BlockKind::Heading { level: 2 }, vec![]).unwrap();
    assert!(d.insert_block(a, None, Place::default(), BlockKind::Heading { level: 9 }, vec![]).is_err());
    assert!(d.insert_block(a, None, Place::default(), BlockKind::Swatch { color: "red".into() }, vec![]).is_err());
    assert!(
        d.update_block(a, None, h, |b| {
            b.style.size = Some(f32::NAN);
            Ok(())
        })
        .is_err()
    );
    assert!(
        d.update_block(a, None, h, |b| {
            b.style.color = Some("#12".into());
            Ok(())
        })
        .is_err()
    );
    let big = vec![Run::plain("x".repeat(200_000))];
    assert!(d.insert_block(a, None, Place::default(), BlockKind::Paragraph, big).is_err());
    assert_eq!(d.get(a).unwrap().current().unwrap().block_count(), 1);
}

#[test]
fn blocks_have_a_depth_cap() {
    let (mut d, a, _, _) = three();
    let mut at = None;
    for _ in 0..MAX_BLOCK_DEPTH {
        at = Some(d.insert_block(a, None, Place { parent: at, index: None }, BlockKind::Toggle { open: true }, vec![]).unwrap());
    }
    assert!(d.insert_block(a, None, Place { parent: at, index: None }, BlockKind::Paragraph, vec![]).is_err());
}

#[test]
fn kinds_are_read_from_names_and_objects() {
    assert_eq!(BlockKind::from_value(&json!("heading2")), Some(BlockKind::Heading { level: 2 }));
    assert_eq!(BlockKind::from_value(&json!("To-do")), Some(BlockKind::Checklist { checked: false }));
    assert_eq!(BlockKind::from_value(&json!({"type": "heading", "level": 3})), Some(BlockKind::Heading { level: 3 }));
    assert_eq!(BlockKind::from_value(&json!({"type": "heading", "level": 7})), None);
    assert_eq!(BlockKind::from_value(&json!({"type": "embed", "node": 5, "live": true})).map(|k| k.name()), Some("embed".into()));
    for junk in [json!(null), json!(3), json!([]), json!("nonsense"), json!({"type": "nope"}), json!({"type": "swatch", "color": "red"})] {
        assert_eq!(BlockKind::from_value(&junk), None, "{junk}");
    }
    // Every listed name is a kind that names itself back.
    for name in BlockKind::NAMES {
        let kind = BlockKind::from_name(name).unwrap();
        assert_eq!(kind.name(), name);
    }
}

#[test]
fn a_file_with_documents_round_trips_and_one_without_is_unchanged() {
    let plain = Document::new("No docs");
    let json = serde_json::to_value(&plain).unwrap();
    assert!(json.get("docs").is_none(), "an empty store is not written");
    let again: Document = serde_json::from_value(json).unwrap();
    assert!(again.docs.is_empty());

    let mut doc = Document::new("With docs");
    let a = doc.docs.create("Spec", None, None).unwrap();
    let b = doc.docs.create("Notes", Some(a), None).unwrap();
    doc.docs.set_icon(b, "📝").unwrap();
    let h = doc.docs.insert_block(b, None, Place::default(), BlockKind::Heading { level: 1 }, vec![Run::plain("Hello")]).unwrap();
    let c = doc
        .docs
        .insert_block(
            b,
            None,
            Place::default(),
            BlockKind::Checklist { checked: true },
            vec![Run { text: "done".into(), bold: true, ..Run::default() }],
        )
        .unwrap();
    doc.docs
        .update_block(b, None, c, |blk| {
            blk.style.font = Some("Inter".into());
            blk.style.size = Some(18.0);
            blk.style.color = Some("#FF8800".into());
            Ok(())
        })
        .unwrap();
    let text = serde_json::to_string(&doc).unwrap();
    let back: Document = serde_json::from_str(&text).unwrap();
    assert_eq!(back.docs, doc.docs);
    assert_eq!(back.docs.get(b).unwrap().parent, Some(a));
    assert_eq!(back.docs.get(b).unwrap().current().unwrap().block(h).unwrap().plain_text(), "Hello");
    // Ids keep counting after a reload.
    let mut back = back;
    let next = back.docs.create("Later", None, None).unwrap();
    assert!(next.0 > b.0);
    let nb = back.docs.insert_block(next, None, Place::default(), BlockKind::Paragraph, vec![]).unwrap();
    assert!(nb.0 > c.0);
}

#[test]
fn a_parent_with_a_smaller_id_than_its_child_survives_the_round_trip() {
    // The child (id 1) sits inside the parent (id 2): the file lists ids in tree order, not id order.
    let mut d = Documents::default();
    let child = d.create("child", None, None).unwrap();
    let parent = d.create("parent", None, None).unwrap();
    d.move_to(child, Some(parent), None).unwrap();
    let back: Documents = serde_json::from_value(serde_json::to_value(&d).unwrap()).unwrap();
    assert_eq!(back, d);
    assert_eq!(back.siblings(None), [parent]);
    assert_eq!(back.get(child).unwrap().parent, Some(parent));
}

fn read(v: serde_json::Value) -> Documents {
    serde_json::from_value(v).unwrap()
}

#[test]
fn a_hand_edited_file_is_repaired_on_load() {
    // A cycle, a dangling child, a duplicate document id, duplicate block ids, no versions, numbers out of order.
    let d = read(json!({
        "docs": [
            {"id": 1, "title": "a", "children": [2, 99]},
            {"id": 2, "title": "b", "children": [1]},
            {"id": 2, "title": "dup"},
            {"id": 3, "title": "no versions"},
            {"id": 4, "title": "blocks", "versions": [
                {"number": 7, "blocks": [
                    {"id": 5, "kind": {"type": "paragraph"}},
                    {"id": 5, "kind": {"type": "heading", "level": 40}, "style": {"size": -3.0, "color": "bad"}},
                    {"id": 0, "kind": {"type": "swatch", "color": "nope"}}
                ]}
            ]}
        ],
        "roots": [2, 2, 77]
    }));
    // Every document reachable exactly once.
    let walked = d.walk();
    assert_eq!(walked.len(), d.len());
    let mut sorted = walked.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), walked.len());
    assert_eq!(d.get(DocId(2)).unwrap().title, "b");
    for id in &walked {
        let doc = d.get(*id).unwrap();
        assert!(!doc.versions.is_empty());
        for (i, v) in doc.versions.iter().enumerate() {
            assert_eq!(v.number as usize, i + 1);
        }
        for c in &doc.children {
            assert_eq!(d.get(*c).unwrap().parent, Some(*id));
        }
    }
    // Block ids are unique and the numbers are in range.
    let blocks = &d.get(DocId(4)).unwrap().current().unwrap().blocks;
    let mut ids: Vec<u64> = blocks.iter().map(|b| b.id.0).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), 3);
    assert!(ids.iter().all(|i| *i > 0));
    assert_eq!(blocks[1].kind, BlockKind::Heading { level: 4 });
    assert!(blocks[1].style.size.is_none() && blocks[1].style.color.is_none());
    assert_eq!(blocks[2].kind, BlockKind::Swatch { color: String::new() });
}

#[test]
fn junk_in_the_store_never_panics() {
    for junk in [json!(null), json!(3), json!("x"), json!([]), json!({"docs": 3}), json!({"docs": [null]}), json!({"docs": [{"id": "a"}]})] {
        let _ = serde_json::from_value::<Documents>(junk);
    }
    // Deep nesting in a file is cut at the cap.
    let mut block = json!({"id": 1000, "kind": {"type": "paragraph"}});
    for i in 0..40 {
        block = json!({"id": i + 1, "kind": {"type": "toggle"}, "children": [block]});
    }
    let d = read(json!({"docs": [{"id": 1, "versions": [{"blocks": [block]}]}]}));
    let v = d.get(DocId(1)).unwrap().current().unwrap();
    assert!(v.block_count() <= MAX_BLOCK_DEPTH);
}
