//! The `doc.*` commands: what they do, undo, refusals, junk, and saving the file.

use serde_json::{Value, json};

use crate::{
    Session,
    cmd::file::{file_bytes, load_bytes},
};

fn ex(s: &mut Session, command: &str, params: Value) -> Value {
    s.execute(command, &params).unwrap()
}

fn id_of(v: &Value) -> u64 {
    v["id"].as_u64().unwrap()
}

fn tree(s: &mut Session) -> Vec<(u64, String)> {
    let v = ex(s, "doc.get", json!({}));
    v["documents"].as_array().unwrap().iter().map(|d| (d["id"].as_u64().unwrap(), d["title"].as_str().unwrap().to_string())).collect()
}

fn blocks(s: &mut Session, doc: u64) -> Vec<Value> {
    ex(s, "doc.get", json!({"id": doc}))["blocks"].as_array().unwrap().clone()
}

#[test]
fn documents_are_made_renamed_nested_and_deleted_and_each_step_undoes() {
    let mut s = Session::new();
    let a = id_of(&ex(&mut s, "doc.create", json!({"title": "Spec"})));
    let b = id_of(&ex(&mut s, "doc.create", json!({"title": "Notes", "icon": "📝"})));
    let c = id_of(&ex(&mut s, "doc.create", json!({"title": "Inner", "parent": a})));
    assert_eq!(tree(&mut s), [(a, "Spec".into()), (c, "Inner".into()), (b, "Notes".into())]);

    ex(&mut s, "doc.rename", json!({"id": b, "title": "Meeting"}));
    ex(&mut s, "doc.set_icon", json!({"id": b, "icon": "🎯"}));
    let info = ex(&mut s, "doc.get", json!({"id": b}));
    assert_eq!((info["title"].as_str(), info["icon"].as_str()), (Some("Meeting"), Some("🎯")));

    ex(&mut s, "doc.move", json!({"id": b, "parent": a, "index": 0}));
    assert_eq!(tree(&mut s), [(a, "Spec".into()), (b, "Meeting".into()), (c, "Inner".into())]);
    ex(&mut s, "doc.move", json!({"id": b}));
    assert_eq!(tree(&mut s), [(a, "Spec".into()), (c, "Inner".into()), (b, "Meeting".into())]);

    let gone = ex(&mut s, "doc.delete", json!({"id": a}));
    assert_eq!(gone["deleted"].as_array().unwrap().len(), 2);
    assert_eq!(tree(&mut s), [(b, "Meeting".into())]);

    // Undo walks back one step at a time, redo forward.
    ex(&mut s, "edit.undo", json!({}));
    assert_eq!(tree(&mut s).len(), 3);
    ex(&mut s, "edit.undo", json!({})); // the second move
    ex(&mut s, "edit.undo", json!({})); // the first move
    assert_eq!(tree(&mut s), [(a, "Spec".into()), (c, "Inner".into()), (b, "Meeting".into())]);
    ex(&mut s, "edit.undo", json!({})); // icon
    ex(&mut s, "edit.undo", json!({})); // rename
    assert_eq!(ex(&mut s, "doc.get", json!({"id": b}))["title"], "Notes");
    ex(&mut s, "edit.redo", json!({}));
    assert_eq!(ex(&mut s, "doc.get", json!({"id": b}))["title"], "Meeting");
}

