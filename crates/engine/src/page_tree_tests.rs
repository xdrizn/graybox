//! Nested pages (G9): the `page.nest|unnest|collapse|reveal|tree` commands, the page commands that
//! know about subpages, undo, page order everywhere, and saving the file.

use serde_json::{Value, json};

use crate::{
    Session,
    cmd::file::{file_bytes, load_bytes},
};

fn ex(s: &mut Session, command: &str, params: Value) -> Value {
    s.execute(command, &params).unwrap()
}

fn refused(s: &mut Session, command: &str, params: Value) -> String {
    s.execute(command, &params).unwrap_err().to_string()
}

/// A file with pages A B C D E; returns their ids.
fn five() -> (Session, Vec<u64>) {
    let mut s = Session::new();
    ex(&mut s, "page.rename", json!({"name": "A"}));
    let mut ids = vec![s.doc().unwrap().pages[0].0];
    for n in ["B", "C", "D", "E"] {
        ids.push(ex(&mut s, "page.add", json!({"name": n}))["id"].as_u64().unwrap());
    }
    (s, ids)
}

fn names(s: &Session) -> Vec<String> {
    let d = s.doc().unwrap();
    d.pages.iter().map(|p| d.node(*p).unwrap().name.clone()).collect()
}

/// `name:depth` for every page in list order.
fn shape(s: &mut Session) -> Vec<String> {
    ex(s, "page.tree", json!({})).as_array().unwrap().iter().map(|p| format!("{}:{}", p["name"].as_str().unwrap(), p["depth"])).collect()
}

#[test]
fn nesting_puts_pages_inside_others_two_deep_and_undoes_step_by_step() {
    let (mut s, id) = five();
    ex(&mut s, "page.nest", json!({"id": id[1], "parent": id[0]}));
    ex(&mut s, "page.nest", json!({"id": id[2], "parent": id[0]}));
    ex(&mut s, "page.nest", json!({"id": id[3], "parent": id[1]}));
    assert_eq!(shape(&mut s), ["A:0", "B:1", "D:2", "C:1", "E:0"]);
    assert_eq!(names(&s), ["A", "B", "D", "C", "E"], "the page list is the tree order");

    ex(&mut s, "edit.undo", json!({}));
    assert_eq!(shape(&mut s), ["A:0", "B:1", "C:1", "D:0", "E:0"]);
    ex(&mut s, "edit.undo", json!({}));
    ex(&mut s, "edit.undo", json!({}));
    assert_eq!(shape(&mut s), ["A:0", "B:0", "C:0", "D:0", "E:0"]);
    ex(&mut s, "edit.redo", json!({}));
    assert_eq!(shape(&mut s), ["A:0", "B:1", "C:0", "D:0", "E:0"]);
}

#[test]
fn unnesting_puts_the_page_right_after_the_one_it_was_in() {
    let (mut s, id) = five();
    ex(&mut s, "page.nest", json!({"id": id[1], "parent": id[0]}));
    ex(&mut s, "page.nest", json!({"id": id[2], "parent": id[1]}));
    ex(&mut s, "page.nest", json!({"id": id[3], "parent": id[1]}));
    assert_eq!(shape(&mut s), ["A:0", "B:1", "C:2", "D:2", "E:0"]);
    ex(&mut s, "page.unnest", json!({"id": id[2]}));
    assert_eq!(shape(&mut s), ["A:0", "B:1", "D:2", "C:1", "E:0"]);
    ex(&mut s, "page.unnest", json!({"id": id[1]}));
    assert_eq!(shape(&mut s), ["A:0", "C:1", "B:0", "D:1", "E:0"]);
    assert!(refused(&mut s, "page.unnest", json!({"id": id[0]})).contains("not inside"));
}

#[test]
fn refusals_leave_the_list_as_it_was() {
    let (mut s, id) = five();
    ex(&mut s, "page.nest", json!({"id": id[1], "parent": id[0]}));
    let before = shape(&mut s);
    assert!(refused(&mut s, "page.nest", json!({"id": id[0], "parent": id[1]})).contains("inside"));
    assert!(refused(&mut s, "page.nest", json!({"id": id[0], "parent": id[0]})).contains("inside"));
    assert!(refused(&mut s, "page.nest", json!({"id": id[2], "parent": 99999})).contains("not a page"));
    assert!(refused(&mut s, "page.nest", json!({"id": 99999, "parent": id[0]})).contains("not a page"));
    assert!(refused(&mut s, "page.nest", json!({"id": id[2], "parent": "x"})).contains("parent"));
    assert_eq!(shape(&mut s), before);
}

