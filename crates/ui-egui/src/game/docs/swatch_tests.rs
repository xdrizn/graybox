//! Colour swatches and shapes in documents (G13.2): `/` adds them, the swatch shows its chip and
//! code beside its name, and a click on the chip or the shape offers other colours and shapes.

use egui::{Color32, Key, Pos2, Shape};
use figcraft_doc::docs::BlockKind;
use figcraft_engine::Session;
use serde_json::json;

use super::blocks_tests::{blocks, click, key, settle, shapes, type_text};
use crate::LeftTab;
use crate::layers_drag_tests::Ui;
use crate::live_tests::{has_text, text_center};

/// A document with one empty paragraph, open in the editor.
fn with_doc() -> (Ui, u64) {
    let mut s = Session::new();
    let doc = s.execute("doc.create", &json!({"title": "Palette"})).unwrap()["id"].as_u64().unwrap();
    s.execute("doc.block.insert", &json!({"doc": doc, "kind": "paragraph"})).unwrap();
    let mut u = Ui::new(s);
    u.app.ui.left_tab = LeftTab::Documents;
    u.app.ui.open_left_panel();
    super::open(&mut u.app, doc);
    for _ in 0..3 {
        u.frame(vec![]);
    }
    (u, doc)
}

fn first_kind(u: &Ui, doc: u64) -> BlockKind {
    blocks(u, doc).into_iter().next().map(|b| b.kind).unwrap()
}

fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// The middle of the first filled rectangle in `c`.
fn rect_in(s: &[Shape], c: Color32) -> Option<egui::Rect> {
    s.iter().find_map(|s| match s {
        Shape::Rect(r) if r.fill == c => Some(r.rect),
        _ => None,
    })
}

/// The middle of the first dot (filled circle) in `c`.
fn dot_in(s: &[Shape], c: Color32) -> Pos2 {
    s.iter()
        .find_map(|s| match s {
            Shape::Circle(d) if d.fill == c => Some(d.center),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no dot in {c:?}"))
}

fn ellipses(s: &[Shape], c: Color32) -> usize {
    s.iter().filter(|s| matches!(s, Shape::Ellipse(e) if e.fill == c)).count()
}

#[test]
fn slash_swatch_adds_a_chip_with_its_code_and_a_name() {
    let (mut u, doc) = with_doc();
    let at = text_center(&shapes(&mut u), "Type “/” for blocks");
    click(&mut u, at);
    type_text(&mut u, "/swatch");
    key(&mut u, Key::Enter);
    assert_eq!(first_kind(&u, doc), BlockKind::Swatch { color: "#3A7FD5".into() });
    let s = shapes(&mut u);
    assert!(has_text(&s, "#3A7FD5"), "the code shows");
    assert!(has_text(&s, "Colour name"), "the name is asked for");
    assert!(rect_in(&s, rgb(0x3A7FD5)).is_some(), "the chip shows");
    type_text(&mut u, "Primary");
    settle(&mut u);
    assert_eq!(blocks(&u, doc)[0].plain_text(), "Primary");
}

#[test]
fn a_click_on_the_chip_picks_another_colour() {
    let (mut u, doc) = with_doc();
    let para = blocks(&u, doc)[0].id.0;
    u.app.run("doc.block.insert", json!({"doc": doc, "kind": {"type": "swatch", "color": "#3A7FD5"}}));
    u.app.run("doc.block.delete", json!({"doc": doc, "id": para}));
    let s = shapes(&mut u);
    let chip = rect_in(&s, rgb(0x3A7FD5)).expect("a chip");
    click(&mut u, chip.center());
    let red = dot_in(&shown(&mut u), rgb(0xD9483B));
    click(&mut u, red);
    assert_eq!(first_kind(&u, doc), BlockKind::Swatch { color: "#D9483B".into() });
    assert!(has_text(&shapes(&mut u), "#D9483B"));
}

#[test]
fn slash_shape_draws_a_rectangle_and_a_click_turns_it_into_an_ellipse_in_another_fill() {
    let (mut u, doc) = with_doc();
    let at = text_center(&shapes(&mut u), "Type “/” for blocks");
    click(&mut u, at);
    type_text(&mut u, "/shape");
    key(&mut u, Key::Enter);
    let blue = BlockKind::Shape { shape: "rectangle".into(), fill: Some("#3A7FD5".into()) };
    assert_eq!(first_kind(&u, doc), blue);
    let s = shapes(&mut u);
    let r = rect_in(&s, rgb(0x3A7FD5)).expect("the rectangle");
    assert_eq!((r.width(), r.height()), (120.0, 60.0));
    click(&mut u, r.center());
    let item = text_center(&shown(&mut u), "    Ellipse");
    click(&mut u, item);
    assert!(matches!(first_kind(&u, doc), BlockKind::Shape { shape, .. } if shape == "ellipse"));
    let s = shapes(&mut u);
    assert_eq!(ellipses(&s, rgb(0x3A7FD5)), 1, "drawn as an ellipse");
    // Its fill: the ellipse's middle opens the menu again.
    let mid = s
        .iter()
        .find_map(|s| match s {
            Shape::Ellipse(e) => Some(e.center),
            _ => None,
        })
        .unwrap();
    click(&mut u, mid);
    let green = dot_in(&shown(&mut u), rgb(0x3E9A62));
    click(&mut u, green);
    assert_eq!(first_kind(&u, doc), BlockKind::Shape { shape: "ellipse".into(), fill: Some("#3E9A62".into()) });
}

/// What is drawn once a menu that just opened has faded in.
fn shown(u: &mut Ui) -> Vec<Shape> {
    for _ in 0..20 {
        u.frame(vec![]);
    }
    shapes(u)
}