#[test]
fn blocks_are_added_edited_turned_into_moved_and_deleted_and_undone() {
    let mut s = Session::new();
    let d = id_of(&ex(&mut s, "doc.create", json!({"title": "Spec"})));
    let h = id_of(&ex(&mut s, "doc.block.insert", json!({"doc": d, "kind": "heading1", "text": "Goals"})));
    let p = id_of(&ex(&mut s, "doc.block.insert", json!({"doc": d, "text": "Ship it", "style": {"color": "#FF8800", "size": 18}})));
    let t = id_of(&ex(&mut s, "doc.block.insert", json!({"doc": d, "kind": "todo", "text": "Write", "index": 0})));
    let kinds = |s: &mut Session| blocks(s, d).iter().map(|b| b["kind"]["type"].as_str().unwrap().to_string()).collect::<Vec<_>>();
    assert_eq!(kinds(&mut s), ["checklist", "heading", "paragraph"]);

    ex(&mut s, "doc.block.set", json!({"doc": d, "id": t, "data": {"checked": true}}));
    assert_eq!(blocks(&mut s, d)[0]["kind"]["checked"], true);
    ex(&mut s, "doc.block.set", json!({"doc": d, "id": p, "text": "Ship it today", "style": {"font": "Inter", "size": null}}));
    let para = blocks(&mut s, d)[2].clone();
    assert_eq!(para["text"][0]["text"], "Ship it today");
    assert_eq!(para["style"]["font"], "Inter");
    assert_eq!(para["style"]["color"], "#FF8800");
    assert!(para["style"].get("size").is_none());

    ex(&mut s, "doc.block.set", json!({"doc": d, "id": p, "runs": [{"text": "bold", "bold": true}, {"text": " plain"}]}));
    assert_eq!(blocks(&mut s, d)[2]["text"][0]["bold"], true);

    ex(&mut s, "doc.block.turn_into", json!({"doc": d, "id": p, "kind": "heading2"}));
    let turned = blocks(&mut s, d)[2].clone();
    assert_eq!((turned["kind"]["type"].as_str(), turned["kind"]["level"].as_u64()), (Some("heading"), Some(2)));
    assert_eq!(turned["text"].as_array().unwrap().len(), 2, "the text stays");
    ex(&mut s, "doc.block.turn_into", json!({"doc": d, "id": p, "kind": {"type": "callout", "icon": "💡"}}));
    assert_eq!(blocks(&mut s, d)[2]["kind"]["icon"], "💡");

    ex(&mut s, "doc.block.move", json!({"doc": d, "id": h, "parent": t}));
    assert_eq!(kinds(&mut s), ["checklist", "callout"]);
    assert_eq!(blocks(&mut s, d)[0]["children"][0]["id"], h);
    ex(&mut s, "doc.block.delete", json!({"doc": d, "id": t}));
    assert_eq!(kinds(&mut s), ["callout"]);

    // One undo per command.
    ex(&mut s, "edit.undo", json!({}));
    assert_eq!(kinds(&mut s), ["checklist", "callout"]);
    ex(&mut s, "edit.undo", json!({}));
    assert_eq!(kinds(&mut s), ["checklist", "heading", "callout"]);
}

#[test]
fn refusals_say_why_and_change_nothing() {
    let mut s = Session::new();
    let a = id_of(&ex(&mut s, "doc.create", json!({"title": "A"})));
    let b = id_of(&ex(&mut s, "doc.create", json!({"title": "B", "parent": a})));
    let before = tree(&mut s);
    for (cmd, params) in [
        ("doc.move", json!({"id": a, "parent": b})),
        ("doc.move", json!({"id": a, "parent": a})),
        ("doc.rename", json!({"id": 999, "title": "x"})),
        ("doc.rename", json!({"id": a})),
        ("doc.rename", json!({"id": a, "title": 3})),
        ("doc.delete", json!({"id": 999})),
        ("doc.create", json!({"parent": 999})),
        ("doc.set_icon", json!({"id": a, "icon": "this icon is far too long to be one"})),
        ("doc.block.insert", json!({"doc": 999})),
        ("doc.block.insert", json!({"doc": a, "kind": "nonsense"})),
        ("doc.block.insert", json!({"doc": a, "text": "x", "runs": []})),
        ("doc.block.insert", json!({"doc": a, "version": 9})),
        ("doc.block.insert", json!({"doc": a, "style": {"size": 1000}})),
        ("doc.block.insert", json!({"doc": a, "style": {"nope": 1}})),
        ("doc.block.set", json!({"doc": a, "id": 1})),
        ("doc.block.set", json!({"doc": a, "id": 1, "text": "x"})),
        ("doc.block.turn_into", json!({"doc": a, "id": 1})),
        ("doc.block.move", json!({"doc": a, "id": 1})),
        ("doc.block.delete", json!({"doc": a, "id": 1})),
        ("doc.nothing", json!({})),
    ] {
        assert!(s.execute(cmd, &params).is_err(), "{cmd} {params}");
    }
    assert_eq!(tree(&mut s), before);
    assert!(blocks(&mut s, a).is_empty());
    // A refusal is not an undo step.
    ex(&mut s, "edit.undo", json!({}));
    assert_eq!(tree(&mut s).len(), 1);
}

#[test]
fn data_that_does_not_fit_leaves_the_block_alone() {
    let mut s = Session::new();
    let d = id_of(&ex(&mut s, "doc.create", json!({})));
    let p = id_of(&ex(&mut s, "doc.block.insert", json!({"doc": d, "text": "keep"})));
    for params in [
        json!({"doc": d, "id": p, "data": {"type": "divider"}}),
        json!({"doc": d, "id": p, "data": 3}),
        json!({"doc": d, "id": p, "style": {"color": "red"}}),
        json!({"doc": d, "id": p, "text": "gone", "style": {"size": "big"}}),
    ] {
        assert!(s.execute("doc.block.set", &params).is_err(), "{params}");
    }
    assert_eq!(blocks(&mut s, d)[0]["text"][0]["text"], "keep");
}

