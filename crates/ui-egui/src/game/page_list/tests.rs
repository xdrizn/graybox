//! The Pages list as a tree, driven with raw input like a person would: rows, chevrons, indentation,
//! drag positions, the row menu, deleting a page that has pages inside it.

use egui::{Event, Modifiers, PointerButton, Pos2, Shape};
use figcraft_engine::Session;
use serde_json::{Value, json};

use crate::game::tree_list::{INDENT, ROW_H};
use crate::layers_drag_tests::Ui;
use crate::live_tests::{flat, texts};

/// A file with the pages Alpha Beta Gamma Delta Epsilon, in that order; returns their ids.
fn five() -> (Session, [u64; 5]) {
    let mut s = Session::new();
    let first = s.page().unwrap().0;
    s.execute("page.rename", &json!({"id": first, "name": "Alpha"})).unwrap();
    let mut ids = [first, 0, 0, 0, 0];
    for (k, n) in ["Beta", "Gamma", "Delta", "Epsilon"].into_iter().enumerate() {
        ids[k + 1] = s.execute("page.add", &json!({"name": n})).unwrap()["id"].as_u64().unwrap();
    }
    s.execute("page.select", &json!({"id": first})).unwrap();
    (s, ids)
}

fn ui(s: Session) -> Ui {
    let mut u = Ui::new(s);
    for _ in 0..5 {
        u.frame(vec![]);
    }
    u
}

fn shapes(u: &mut Ui) -> Vec<Shape> {
    flat(&u.frame_output(vec![]).shapes)
}

/// Where the name `name` is drawn: its top-left and its size.
fn text_box(u: &mut Ui, name: &str) -> Option<(Pos2, egui::Vec2)> {
    texts(&shapes(u)).into_iter().find(|(t, _, _)| t == name).map(|(_, p, s)| (p, s))
}

/// The middle of the row of page `name` (its name's middle, a bit left of the row's).
fn at(u: &mut Ui, name: &str) -> Pos2 {
    let (p, s) = text_box(u, name).unwrap_or_else(|| panic!("no row {name}"));
    p + s / 2.0
}

fn click(u: &mut Ui, at: Pos2) {
    for pressed in [true, false] {
        u.frame(vec![Event::PointerMoved(at), Event::PointerButton { pos: at, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE }]);
    }
    u.frame(vec![]);
}

fn right_click(u: &mut Ui, at: Pos2) {
    for pressed in [true, false] {
        u.frame(vec![
            Event::PointerMoved(at),
            Event::PointerButton { pos: at, button: PointerButton::Secondary, pressed, modifiers: Modifiers::NONE },
        ]);
    }
    u.frame(vec![]);
}

/// Press on `from`, drag to `to` in steps and let go there.
fn drag(u: &mut Ui, from: Pos2, to: Pos2) {
    u.frame(vec![Event::PointerMoved(from)]);
    u.drag_to(from, to);
    u.button(to, false);
    for _ in 0..3 {
        u.frame(vec![]);
    }
}

/// `name:depth` of every page in the list order, and whether it is collapsed.
fn tree(u: &mut Ui) -> Vec<String> {
    let v = u.app.session.execute("page.tree", &json!({})).unwrap();
    v.as_array()
        .unwrap()
        .iter()
        .map(|p| format!("{}:{}{}", p["name"].as_str().unwrap(), p["depth"], if p["collapsed"] == true { "-" } else { "" }))
        .collect()
}

fn nest(u: &mut Ui, id: u64, parent: u64) {
    u.app.session.execute("page.nest", &json!({"id": id, "parent": parent})).unwrap();
    for _ in 0..3 {
        u.frame(vec![]);
    }
}

#[test]
fn rows_are_as_high_as_the_docs_rows_and_nested_ones_are_indented_like_them() {
    let (s, id) = five();
    let mut u = ui(s);
    nest(&mut u, id[1], id[0]);
    let (a, b, g) = (at(&mut u, "Alpha"), at(&mut u, "Beta"), at(&mut u, "Gamma"));
    assert_eq!(b.y - a.y, ROW_H, "rows are the Docs tree's height");
    assert_eq!(g.y - b.y, ROW_H);
    let left = |u: &mut Ui, n: &str| text_box(u, n).unwrap().0.x;
    assert_eq!(left(&mut u, "Beta") - left(&mut u, "Gamma"), INDENT, "one level in is the Docs tree's indent");
    // Only the page with pages inside it has a chevron (a small filled triangle on its row).
    let chevrons = |u: &mut Ui, y: f32| {
        shapes(u).iter().filter(|s| matches!(s, Shape::Path(p) if p.closed && p.points.len() == 3 && (p.points[0].y - y).abs() < 6.0)).count()
    };
    assert_eq!(chevrons(&mut u, a.y), 1, "Alpha has one");
    assert_eq!(chevrons(&mut u, g.y), 0, "Gamma has none");
}

