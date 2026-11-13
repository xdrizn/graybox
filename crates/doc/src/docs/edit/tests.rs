//! Splitting, merging and duplicating blocks, as Enter, Backspace and Duplicate do it.

use crate::docs::{Block, BlockId, BlockKind, DocId, Documents, Place, Run};

fn doc_with(kinds: &[(&str, &str)]) -> (Documents, DocId, Vec<BlockId>) {
    let mut d = Documents::default();
    let doc = d.create("Spec", None, None).unwrap();
    let ids = kinds
        .iter()
        .map(|(kind, text)| {
            let runs = if text.is_empty() { vec![] } else { vec![Run::plain(*text)] };
            d.insert_block(doc, None, Place::default(), BlockKind::from_name(kind).unwrap(), runs).unwrap()
        })
        .collect();
    (d, doc, ids)
}

fn blocks(d: &Documents, doc: DocId) -> Vec<Block> {
    d.get(doc).unwrap().current().unwrap().blocks.clone()
}

fn texts(d: &Documents, doc: DocId) -> Vec<String> {
    blocks(d, doc).iter().map(Block::plain_text).collect()
}

#[test]
fn splitting_in_the_middle_leaves_the_start_and_puts_the_rest_in_a_new_block_after() {
    let (mut d, doc, ids) = doc_with(&[("paragraph", "Hello world")]);
    let s = d.split_block(doc, None, ids[0], 5).unwrap();
    assert_eq!(texts(&d, doc), ["Hello", " world"]);
    assert_eq!(s.focus, s.new);
    assert_eq!(blocks(&d, doc)[1].id, s.new);
}

#[test]
fn splitting_keeps_bold_and_links_on_both_sides() {
    let mut d = Documents::default();
    let doc = d.create("Spec", None, None).unwrap();
    let bold = Run { bold: true, ..Run::plain("big ") };
    let link = Run { link: Some("https://example.com".into()), ..Run::plain("site") };
    let id = d.insert_block(doc, None, Place::default(), BlockKind::Paragraph, vec![bold, link]).unwrap();
    d.split_block(doc, None, id, 6).unwrap();
    let b = blocks(&d, doc);
    assert_eq!(b[0].text.len(), 2);
    assert!(b[0].text[0].bold);
    assert_eq!(b[0].text[1].text, "si");
    assert_eq!(b[1].text[0].text, "te");
    assert_eq!(b[1].text[0].link.as_deref(), Some("https://example.com"));
}

#[test]
fn a_list_item_goes_on_as_a_list_item_and_a_checked_task_as_an_unchecked_one() {
    let (mut d, doc, ids) = doc_with(&[("bulleted", "one"), ("numbered", "two"), ("heading2", "Title")]);
    d.update_block(doc, None, ids[0], |b| {
        b.kind = BlockKind::Checklist { checked: true };
        Ok(())
    })
    .unwrap();
    for id in &ids {
        d.split_block(doc, None, *id, 3).unwrap();
    }
    let kinds: Vec<String> = blocks(&d, doc).iter().map(|b| b.kind.name()).collect();
    // A heading is followed by text, as in Notion.
    assert_eq!(kinds, ["checklist", "checklist", "numbered", "numbered", "heading2", "paragraph"]);
    assert_eq!(blocks(&d, doc)[1].kind, BlockKind::Checklist { checked: false });
}

#[test]
fn enter_at_the_start_of_a_heading_puts_an_empty_line_above_and_the_heading_keeps_its_words() {
    let (mut d, doc, ids) = doc_with(&[("heading1", "Title")]);
    let s = d.split_block(doc, None, ids[0], 0).unwrap();
    let b = blocks(&d, doc);
    assert_eq!(texts(&d, doc), ["", "Title"]);
    assert_eq!(b[0].kind, BlockKind::Paragraph);
    assert_eq!(b[1].id, ids[0]);
    assert_eq!(b[1].kind, BlockKind::Heading { level: 1 });
    // The keyboard stays on the heading.
    assert_eq!(s.focus, ids[0]);
}

#[test]
fn enter_at_the_end_of_an_open_toggle_starts_its_content() {
    let (mut d, doc, ids) = doc_with(&[("toggle", "More")]);
    d.update_block(doc, None, ids[0], |b| {
        b.kind = BlockKind::Toggle { open: true };
        Ok(())
    })
    .unwrap();
    let s = d.split_block(doc, None, ids[0], 4).unwrap();
    let b = blocks(&d, doc);
    assert_eq!(b.len(), 1);
    assert_eq!(b[0].children.len(), 1);
    assert_eq!(b[0].children[0].id, s.new);
}

#[test]
fn merging_joins_the_words_to_the_block_above_and_says_where_the_cursor_goes() {
    let (mut d, doc, ids) = doc_with(&[("heading2", "Hello"), ("paragraph", " world")]);
    let m = d.merge_block(doc, None, ids[1]).unwrap();
    assert_eq!(texts(&d, doc), ["Hello world"]);
    assert_eq!(m.into, ids[0]);
    assert_eq!(m.at, 5);
    assert_eq!(blocks(&d, doc)[0].kind, BlockKind::Heading { level: 2 });
}