#[test]
fn a_refused_style_or_data_says_what_was_wrong() {
    let mut s = Session::new();
    let d = id_of(&ex(&mut s, "doc.create", json!({})));
    let p = id_of(&ex(&mut s, "doc.block.insert", json!({"doc": d, "kind": "checklist", "text": "x"})));
    let why = |s: &mut Session, params: Value| s.execute("doc.block.set", &params).unwrap_err().to_string();
    assert!(why(&mut s, json!({"doc": d, "id": p, "style": {"colour": "#fff"}})).contains("style has no colour"));
    assert!(why(&mut s, json!({"doc": d, "id": p, "style": {"size": "big"}})).contains("style.size"));
    assert!(why(&mut s, json!({"doc": d, "id": p, "data": {"checked": "yes"}})).contains("checklist"));
    // `doc.get` lists a count in the tree and the blocks themselves for one document.
    assert_eq!(ex(&mut s, "doc.get", json!({}))["documents"][0]["blockCount"], 1);
    assert!(ex(&mut s, "doc.get", json!({"id": d}))["blocks"].is_array());
}

#[test]
fn a_new_document_has_a_title_that_can_be_empty() {
    let mut s = Session::new();
    let d = id_of(&ex(&mut s, "doc.create", json!({})));
    assert_eq!(ex(&mut s, "doc.get", json!({"id": d}))["title"], "");
}

#[test]
fn documents_survive_saving_and_opening_the_file() {
    let mut s = Session::new();
    let a = id_of(&ex(&mut s, "doc.create", json!({"title": "Spec", "icon": "📄"})));
    let b = id_of(&ex(&mut s, "doc.create", json!({"title": "Notes", "parent": a})));
    ex(&mut s, "doc.block.insert", json!({"doc": b, "kind": "heading1", "text": "Hello"}));
    ex(&mut s, "doc.block.insert", json!({"doc": b, "text": "World", "style": {"color": "#00FF00"}}));
    let bytes = file_bytes(s.ds().unwrap()).unwrap();
    let (doc, _) = load_bytes(&bytes, "docs.figcraft").unwrap();
    assert_eq!(doc.docs, s.doc().unwrap().docs);
    assert_eq!(doc.docs.len(), 2);
    assert_eq!(doc.docs.get(figcraft_doc::docs::DocId(b)).unwrap().parent, Some(figcraft_doc::docs::DocId(a)));
}

#[test]
fn a_file_without_documents_saves_without_the_key() {
    let s = Session::new();
    let bytes = file_bytes(s.ds().unwrap()).unwrap();
    let (doc, _) = load_bytes(&bytes, "plain.figcraft").unwrap();
    assert!(doc.docs.is_empty());
    let json = serde_json::to_value(&doc).unwrap();
    assert!(json.get("docs").is_none());
}

#[test]
fn junk_parameters_are_refused_not_panicked_on() {
    let mut s = Session::new();
    let d = id_of(&ex(&mut s, "doc.create", json!({"title": "J"})));
    let junk = [
        json!(null),
        json!(1),
        json!("x"),
        json!([]),
        json!({"id": -1}),
        json!({"id": 1.5}),
        json!({"id": "1"}),
        json!({"id": u64::MAX, "doc": u64::MAX}),
        json!({"doc": d, "id": d, "index": -1}),
        json!({"doc": d, "index": 1e30}),
        json!({"doc": d, "kind": {"type": "heading", "level": 1e9}}),
        json!({"doc": d, "runs": [1, 2]}),
        json!({"doc": d, "runs": [{"text": 5}]}),
        json!({"doc": d, "text": {"a": 1}}),
    ];
    for cmd in [
        "doc.get",
        "doc.create",
        "doc.rename",
        "doc.move",
        "doc.delete",
        "doc.set_icon",
        "doc.block.insert",
        "doc.block.set",
        "doc.block.turn_into",
        "doc.block.move",
        "doc.block.delete",
        "doc.block.split",
        "doc.block.merge",
        "doc.block.duplicate",
        "doc.block.format",
        "doc.table.cell",
        "doc.table.change",
    ] {
        for j in &junk {
            let _ = s.execute(cmd, j);
        }
    }
    // The session is still sane.
    assert!(ex(&mut s, "doc.get", json!({"id": d}))["id"].is_u64() || tree(&mut s).is_empty());
}