#[test]
fn the_chevron_collapses_and_expands_and_the_file_remembers() {
    let (s, id) = five();
    let mut u = ui(s);
    nest(&mut u, id[1], id[0]);
    nest(&mut u, id[2], id[1]);
    assert_eq!(tree(&mut u), ["Alpha:0", "Beta:1", "Gamma:2", "Delta:0", "Epsilon:0"]);
    let chevron = |u: &mut Ui, name: &str| {
        let (p, s) = text_box(u, name).unwrap();
        Pos2::new(p.x - 34.0, p.y + s.y / 2.0)
    };
    // Alpha closes: Beta and Gamma are no longer listed, and the list keeps their order.
    let c = chevron(&mut u, "Alpha");
    click(&mut u, c);
    assert!(text_box(&mut u, "Beta").is_none() && text_box(&mut u, "Gamma").is_none(), "its pages are hidden");
    assert!(text_box(&mut u, "Delta").is_some());
    assert_eq!(tree(&mut u), ["Alpha:0-", "Beta:1", "Gamma:2", "Delta:0", "Epsilon:0"]);
    assert!(u.app.session.ds().unwrap().is_dirty(), "collapsing is saved with the file");
    let c = chevron(&mut u, "Alpha");
    click(&mut u, c);
    assert!(text_box(&mut u, "Gamma").is_some());
    assert_eq!(tree(&mut u)[0], "Alpha:0");
    // Beta closes on its own, Alpha stays open.
    let c = chevron(&mut u, "Beta");
    click(&mut u, c);
    assert!(text_box(&mut u, "Beta").is_some() && text_box(&mut u, "Gamma").is_none());
    assert_eq!(tree(&mut u)[1], "Beta:1-");
}

#[test]
fn going_to_a_page_inside_a_closed_page_opens_the_pages_above_it() {
    let (s, id) = five();
    let mut u = ui(s);
    nest(&mut u, id[1], id[0]);
    nest(&mut u, id[2], id[1]);
    u.app.session.execute("page.collapse", &json!({"id": id[0]})).unwrap();
    u.app.session.execute("page.collapse", &json!({"id": id[1]})).unwrap();
    for _ in 0..3 {
        u.frame(vec![]);
    }
    assert!(text_box(&mut u, "Gamma").is_none());
    // The next page shortcut from Alpha goes into the closed page and opens it.
    u.app.session.execute("view.nextPage", &json!({})).unwrap();
    for _ in 0..3 {
        u.frame(vec![]);
    }
    assert!(text_box(&mut u, "Beta").is_some(), "Alpha opened");
    // Arriving some other way (here: straight into the state, as undo does) opens them too.
    u.app.session.execute("page.collapse", &json!({"id": id[0]})).unwrap();
    u.app.session.ds_mut().unwrap().page = figcraft_doc::NodeId(id[2]);
    for _ in 0..5 {
        u.frame(vec![]);
    }
    assert!(text_box(&mut u, "Gamma").is_some(), "the list opened the pages above the open page");
    assert_eq!(tree(&mut u)[0], "Alpha:0");
}

#[test]
fn dropping_on_the_middle_of_a_row_goes_inside_and_on_its_edges_before_or_after() {
    let (s, id) = five();
    let mut u = ui(s);
    // Delta onto the middle of Alpha: inside it.
    let (from, to) = (at(&mut u, "Delta"), at(&mut u, "Alpha"));
    drag(&mut u, from, to);
    assert_eq!(tree(&mut u), ["Alpha:0", "Delta:1", "Beta:0", "Gamma:0", "Epsilon:0"]);
    // Epsilon onto the top edge of Beta: before it, at the top level.
    let beta = at(&mut u, "Beta");
    let from = at(&mut u, "Epsilon");
    drag(&mut u, from, beta - egui::vec2(0.0, ROW_H / 2.0 - 3.0));
    assert_eq!(tree(&mut u), ["Alpha:0", "Delta:1", "Epsilon:0", "Beta:0", "Gamma:0"]);
    // Gamma onto the bottom edge of Delta (inside Alpha): after it, inside Alpha too.
    let delta = at(&mut u, "Delta");
    let from = at(&mut u, "Gamma");
    drag(&mut u, from, delta + egui::vec2(0.0, ROW_H / 2.0 - 3.0));
    assert_eq!(tree(&mut u), ["Alpha:0", "Delta:1", "Gamma:1", "Epsilon:0", "Beta:0"]);
    // Delta back out: onto the bottom edge of Epsilon, a top-level row.
    let eps = at(&mut u, "Epsilon");
    let from = at(&mut u, "Delta");
    drag(&mut u, from, eps + egui::vec2(0.0, ROW_H / 2.0 - 3.0));
    assert_eq!(tree(&mut u), ["Alpha:0", "Gamma:1", "Epsilon:0", "Delta:0", "Beta:0"]);
    // One drop is one undo step.
    u.app.session.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(tree(&mut u), ["Alpha:0", "Delta:1", "Gamma:1", "Epsilon:0", "Beta:0"]);
    assert!(u.app.ui.last_error.is_none(), "{:?}", u.app.ui.last_error);
    let _ = id;
}