#[test]
fn collapsed_state_is_kept_and_selecting_a_hidden_page_expands_its_parents() {
    let (mut s, id) = five();
    ex(&mut s, "page.nest", json!({"id": id[1], "parent": id[0]}));
    ex(&mut s, "page.nest", json!({"id": id[2], "parent": id[1]}));
    ex(&mut s, "page.collapse", json!({"id": id[0]}));
    ex(&mut s, "page.collapse", json!({"id": id[1]}));
    let tree = ex(&mut s, "page.tree", json!({}));
    assert_eq!(tree[0]["collapsed"], true);
    assert_eq!(tree[1]["collapsed"], true);
    ex(&mut s, "page.select", json!({"id": id[2]}));
    let tree = ex(&mut s, "page.tree", json!({}));
    assert_eq!((tree[0]["collapsed"].as_bool(), tree[1]["collapsed"].as_bool()), (Some(false), Some(false)), "the parents opened");
    // Collapsing again and going to the next page by its shortcut command does the same.
    ex(&mut s, "page.collapse", json!({"id": id[0]}));
    ex(&mut s, "page.select", json!({"id": id[0]}));
    ex(&mut s, "view.nextPage", json!({}));
    assert_eq!(s.page().unwrap().0, id[1]);
    ex(&mut s, "page.collapse", json!({"id": id[0]}));
    ex(&mut s, "page.select", json!({"id": id[4]}));
    ex(&mut s, "view.prevPage", json!({}));
    ex(&mut s, "view.prevPage", json!({}));
    assert_eq!(s.page().unwrap().0, id[2], "going back reaches a page inside a collapsed parent");
    assert_eq!(ex(&mut s, "page.tree", json!({}))[0]["collapsed"], false);
    // Collapsing is no undo step of its own.
    let undo = s.ds().unwrap().history.undo.len();
    ex(&mut s, "page.collapse", json!({"id": id[0]}));
    assert_eq!(s.ds().unwrap().history.undo.len(), undo);
    assert!(s.ds().unwrap().is_dirty(), "but the file has changed and wants saving");
}

#[test]
fn next_and_previous_page_follow_the_tree() {
    let (mut s, id) = five();
    ex(&mut s, "page.nest", json!({"id": id[3], "parent": id[0]}));
    ex(&mut s, "page.select", json!({"id": id[0]}));
    let mut seen = vec![];
    for _ in 0..4 {
        ex(&mut s, "view.nextPage", json!({}));
        seen.push(s.page().unwrap().0);
    }
    assert_eq!(seen, [id[3], id[1], id[2], id[4]], "the subpage D comes right after A");
}

#[test]
fn adding_a_subpage_is_one_step_and_opens_the_parent() {
    let (mut s, id) = five();
    ex(&mut s, "page.collapse", json!({"id": id[1]}));
    let undo = s.ds().unwrap().history.undo.len();
    let sub = ex(&mut s, "page.add", json!({"name": "Sub", "parent": id[1]}))["id"].as_u64().unwrap();
    assert_eq!(s.ds().unwrap().history.undo.len(), undo + 1);
    assert_eq!(shape(&mut s), ["A:0", "B:0", "Sub:1", "C:0", "D:0", "E:0"]);
    assert_eq!(s.page().unwrap().0, sub);
    assert_eq!(ex(&mut s, "page.tree", json!({}))[1]["collapsed"], false);
    ex(&mut s, "edit.undo", json!({}));
    assert_eq!(shape(&mut s), ["A:0", "B:0", "C:0", "D:0", "E:0"]);
    assert!(refused(&mut s, "page.add", json!({"parent": 99999})).contains("not a page"));
    assert_eq!(names(&s).len(), 5, "a refused add leaves no page behind");
}

#[test]
fn deleting_a_page_moves_its_subpages_up_or_takes_them_along() {
    let (mut s, id) = five();
    ex(&mut s, "page.nest", json!({"id": id[1], "parent": id[0]}));
    ex(&mut s, "page.nest", json!({"id": id[2], "parent": id[1]}));
    ex(&mut s, "page.nest", json!({"id": id[3], "parent": id[1]}));
    ex(&mut s, "page.delete", json!({"id": id[1]}));
    assert_eq!(shape(&mut s), ["A:0", "C:1", "D:1", "E:0"], "its subpages moved up a level");
    ex(&mut s, "edit.undo", json!({}));
    assert_eq!(shape(&mut s), ["A:0", "B:1", "C:2", "D:2", "E:0"]);
    ex(&mut s, "page.delete", json!({"id": id[1], "subpages": true}));
    assert_eq!(shape(&mut s), ["A:0", "E:0"]);
    assert!(s.doc().unwrap().page_tree.is_empty(), "nothing is left of the deleted pages' places: {:?}", s.doc().unwrap().page_tree);
    assert!(s.doc().unwrap().validate().is_empty(), "and the file is valid");
    ex(&mut s, "edit.undo", json!({}));
    assert_eq!(shape(&mut s), ["A:0", "B:1", "C:2", "D:2", "E:0"], "one undo brings the whole branch back");
}

