//! Emoji on pages and documents: a double-click on a row's icon opens the picker, the pick replaces
//! the icon, and every emoji on offer is one the app's fonts can draw (no boxes).

use egui::{Event, Modifiers, PointerButton, Pos2, vec2};
use figcraft_engine::Session;
use serde_json::json;

use super::emoji_picker::{GROUPS, can_draw};
use crate::LeftTab;
use crate::layers_drag_tests::Ui;
use crate::live_tests::{flat, has_text, texts};

fn shapes(u: &mut Ui) -> Vec<egui::Shape> {
    flat(&u.frame_output(vec![]).shapes)
}

fn press(u: &mut Ui, at: Pos2, pressed: bool) {
    u.frame(vec![Event::PointerMoved(at), Event::PointerButton { pos: at, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE }]);
}

fn double_click(u: &mut Ui, at: Pos2) {
    for _ in 0..2 {
        press(u, at, true);
        press(u, at, false);
    }
    u.frame(vec![]);
}

fn click(u: &mut Ui, at: Pos2) {
    press(u, at, true);
    press(u, at, false);
    u.frame(vec![]);
}

/// The centre of the icon of the row whose title is `title` (the icon sits 14 px left of the title).
fn icon_of(sh: &[egui::Shape], title: &str) -> Pos2 {
    let (_, p, size) = texts(sh).into_iter().find(|(t, p, _)| t == title && p.x < 300.0).unwrap_or_else(|| panic!("no row {title:?}"));
    Pos2::new(p.x - 14.0, p.y + size.y / 2.0)
}

/// The middle of the emoji `e` drawn in the picker (the biggest copy on screen).
fn emoji_in_picker(sh: &[egui::Shape], e: &str) -> Pos2 {
    let (_, p, size) =
        texts(sh).into_iter().filter(|(t, _, _)| t == e).max_by(|a, b| a.2.y.total_cmp(&b.2.y)).unwrap_or_else(|| panic!("no {e:?} in the picker"));
    p + size / 2.0
}

fn settle(u: &mut Ui) {
    for _ in 0..3 {
        u.frame(vec![]);
    }
}

#[test]
fn every_emoji_on_offer_can_be_drawn_by_the_app() {
    let mut u = Ui::new(Session::new());
    settle(&mut u);
    let missing: Vec<&str> = GROUPS.iter().flat_map(|(_, e)| e.iter().copied()).filter(|e| !can_draw(&u.ctx, e)).collect();
    assert!(missing.is_empty(), "these would show as boxes: {missing:?}");
}

#[test]
fn double_clicking_a_page_icon_picks_an_emoji_that_replaces_the_icon() {
    let mut s = Session::new();
    s.execute("page.rename", &json!({"name": "Menus"})).unwrap();
    let page = s.page().unwrap();
    let mut u = Ui::new(s);
    settle(&mut u);
    let sh = shapes(&mut u);
    let icon = icon_of(&sh, "Menus");
    u.frame(vec![Event::PointerMoved(icon)]);
    double_click(&mut u, icon);
    assert!(u.app.ui.rename_page.is_none(), "a double-click on the icon does not rename");
    let sh = shapes(&mut u);
    assert!(has_text(&sh, "Nature"), "the picker shows its groups");
    let at = emoji_in_picker(&sh, "🎮");
    click(&mut u, at);
    assert_eq!(u.app.session.doc().unwrap().page_icon(page), Some("🎮"));
    assert_eq!(u.app.session.doc().unwrap().node(page).unwrap().name, "Menus", "the name stays");
    let sh = shapes(&mut u);
    assert!(!has_text(&sh, "Nature"), "the picker closed");
    assert!(has_text(&sh, "🎮"), "the row wears the emoji");
    // A double-click on the name still renames (a moment later, so it is not taken with the pick).
    u.advance_clock(1.0);
    let name = icon + vec2(40.0, 0.0);
    double_click(&mut u, name);
    assert_eq!(u.app.ui.rename_page.as_ref().map(|r| r.0), Some(page));
}

#[test]
fn a_page_icon_the_fonts_cannot_draw_shows_the_plain_page_icon() {
    let mut s = Session::new();
    s.execute("page.rename", &json!({"name": "Menus"})).unwrap();
    // An Egyptian hieroglyph: the app's fonts do not have it.
    s.execute("page.set_icon", &json!({"icon": "\u{13000}"})).unwrap();
    let mut u = Ui::new(s);
    settle(&mut u);
    assert!(!can_draw(&u.ctx, "\u{13000}"));
    let sh = shapes(&mut u);
    assert!(has_text(&sh, "Menus"));
    assert!(!has_text(&sh, "\u{13000}"), "never a box");
}

#[test]
fn double_clicking_a_document_icon_uses_the_same_picker() {
    let mut s = Session::new();
    let doc = s.execute("doc.create", &json!({"title": "Spec"})).unwrap()["id"].as_u64().unwrap();
    let mut u = Ui::new(s);
    u.app.ui.left_tab = LeftTab::Documents;
    u.app.ui.open_left_panel();
    settle(&mut u);
    let sh = shapes(&mut u);
    let icon = icon_of(&sh, "Spec");
    u.frame(vec![Event::PointerMoved(icon)]);
    double_click(&mut u, icon);
    let sh = shapes(&mut u);
    assert!(has_text(&sh, "Nature"), "the picker shows its groups");
    let at = emoji_in_picker(&sh, "📖");
    click(&mut u, at);
    let got = u.app.session.execute("doc.get", &json!({"id": doc})).unwrap();
    assert_eq!(got["icon"].as_str(), Some("📖"));
}