#[test]
fn a_page_cannot_be_dropped_inside_itself_or_its_own_subpages() {
    let (s, id) = five();
    let mut u = ui(s);
    nest(&mut u, id[1], id[0]);
    let before = tree(&mut u);
    let (alpha, beta) = (at(&mut u, "Alpha"), at(&mut u, "Beta"));
    drag(&mut u, alpha, beta);
    assert_eq!(tree(&mut u), before, "Alpha into Beta, which is inside it: nothing");
    drag(&mut u, alpha, alpha + egui::vec2(0.0, 2.0));
    assert_eq!(tree(&mut u), before, "onto itself: nothing");
    assert!(u.app.ui.last_error.is_none(), "{:?}", u.app.ui.last_error);
}

#[test]
fn escape_cancels_a_drag() {
    let (s, _) = five();
    let mut u = ui(s);
    let (from, to) = (at(&mut u, "Delta"), at(&mut u, "Alpha"));
    let before = tree(&mut u);
    u.frame(vec![Event::PointerMoved(from)]);
    u.drag_to(from, to);
    u.frame(vec![Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }]);
    u.button(to, false);
    for _ in 0..3 {
        u.frame(vec![]);
    }
    assert_eq!(tree(&mut u), before);
}

#[test]
fn dev_mode_does_not_nest_but_still_opens_and_collapses() {
    let (s, id) = five();
    let mut u = ui(s);
    nest(&mut u, id[1], id[0]);
    u.app.ui.right_tab = crate::RightTab::Inspect;
    u.frame(vec![]);
    let before = tree(&mut u);
    let (from, to) = (at(&mut u, "Delta"), at(&mut u, "Alpha"));
    drag(&mut u, from, to);
    assert_eq!(tree(&mut u), before, "no nesting in Dev Mode");
    assert!(u.app.ui.toast.take().is_some_and(|(m, _)| m.contains("Shift+D")), "and it says why");
    let (p, s) = text_box(&mut u, "Alpha").unwrap();
    click(&mut u, Pos2::new(p.x - 34.0, p.y + s.y / 2.0));
    assert!(text_box(&mut u, "Beta").is_none(), "the chevron still works");
}

/// Click the menu item `item` of the menu opened on the row `row`.
fn menu_item(u: &mut Ui, row: &str, item: &str) {
    let r = at(u, row);
    right_click(u, r);
    let it = crate::live_tests::text_center(&shapes(u), item);
    click(u, it);
}

#[test]
fn the_row_menu_adds_a_subpage_and_moves_a_nested_page_out() {
    let (s, id) = five();
    let mut u = ui(s);
    nest(&mut u, id[1], id[0]);
    menu_item(&mut u, "Beta", "Move out");
    assert_eq!(tree(&mut u), ["Alpha:0", "Beta:0", "Gamma:0", "Delta:0", "Epsilon:0"], "it lands right after the page it was in");
    // A page at the top level has no Move out.
    let r = at(&mut u, "Beta");
    right_click(&mut u, r);
    assert!(!texts(&shapes(&mut u)).iter().any(|(t, _, _)| t == "Move out"));
    u.frame(vec![Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }]);
    menu_item(&mut u, "Gamma", "Add subpage");
    let t = tree(&mut u);
    assert_eq!(t[2], "Gamma:0");
    assert!(t[3].ends_with(":1"), "a page inside Gamma: {t:?}");
    assert_eq!(u.app.session.doc().unwrap().page_parent(u.app.session.page().unwrap()), Some(figcraft_doc::NodeId(id[2])), "and it opens");
    u.app.session.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(tree(&mut u).len(), 5, "adding a subpage is one undo step");
}