#[test]
fn merging_into_the_last_block_inside_the_one_above() {
    let (mut d, doc, ids) = doc_with(&[("bulleted", "a"), ("paragraph", "c")]);
    let inner = d.insert_block(doc, None, Place { parent: Some(ids[0]), index: None }, BlockKind::Bulleted, vec![Run::plain("b")]).unwrap();
    let m = d.merge_block(doc, None, ids[1]).unwrap();
    assert_eq!(m.into, inner);
    let b = blocks(&d, doc);
    assert_eq!(b.len(), 1);
    assert_eq!(b[0].children[0].plain_text(), "bc");
}

#[test]
fn merging_a_block_keeps_what_was_inside_it_in_its_place() {
    let (mut d, doc, ids) = doc_with(&[("paragraph", "a"), ("toggle", "b")]);
    d.insert_block(doc, None, Place { parent: Some(ids[1]), index: None }, BlockKind::Paragraph, vec![Run::plain("inside")]).unwrap();
    d.merge_block(doc, None, ids[1]).unwrap();
    assert_eq!(texts(&d, doc), ["ab", "inside"]);
}

#[test]
fn merging_the_first_block_or_into_a_divider_is_refused_and_changes_nothing() {
    let (mut d, doc, ids) = doc_with(&[("paragraph", "a"), ("divider", ""), ("paragraph", "b")]);
    let before = blocks(&d, doc);
    assert!(d.merge_block(doc, None, ids[0]).is_err());
    assert!(d.merge_block(doc, None, ids[2]).is_err());
    assert_eq!(blocks(&d, doc), before);
}

#[test]
fn duplicating_copies_the_block_and_its_content_with_new_ids_right_after_it() {
    let (mut d, doc, ids) = doc_with(&[("toggle", "Group"), ("paragraph", "after")]);
    let inner = d.insert_block(doc, None, Place { parent: Some(ids[0]), index: None }, BlockKind::Paragraph, vec![Run::plain("x")]).unwrap();
    let copy = d.duplicate_block(doc, None, ids[0]).unwrap();
    let b = blocks(&d, doc);
    assert_eq!(texts(&d, doc), ["Group", "Group", "after"]);
    assert_eq!(b[1].id, copy);
    assert_ne!(b[1].children[0].id, inner);
    assert_eq!(b[1].children[0].plain_text(), "x");
}

#[test]
fn a_closed_version_refuses_all_three() {
    let (mut d, doc, ids) = doc_with(&[("paragraph", "a"), ("paragraph", "b")]);
    d.update_version_state_for_tests(doc, crate::docs::VersionState::Closed);
    assert!(d.split_block(doc, None, ids[0], 1).is_err());
    assert!(d.merge_block(doc, None, ids[1]).is_err());
    assert!(d.duplicate_block(doc, None, ids[0]).is_err());
}

#[test]
fn the_words_after_the_cursor_keep_the_colour_of_their_block() {
    let (mut d, doc, ids) = doc_with(&[("paragraph", "red words")]);
    d.update_block(doc, None, ids[0], |b| {
        b.style.color = Some("#D9483B".into());
        Ok(())
    })
    .unwrap();
    d.split_block(doc, None, ids[0], 3).unwrap();
    assert_eq!(blocks(&d, doc)[1].style.color.as_deref(), Some("#D9483B"));
}

#[test]
fn a_new_empty_line_starts_plain_even_after_a_coloured_block() {
    let (mut d, doc, ids) = doc_with(&[("heading2", "Red title")]);
    d.update_block(doc, None, ids[0], |b| {
        b.style.color = Some("#D9483B".into());
        b.style.size = Some(30.0);
        Ok(())
    })
    .unwrap();
    // Enter at the end, then Enter at the start of the heading.
    d.split_block(doc, None, ids[0], 9).unwrap();
    d.split_block(doc, None, ids[0], 0).unwrap();
    let b = blocks(&d, doc);
    assert_eq!(b[1].id, ids[0]);
    assert_eq!(b[0].style.color, None);
    assert_eq!(b[2].style.color, None);
    assert_eq!(b[2].style.size, None);
}

#[test]
fn an_open_toggle_as_deep_as_blocks_go_gets_the_new_block_beside_it_not_inside() {
    let mut d = Documents::default();
    let doc = d.create("Deep", None, None).unwrap();
    let mut parent = None;
    let mut last = BlockId(0);
    for _ in 0..crate::docs::MAX_BLOCK_DEPTH {
        last = d.insert_block(doc, None, Place { parent, index: None }, BlockKind::Toggle { open: true }, vec![Run::plain("t")]).unwrap();
        parent = Some(last);
    }
    let s = d.split_block(doc, None, last, 1).unwrap();
    let v = d.get(doc).unwrap().current().unwrap().clone();
    assert!(v.block(last).unwrap().children.is_empty());
    assert_eq!(v.locate(s.new).map(|(p, _)| p), v.locate(last).map(|(p, _)| p));
}

#[test]
fn a_cursor_past_the_end_or_inside_a_letter_is_safe() {
    let (mut d, doc, ids) = doc_with(&[("paragraph", "çã🙂x")]);
    d.split_block(doc, None, ids[0], 99).unwrap();
    assert_eq!(texts(&d, doc), ["çã🙂x", ""]);
    d.split_block(doc, None, ids[0], 3).unwrap();
    assert_eq!(texts(&d, doc), ["çã🙂", "x", ""]);
}
