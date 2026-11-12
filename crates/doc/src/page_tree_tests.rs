//! Nested pages (G9): the order of the list, nesting rules, repair of hand-edited files.

use crate::page_tree::NestError;
use crate::{Document, NodeId};

/// A file with pages A B C D E (A is the first, which `Document::new` makes).
fn five() -> (Document, Vec<NodeId>) {
    let mut d = Document::new("t");
    let a = d.pages[0];
    let mut ids = vec![a];
    for n in ["B", "C", "D", "E"] {
        ids.push(d.add_page(n));
    }
    (d, ids)
}

#[test]
fn a_file_without_nesting_is_flat_and_writes_no_key() {
    let (d, ids) = five();
    assert_eq!(d.page_children(None), ids);
    assert_eq!(d.page_depth(ids[2]), 0);
    let json = serde_json::to_string(&d).unwrap();
    assert!(!json.contains("page_tree"), "an old file stays as it was: {json}");
}

#[test]
fn nesting_moves_the_page_under_its_parent_and_keeps_the_list_in_tree_order() {
    let (mut d, ids) = five();
    let (a, b, c, dd, e) = (ids[0], ids[1], ids[2], ids[3], ids[4]);
    d.nest_page(dd, Some(a), None).unwrap();
    assert_eq!(d.pages, vec![a, dd, b, c, e]);
    d.nest_page(e, Some(a), None).unwrap();
    assert_eq!(d.pages, vec![a, dd, e, b, c]);
    // Two deep: C inside D inside A.
    d.nest_page(c, Some(dd), None).unwrap();
    assert_eq!(d.pages, vec![a, dd, c, e, b]);
    assert_eq!((d.page_depth(a), d.page_depth(dd), d.page_depth(c)), (0, 1, 2));
    assert_eq!(d.page_subtree(a), vec![a, dd, c, e]);
    assert!(d.page_is_inside(c, a));
    assert!(!d.page_is_inside(a, c));
}

#[test]
fn nesting_at_an_index_goes_before_that_sibling() {
    let (mut d, ids) = five();
    let (a, b, c, dd, e) = (ids[0], ids[1], ids[2], ids[3], ids[4]);
    d.nest_page(b, Some(a), None).unwrap();
    d.nest_page(c, Some(a), None).unwrap();
    d.nest_page(e, Some(a), Some(1)).unwrap();
    assert_eq!(d.page_children(Some(a)), vec![b, e, c]);
    assert_eq!(d.pages, vec![a, b, e, c, dd]);
    // Moving a page that already is a sibling counts the siblings left once it is lifted out.
    d.nest_page(b, Some(a), Some(2)).unwrap();
    assert_eq!(d.page_children(Some(a)), vec![e, c, b]);
}

#[test]
fn moving_a_parent_takes_its_subpages_along() {
    let (mut d, ids) = five();
    let (a, b, c, dd, e) = (ids[0], ids[1], ids[2], ids[3], ids[4]);
    d.nest_page(b, Some(a), None).unwrap();
    d.nest_page(c, Some(b), None).unwrap();
    // A (with B and C inside) goes in front of E, in the top level.
    d.nest_page(a, None, Some(2)).unwrap();
    assert_eq!(d.pages, vec![dd, e, a, b, c]);
    assert_eq!(d.page_children(None), vec![dd, e, a]);
}

#[test]
fn a_page_cannot_go_inside_itself_a_subpage_or_a_divider() {
    let (mut d, ids) = five();
    let (a, b, c) = (ids[0], ids[1], ids[2]);
    d.nest_page(b, Some(a), None).unwrap();
    d.nest_page(c, Some(b), None).unwrap();
    assert_eq!(d.nest_page(a, Some(a), None), Err(NestError::Cycle));
    assert_eq!(d.nest_page(a, Some(c), None), Err(NestError::Cycle));
    assert_eq!(d.nest_page(NodeId(9999), Some(a), None), Err(NestError::NotAPage(NodeId(9999))));
    assert_eq!(d.nest_page(a, Some(NodeId(9999)), None), Err(NestError::NotAPage(NodeId(9999))));
    let line = d.add_page("-");
    assert!(d.is_divider(line));
    assert_eq!(d.nest_page(a, Some(line), None), Err(NestError::Divider));
    // The refused edits changed nothing.
    assert_eq!(d.pages[..3], [a, b, c]);
}

#[test]
fn nesting_is_limited_in_depth() {
    let mut d = Document::new("t");
    let mut parent = d.pages[0];
    for _ in 0..crate::page_tree::MAX_PAGE_DEPTH {
        let p = d.add_page("p");
        d.nest_page(p, Some(parent), None).unwrap();
        parent = p;
    }
    let one_more = d.add_page("too deep");
    assert_eq!(d.nest_page(one_more, Some(parent), None), Err(NestError::TooDeep));
}

