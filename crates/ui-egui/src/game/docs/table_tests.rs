//! Tables in documents (G13.2): made from the `/` menu, written in cell by cell, Tab and Enter to move,
//! rows and columns added with the + bars or the cell menu.

use egui::{Event, Key, Modifiers, PointerButton};
use figcraft_doc::docs::{BlockKind, Table};
use serde_json::json;

use super::blocks_tests::{blocks, click, key, key_with, open, settle, shapes, type_text};
use crate::layers_drag_tests::Ui;
use crate::live_tests::text_center;

fn table(u: &Ui, doc: u64) -> Table {
    blocks(u, doc)
        .into_iter()
        .find_map(|b| match b.kind {
            BlockKind::Table { table } => Some(table),
            _ => None,
        })
        .expect("a table in the document")
}

fn cells(t: &Table) -> Vec<Vec<String>> {
    (0..t.rows.len()).map(|r| (0..t.width()).map(|c| t.cell_text(r, c)).collect()).collect()
}

/// A document with one table of `rows` (each a list of cell words).
fn with_table(rows: &[&[&str]]) -> (Ui, u64) {
    let (mut u, doc) = open(&[]);
    let rows: Vec<Vec<Vec<serde_json::Value>>> =
        rows.iter().map(|r| r.iter().map(|c| if c.is_empty() { vec![] } else { vec![json!({"text": c})] }).collect()).collect();
    u.app.run("doc.block.insert", json!({"doc": doc, "kind": {"type": "table", "table": {"rows": rows, "header": true}}}));
    for _ in 0..3 {
        u.frame(vec![]);
    }
    (u, doc)
}

#[test]
fn slash_table_makes_a_three_by_three_table_and_writing_starts_in_its_first_cell() {
    let (mut u, doc) = open(&[("paragraph", "")]);
    let at = text_center(&shapes(&mut u), "Type “/” for blocks");
    click(&mut u, at);
    type_text(&mut u, "/table");
    key(&mut u, Key::Enter);
    let t = table(&u, doc);
    assert_eq!((t.rows.len(), t.width(), t.header), (3, 3, true));
    type_text(&mut u, "Stat");
    settle(&mut u);
    assert_eq!(cells(&table(&u, doc))[0], ["Stat", "", ""]);
}

#[test]
fn tab_goes_to_the_next_cell_and_from_the_last_one_adds_a_row() {
    let (mut u, doc) = with_table(&[&["Stat", "Value"], &["HP", ""]]);
    let at = text_center(&shapes(&mut u), "HP");
    click(&mut u, at);
    key(&mut u, Key::End);
    key(&mut u, Key::Tab);
    type_text(&mut u, "100");
    key(&mut u, Key::Tab);
    type_text(&mut u, "MP");
    settle(&mut u);
    assert_eq!(cells(&table(&u, doc)), [["Stat", "Value"], ["HP", "100"], ["MP", ""]]);
    // Shift+Tab goes back.
    key_with(&mut u, Key::Tab, Modifiers::SHIFT);
    type_text(&mut u, "!");
    settle(&mut u);
    assert_eq!(cells(&table(&u, doc))[1], ["HP", "100!"]);
}

#[test]
fn enter_goes_down_a_cell_and_adds_a_row_at_the_bottom() {
    let (mut u, doc) = with_table(&[&["Stat", "Value"], &["HP", "100"]]);
    let at = text_center(&shapes(&mut u), "Value");
    click(&mut u, at);
    key(&mut u, Key::Enter);
    key(&mut u, Key::Enter);
    type_text(&mut u, "50");
    settle(&mut u);
    assert_eq!(cells(&table(&u, doc)), [["Stat", "Value"], ["HP", "100"], ["", "50"]]);
}

#[test]
fn format_shortcuts_in_a_cell_never_delete_its_words() {
    let (mut u, doc) = with_table(&[&["Stat", "Value"], &["HP bar", ""]]);
    let at = text_center(&shapes(&mut u), "HP bar");
    click(&mut u, at);
    key(&mut u, Key::End);
    // egui's own Ctrl+U and Ctrl+K delete to the line start and end. Ctrl on Windows is both.
    let ctrl = Modifiers::CTRL | Modifiers::COMMAND;
    key_with(&mut u, Key::U, ctrl);
    key_with(&mut u, Key::U, ctrl | Modifiers::SHIFT);
    key(&mut u, Key::Home);
    key_with(&mut u, Key::K, ctrl);
    key_with(&mut u, Key::K, ctrl | Modifiers::SHIFT);
    settle(&mut u);
    assert_eq!(cells(&table(&u, doc))[1], ["HP bar", ""]);
}

#[test]
fn the_plus_bars_add_a_row_and_a_column() {
    let (mut u, doc) = with_table(&[&["a", "b"]]);
    let a = text_center(&shapes(&mut u), "a");
    u.frame(vec![Event::PointerMoved(a)]);
    u.frame(vec![]);
    let row = super::table::last_bar("row").expect("a + bar under the table");
    click(&mut u, row.center());
    u.frame(vec![Event::PointerMoved(a)]);
    u.frame(vec![]);
    let column = super::table::last_bar("column").expect("a + bar right of the table");
    click(&mut u, column.center());
    assert_eq!(cells(&table(&u, doc)), [["a", "b", ""], ["", "", ""]]);
}

#[test]
fn the_cell_menu_deletes_a_column_and_switches_the_header() {
    let (mut u, doc) = with_table(&[&["Stat", "Value", "Note"], &["HP", "100", "max"]]);
    let at = text_center(&shapes(&mut u), "Note");
    let right = |pressed| Event::PointerButton { pos: at, button: PointerButton::Secondary, pressed, modifiers: Modifiers::NONE };
    u.frame(vec![Event::PointerMoved(at), right(true)]);
    u.frame(vec![right(false)]);
    u.frame(vec![]);
    let item = text_center(&shapes(&mut u), "Delete column");
    click(&mut u, item);
    assert_eq!(cells(&table(&u, doc)), [["Stat", "Value"], ["HP", "100"]]);
    let at = text_center(&shapes(&mut u), "Value");
    let right = |pressed| Event::PointerButton { pos: at, button: PointerButton::Secondary, pressed, modifiers: Modifiers::NONE };
    u.frame(vec![Event::PointerMoved(at), right(true)]);
    u.frame(vec![right(false)]);
    u.frame(vec![]);
    let item = text_center(&shapes(&mut u), "✓ Header row");
    click(&mut u, item);
    assert!(!table(&u, doc).header);
}