#[test]
fn a_huge_index_just_means_last() {
    let mut s = Session::new();
    let d = id_of(&ex(&mut s, "doc.create", json!({})));
    ex(&mut s, "doc.block.insert", json!({"doc": d, "text": "a"}));
    ex(&mut s, "doc.block.insert", json!({"doc": d, "text": "b", "index": 4_000_000_000_u64}));
    assert_eq!(blocks(&mut s, d)[1]["text"][0]["text"], "b");
}

fn texts(s: &mut Session, doc: u64) -> Vec<String> {
    blocks(s, doc).iter().map(|b| b["text"].as_array().map_or(String::new(), |r| r.iter().filter_map(|x| x["text"].as_str()).collect())).collect()
}

#[test]
fn enter_splits_a_block_in_one_step_that_one_undo_takes_back() {
    let mut s = Session::new();
    let d = id_of(&ex(&mut s, "doc.create", json!({})));
    let b = id_of(&ex(&mut s, "doc.block.insert", json!({"doc": d, "kind": "bulleted", "text": "Hello world"})));
    let undo = s.ds().unwrap().history.undo.len();
    let out = ex(&mut s, "doc.block.split", json!({"doc": d, "id": b, "at": 5}));
    assert_eq!(texts(&mut s, d), ["Hello", " world"]);
    assert_eq!(blocks(&mut s, d)[1]["kind"]["type"], "bulleted");
    assert_eq!(out["focus"], out["id"]);
    assert_eq!(s.ds().unwrap().history.undo.len(), undo + 1);
    ex(&mut s, "edit.undo", json!({}));
    assert_eq!(texts(&mut s, d), ["Hello world"]);
}

#[test]
fn backspace_at_the_start_joins_a_block_to_the_one_above_and_says_where_the_cursor_goes() {
    let mut s = Session::new();
    let d = id_of(&ex(&mut s, "doc.create", json!({})));
    let a = id_of(&ex(&mut s, "doc.block.insert", json!({"doc": d, "text": "Hello"})));
    let b = id_of(&ex(&mut s, "doc.block.insert", json!({"doc": d, "text": " world"})));
    let out = ex(&mut s, "doc.block.merge", json!({"doc": d, "id": b}));
    assert_eq!((out["id"].as_u64(), out["at"].as_u64()), (Some(a), Some(5)));
    assert_eq!(texts(&mut s, d), ["Hello world"]);
    // Nothing above the first block: refused, nothing changes.
    assert!(s.execute("doc.block.merge", &json!({"doc": d, "id": a})).is_err());
}

#[test]
fn duplicate_copies_a_block_right_after_it() {
    let mut s = Session::new();
    let d = id_of(&ex(&mut s, "doc.create", json!({})));
    let a = id_of(&ex(&mut s, "doc.block.insert", json!({"doc": d, "kind": "callout", "text": "Note"})));
    ex(&mut s, "doc.block.insert", json!({"doc": d, "text": "end"}));
    let copy = id_of(&ex(&mut s, "doc.block.duplicate", json!({"doc": d, "id": a})));
    assert_ne!(copy, a);
    assert_eq!(texts(&mut s, d), ["Note", "Note", "end"]);
    assert_eq!(blocks(&mut s, d)[1]["kind"]["type"], "callout");
}

#[test]
fn a_markdown_shortcut_turns_the_block_and_drops_the_marks_in_one_step() {
    let mut s = Session::new();
    let d = id_of(&ex(&mut s, "doc.create", json!({})));
    let b = id_of(&ex(&mut s, "doc.block.insert", json!({"doc": d, "text": "## "})));
    let undo = s.ds().unwrap().history.undo.len();
    ex(&mut s, "doc.block.turn_into", json!({"doc": d, "id": b, "kind": "heading2", "text": ""}));
    assert_eq!(blocks(&mut s, d)[0]["kind"], json!({"type": "heading", "level": 2}));
    assert_eq!(texts(&mut s, d), [""]);
    assert_eq!(s.ds().unwrap().history.undo.len(), undo + 1);
}