#[test]
fn deleting_a_page_with_subpages_asks_and_offers_to_keep_them() {
    let (s, id) = five();
    let mut u = ui(s);
    nest(&mut u, id[1], id[0]);
    nest(&mut u, id[2], id[0]);
    // Cancel: nothing is deleted.
    menu_item(&mut u, "Alpha", "Delete page");
    assert_eq!(tree(&mut u).len(), 5, "it asked first");
    let sh = shapes(&mut u);
    assert!(crate::live_tests::has_text(&sh, "Keep the subpages") && crate::live_tests::has_text(&sh, "Delete with subpages"));
    let cancel = crate::live_tests::text_center(&sh, "Cancel");
    click(&mut u, cancel);
    assert_eq!(tree(&mut u).len(), 5);
    assert!(!crate::live_tests::has_text(&shapes(&mut u), "Keep the subpages"), "the question is gone");
    // Keep: Alpha goes, Beta and Gamma move up a level.
    menu_item(&mut u, "Alpha", "Delete page");
    let keep = crate::live_tests::text_center(&shapes(&mut u), "Keep the subpages");
    click(&mut u, keep);
    assert_eq!(tree(&mut u), ["Beta:0", "Gamma:0", "Delta:0", "Epsilon:0"]);
    u.app.session.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(tree(&mut u), ["Alpha:0", "Beta:1", "Gamma:1", "Delta:0", "Epsilon:0"]);
    // Delete with them.
    menu_item(&mut u, "Alpha", "Delete page");
    let all = crate::live_tests::text_center(&shapes(&mut u), "Delete with subpages");
    click(&mut u, all);
    assert_eq!(tree(&mut u), ["Delta:0", "Epsilon:0"]);
    // A page without subpages is deleted without a question.
    menu_item(&mut u, "Delta", "Delete page");
    assert_eq!(tree(&mut u), ["Epsilon:0"]);
    assert!(u.app.ui.last_error.is_none(), "{:?}", u.app.ui.last_error);
}

#[test]
fn the_delete_key_on_a_picked_page_with_subpages_asks_too() {
    let (s, id) = five();
    let mut u = ui(s);
    nest(&mut u, id[1], id[0]);
    let r = at(&mut u, "Alpha");
    click(&mut u, r);
    assert!(u.app.ui.pages_focus.is_some());
    u.frame(vec![Event::Key { key: egui::Key::Delete, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }]);
    for _ in 0..3 {
        u.frame(vec![]);
    }
    assert_eq!(tree(&mut u).len(), 5, "nothing deleted yet");
    assert!(crate::live_tests::has_text(&shapes(&mut u), "Delete with subpages"));
}

#[test]
fn searching_lists_the_matching_pages_flat_even_inside_closed_pages() {
    let (s, id) = five();
    let mut u = ui(s);
    nest(&mut u, id[1], id[0]);
    u.app.session.execute("page.collapse", &json!({"id": id[0]})).unwrap();
    u.app.ui.page_search = true;
    u.app.ui.page_filter = "bet".into();
    for _ in 0..3 {
        u.frame(vec![]);
    }
    let (b, g) = (text_box(&mut u, "Beta"), text_box(&mut u, "Gamma"));
    assert!(b.is_some(), "a page inside a closed page is found");
    assert!(g.is_none());
}

#[test]
fn old_files_open_flat_in_the_list() {
    let (s, _) = five();
    let mut u = ui(s);
    assert_eq!(tree(&mut u), ["Alpha:0", "Beta:0", "Gamma:0", "Delta:0", "Epsilon:0"]);
    let v: Value = u.app.session.execute("page.tree", &json!({})).unwrap();
    assert!(v.as_array().unwrap().iter().all(|p| p["parent"].is_null()));
}

#[test]
fn the_dots_button_opens_the_row_menu_and_the_plus_adds_a_subpage() {
    let (s, _) = five();
    let mut u = ui(s);
    let row = at(&mut u, "Beta");
    let dots = Pos2::new(273.0, row.y);
    u.frame(vec![Event::PointerMoved(row)]);
    u.frame(vec![Event::PointerMoved(dots)]);
    u.frame(vec![Event::PointerMoved(dots)]);
    click(&mut u, dots);
    assert!(texts(&shapes(&mut u)).iter().any(|(t, _, _)| t == "Add subpage"), "the menu is open");
    u.frame(vec![Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }]);
    for _ in 0..3 {
        u.frame(vec![]);
    }
    let plus = Pos2::new(249.0, row.y);
    u.frame(vec![Event::PointerMoved(plus)]);
    u.frame(vec![Event::PointerMoved(plus)]);
    click(&mut u, plus);
    assert_eq!(tree(&mut u).len(), 6, "a page was added inside Beta");
}