#[test]
fn deleting_every_page_with_subpages_is_refused_and_rolled_back() {
    let mut s = Session::new();
    let a = s.doc().unwrap().pages[0].0;
    let b = ex(&mut s, "page.add", json!({"name": "B", "parent": a}))["id"].as_u64().unwrap();
    assert!(s.execute("page.delete", &json!({"id": a, "subpages": true})).is_err());
    assert_eq!(shape(&mut s), ["Page 1:0", "B:1"]);
    assert_eq!(s.doc().unwrap().page_parent(figcraft_doc::NodeId(b)), Some(figcraft_doc::NodeId(a)));
}

#[test]
fn a_duplicate_sits_next_to_its_source_after_its_subpages() {
    let (mut s, id) = five();
    ex(&mut s, "page.nest", json!({"id": id[1], "parent": id[0]}));
    ex(&mut s, "page.duplicate", json!({"id": id[0]}));
    assert_eq!(shape(&mut s), ["A:0", "B:1", "Copy of A:0", "C:0", "D:0", "E:0"]);
    ex(&mut s, "page.duplicate", json!({"id": id[1]}));
    assert_eq!(shape(&mut s)[..4], ["A:0", "B:1", "Copy of B:1", "Copy of A:0"]);
}

#[test]
fn moving_a_page_with_the_old_command_keeps_the_tree_in_order() {
    let (mut s, id) = five();
    ex(&mut s, "page.nest", json!({"id": id[1], "parent": id[0]}));
    ex(&mut s, "page.move", json!({"id": id[0], "index": 4}));
    assert_eq!(names(&s), ["C", "D", "E", "A", "B"]);
    assert_eq!(shape(&mut s)[3..], ["A:0", "B:1"]);
}

#[test]
fn nesting_survives_saving_and_opening_and_an_old_file_opens_flat() {
    let (mut s, id) = five();
    ex(&mut s, "page.nest", json!({"id": id[1], "parent": id[0]}));
    ex(&mut s, "page.nest", json!({"id": id[2], "parent": id[1]}));
    ex(&mut s, "page.nest", json!({"id": id[3], "parent": id[0]}));
    ex(&mut s, "page.collapse", json!({"id": id[1]}));
    let bytes = file_bytes(s.ds().unwrap()).unwrap();
    let (doc, _) = load_bytes(&bytes, "nested.figcraft").unwrap();
    assert_eq!(doc.page_tree, s.doc().unwrap().page_tree);
    assert_eq!(doc.pages, s.doc().unwrap().pages);
    assert!(doc.page_tree.collapsed.contains(&figcraft_doc::NodeId(id[1])));

    // The same file from before this feature: no key, every page at the top.
    let flat = Session::new();
    let bytes = file_bytes(flat.ds().unwrap()).unwrap();
    let (doc, _) = load_bytes(&bytes, "plain.figcraft").unwrap();
    assert!(doc.page_tree.is_empty());
    assert!(serde_json::to_value(&doc).unwrap().get("page_tree").is_none());
}

#[test]
fn a_file_whose_parent_page_is_missing_opens_with_the_page_at_the_top() {
    let (mut s, id) = five();
    ex(&mut s, "page.nest", json!({"id": id[1], "parent": id[0]}));
    // Hand-edit: the parent is a page number that does not exist.
    s.doc_mut().unwrap().page_tree.parents.insert(figcraft_doc::NodeId(id[1]), figcraft_doc::NodeId(424_242));
    let bytes = file_bytes(s.ds().unwrap()).unwrap();
    let (doc, _) = load_bytes(&bytes, "broken.figcraft").unwrap();
    assert_eq!(doc.page_parent(figcraft_doc::NodeId(id[1])), None);
    assert!(doc.page_tree.parents.is_empty());
}

#[test]
fn nest_commands_survive_junk() {
    let (mut s, id) = five();
    let junk = [
        json!(null),
        json!([]),
        json!({"id": -1}),
        json!({"id": 1.5, "parent": "x"}),
        json!({"id": id[1], "parent": u64::MAX, "index": -1}),
        json!({"id": id[1], "parent": id[0], "index": 1e308}),
        json!({"id": id[1], "parent": id[0], "index": "x"}),
        json!({"id": id[1], "collapsed": "yes"}),
        json!({"subpages": 3}),
    ];
    for cmd in ["page.nest", "page.unnest", "page.collapse", "page.reveal", "page.tree", "page.add", "page.delete"] {
        for j in &junk {
            let r = s.execute(cmd, j);
            assert!(!matches!(r, Err(crate::EngineError::Internal { .. })), "{cmd} {j}: {r:?}");
        }
    }
}