#[test]
fn format_puts_bold_on_some_words_in_one_step_and_toggles_it_back() {
    let mut s = Session::new();
    let d = id_of(&ex(&mut s, "doc.create", json!({})));
    let b = id_of(&ex(&mut s, "doc.block.insert", json!({"doc": d, "text": "Hello world"})));
    let undo = s.ds().unwrap().history.undo.len();
    ex(&mut s, "doc.block.format", json!({"doc": d, "id": b, "from": 6, "to": 11, "mark": "bold"}));
    let runs = blocks(&mut s, d)[0]["text"].clone();
    assert_eq!(runs, json!([{"text": "Hello "}, {"text": "world", "bold": true}]));
    assert_eq!(s.ds().unwrap().history.undo.len(), undo + 1);
    ex(&mut s, "doc.block.format", json!({"doc": d, "id": b, "from": 0, "to": 5, "mark": "color", "value": "#FF0000"}));
    assert_eq!(blocks(&mut s, d)[0]["text"][0], json!({"text": "Hello", "color": "#FF0000"}));
    ex(&mut s, "doc.block.format", json!({"doc": d, "id": b, "from": 6, "to": 11, "mark": "bold"}));
    assert_eq!(blocks(&mut s, d)[0]["text"][1], json!({"text": " world"}));
    // Explicitly on, and refusals.
    ex(&mut s, "doc.block.format", json!({"doc": d, "id": b, "from": 0, "to": 11, "mark": "italic", "on": true}));
    ex(&mut s, "doc.block.format", json!({"doc": d, "id": b, "from": 0, "to": 11, "mark": "italic", "on": true}));
    assert!(blocks(&mut s, d)[0]["text"].as_array().unwrap().iter().all(|r| r["italic"] == true));
    for bad in [
        json!({"doc": d, "id": b, "from": 3, "to": 3, "mark": "bold"}),
        json!({"doc": d, "id": b, "from": 0, "to": 3, "mark": "blink"}),
        json!({"doc": d, "id": b, "from": 0, "to": 3}),
        json!({"doc": d, "id": b, "from": 0, "to": 3, "mark": "color", "value": "red"}),
        json!({"doc": d, "id": b, "from": 0, "to": 3, "mark": "bold", "on": "yes"}),
        json!({"doc": d, "id": b, "to": 3, "mark": "bold"}),
    ] {
        assert!(s.execute("doc.block.format", &bad).is_err(), "{bad}");
    }
}

#[test]
fn table_cells_rows_and_columns_change_one_undo_step_at_a_time() {
    let mut s = Session::new();
    let d = id_of(&ex(&mut s, "doc.create", json!({})));
    let rows = json!([[[], []], [[], []]]);
    let t = id_of(&ex(&mut s, "doc.block.insert", json!({"doc": d, "kind": {"type": "table", "table": {"rows": rows, "header": true}}})));
    let undo = s.ds().unwrap().history.undo.len();
    ex(&mut s, "doc.table.cell", json!({"doc": d, "id": t, "row": 0, "col": 1, "text": "Speed"}));
    assert_eq!(s.ds().unwrap().history.undo.len(), undo + 1);
    assert_eq!(blocks(&mut s, d)[0]["kind"]["table"]["rows"][0][1], json!([{"text": "Speed"}]));
    ex(&mut s, "doc.table.change", json!({"doc": d, "id": t, "op": "insertRow", "at": 0}));
    ex(&mut s, "doc.table.change", json!({"doc": d, "id": t, "op": "insertColumn", "at": 99}));
    let table = blocks(&mut s, d)[0]["kind"]["table"].clone();
    assert_eq!(table["rows"].as_array().unwrap().len(), 3);
    assert_eq!(table["rows"][1][1], json!([{"text": "Speed"}]));
    assert_eq!(table["rows"][1].as_array().unwrap().len(), 3);
    ex(&mut s, "doc.table.change", json!({"doc": d, "id": t, "op": "header", "on": false}));
    assert!(blocks(&mut s, d)[0]["kind"]["table"].get("header").is_none());
    ex(&mut s, "edit.undo", json!({}));
    assert_eq!(blocks(&mut s, d)[0]["kind"]["table"]["header"], true);
    for bad in [
        json!({"doc": d, "id": t, "row": 9, "col": 0, "text": "x"}),
        json!({"doc": d, "id": t, "row": 0, "col": 0}),
        json!({"doc": d, "id": t, "row": -1, "col": 0, "text": "x"}),
    ] {
        assert!(s.execute("doc.table.cell", &bad).is_err(), "{bad}");
    }
    for bad in [
        json!({"doc": d, "id": t, "op": "explode"}),
        json!({"doc": d, "id": t}),
        json!({"doc": d, "id": t, "op": "deleteRow", "at": 50}),
        // A delete names its row or column: none never means the first.
        json!({"doc": d, "id": t, "op": "deleteRow"}),
        json!({"doc": d, "id": t, "op": "deleteColumn"}),
        json!({"doc": d, "id": t, "op": "header", "on": "yes"}),
    ] {
        assert!(s.execute("doc.table.change", &bad).is_err(), "{bad}");
    }
}