#[test]
fn collapsing_and_revealing() {
    let (mut d, ids) = five();
    let (a, b, c) = (ids[0], ids[1], ids[2]);
    d.nest_page(b, Some(a), None).unwrap();
    d.nest_page(c, Some(b), None).unwrap();
    assert!(d.set_page_collapsed(a, true));
    assert!(!d.set_page_collapsed(a, true), "already collapsed: no change");
    assert!(d.set_page_collapsed(b, true));
    assert!(d.page_hidden(c));
    assert!(!d.page_hidden(a));
    assert!(d.reveal_page(c));
    assert!(!d.page_hidden(c));
    assert!(!d.reveal_page(c), "nothing left to expand");
    assert!(!d.set_page_collapsed(NodeId(9999), true), "not a page");
}

#[test]
fn lifting_subpages_moves_them_up_a_level_in_place() {
    let (mut d, ids) = five();
    let (a, b, c, dd, e) = (ids[0], ids[1], ids[2], ids[3], ids[4]);
    d.nest_page(b, Some(a), None).unwrap();
    d.nest_page(c, Some(b), None).unwrap();
    d.nest_page(dd, Some(b), None).unwrap();
    d.lift_subpages(b);
    d.remove(b).unwrap();
    d.settle_pages();
    assert_eq!(d.page_children(Some(a)), vec![c, dd]);
    assert_eq!(d.pages, vec![a, c, dd, e]);
}

#[test]
fn a_copy_sits_next_to_its_source_after_its_subpages() {
    let (mut d, ids) = five();
    let (a, b) = (ids[0], ids[1]);
    d.nest_page(b, Some(a), None).unwrap();
    let copy = d.add_page("Copy of A");
    // The way `page.duplicate` places a copy: right after the source.
    d.pages.retain(|x| *x != copy);
    d.pages.insert(1, copy);
    d.nest_like(copy, a);
    assert_eq!(d.pages[..3], [a, b, copy]);
    assert_eq!(d.page_parent(copy), None);
}

#[test]
fn settling_repairs_a_missing_parent_a_cycle_and_a_scattered_list() {
    let (mut d, ids) = five();
    let (a, b, c, dd, e) = (ids[0], ids[1], ids[2], ids[3], ids[4]);
    // B's parent was deleted by hand; C and D point at each other; E sits inside A but far from it.
    d.page_tree.parents.insert(b, NodeId(777));
    d.page_tree.parents.insert(c, dd);
    d.page_tree.parents.insert(dd, c);
    d.page_tree.parents.insert(e, a);
    d.page_tree.collapsed.insert(NodeId(888));
    d.settle_pages();
    assert_eq!(d.page_parent(b), None, "a parent that is gone makes the page top-level");
    assert!(d.page_parent(c).is_none() || d.page_parent(dd).is_none(), "a cycle is cut");
    assert_eq!(d.page_parent(e), Some(a));
    assert!(d.page_tree.collapsed.is_empty(), "a collapsed page that is gone is forgotten");
    assert_eq!(d.pages.len(), 5);
    assert_eq!(d.pages[..2], [a, e], "E comes right after A");
    // Settled twice is the same.
    let before = d.clone();
    d.settle_pages();
    assert_eq!(d, before);
}

#[test]
fn a_parent_that_is_itself_is_ignored() {
    let (mut d, ids) = five();
    d.page_tree.parents.insert(ids[1], ids[1]);
    d.settle_pages();
    assert_eq!(d.page_parent(ids[1]), None);
    assert_eq!(d.pages, ids);
}

#[test]
fn nesting_and_collapsing_survive_a_save_and_load_and_an_old_file_opens_flat() {
    let (mut d, ids) = five();
    let (a, b, c) = (ids[0], ids[1], ids[2]);
    d.nest_page(b, Some(a), None).unwrap();
    d.nest_page(c, Some(b), None).unwrap();
    d.set_page_collapsed(b, true);
    let json = serde_json::to_string(&d).unwrap();
    let mut back: Document = serde_json::from_str(&json).unwrap();
    back.repair();
    assert_eq!(back.page_tree, d.page_tree);
    assert_eq!(back.pages, d.pages);
    // The same file without the key, as an older version wrote it: flat.
    let mut value: serde_json::Value = serde_json::from_str(&json).unwrap();
    value.as_object_mut().unwrap().remove("page_tree");
    let mut old: Document = serde_json::from_value(value).unwrap();
    old.repair();
    assert_eq!(old.page_parent(b), None);
    assert!(old.page_tree.is_empty());
    assert_eq!(old.pages, d.pages);
}
