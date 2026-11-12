//! Pixel mode driven with raw input like a person would: entering it, drawing with the pencil (one
//! undo step), the secondary colour, the bucket, the eyedropper, the tool keys and leaving.

use egui::{Event, Key, Modifiers, PointerButton, Pos2};
use figcraft_engine::Session;
use figcraft_pixel::store;
use serde_json::json;

use super::{Mode, Tool};
use crate::layers_drag_tests::Ui;

/// A 16×16 sprite at scale 8, edited in Pixel mode; its id.
fn editing() -> (Ui, u64) {
    let mut s = Session::new();
    let id = s.execute("pixel.create", &json!({"x": 0, "y": 0, "width": 16, "height": 16, "scale": 8})).unwrap()["id"].as_u64().unwrap();
    let mut u = Ui::new(s);
    for _ in 0..3 {
        u.frame(vec![]);
    }
    u.app.run("pixel.mode", json!({"id": id}));
    for _ in 0..2 {
        u.frame(vec![]);
    }
    (u, id)
}

/// The screen point in the middle of art pixel (`x`, `y`).
fn at(u: &Ui, x: f64, y: f64) -> Pos2 {
    u.app.to_screen(x * 8.0 + 4.0, y * 8.0 + 4.0)
}

fn pixel(u: &mut Ui, id: u64, x: i64, y: i64) -> Option<String> {
    let r = u.app.session.execute("pixel.pick", &json!({"id": id, "x": x, "y": y})).unwrap();
    r["color"].as_str().map(str::to_string)
}

fn press(u: &mut Ui, p: Pos2, button: PointerButton, pressed: bool) {
    u.frame(vec![Event::PointerMoved(p), Event::PointerButton { pos: p, button, pressed, modifiers: Modifiers::NONE }]);
}

fn key(u: &mut Ui, k: Key, shift: bool) {
    let modifiers = if shift { Modifiers::SHIFT } else { Modifiers::NONE };
    u.frame(vec![Event::Key { key: k, physical_key: None, pressed: true, repeat: false, modifiers }]);
    u.frame(vec![Event::Key { key: k, physical_key: None, pressed: false, repeat: false, modifiers }]);
}

fn undo_labels(u: &Ui) -> Vec<String> {
    u.app.session.undo_labels(1000)
}

#[test]
fn a_drag_with_the_pencil_draws_one_undo_step() {
    let (mut u, id) = editing();
    assert_eq!(u.app.pixel.mode, Mode::Edit(figcraft_doc::NodeId(id)));
    let before = undo_labels(&u).len();
    let (a, b) = (at(&u, 1.0, 1.0), at(&u, 6.0, 1.0));
    u.frame(vec![Event::PointerMoved(a)]);
    press(&mut u, a, PointerButton::Primary, true);
    for i in 1..=5 {
        u.frame(vec![Event::PointerMoved(a + (b - a) * (i as f32 / 5.0))]);
    }
    press(&mut u, b, PointerButton::Primary, false);
    u.frame(vec![]);
    for x in 1..=6 {
        assert_eq!(pixel(&mut u, id, x, 1).as_deref(), Some("#1a1c2c"), "pixel {x}");
    }
    assert_eq!(pixel(&mut u, id, 7, 1), None);
    assert_eq!(undo_labels(&u).len(), before + 1, "one stroke, one undo step");
}

#[test]
fn the_right_button_draws_the_secondary_colour_and_the_bucket_fills() {
    let (mut u, id) = editing();
    let p = at(&u, 3.0, 3.0);
    u.frame(vec![Event::PointerMoved(p)]);
    press(&mut u, p, PointerButton::Secondary, true);
    press(&mut u, p, PointerButton::Secondary, false);
    u.frame(vec![]);
    assert_eq!(pixel(&mut u, id, 3, 3).as_deref(), Some("#f4f4f4"));
    key(&mut u, Key::G, false);
    assert_eq!(u.app.pixel.tool, Tool::Bucket);
    let q = at(&u, 10.0, 10.0);
    u.frame(vec![Event::PointerMoved(q)]);
    press(&mut u, q, PointerButton::Primary, true);
    press(&mut u, q, PointerButton::Primary, false);
    u.frame(vec![]);
    assert_eq!(pixel(&mut u, id, 0, 0).as_deref(), Some("#1a1c2c"));
    assert_eq!(pixel(&mut u, id, 3, 3).as_deref(), Some("#f4f4f4"), "the other colour is a wall");
}

#[test]
fn keys_pick_tools_swap_colours_and_size_the_brush() {
    let (mut u, _) = editing();
    for (k, shift, tool) in [
        (Key::E, false, Tool::Eraser),
        (Key::L, false, Tool::Line),
        (Key::U, false, Tool::Rect),
        (Key::U, true, Tool::Ellipse),
        (Key::I, false, Tool::Eyedropper),
        (Key::B, false, Tool::Pencil),
    ] {
        key(&mut u, k, shift);
        assert_eq!(u.app.pixel.tool, tool);
    }
    let colors = u.app.pixel.colors;
    key(&mut u, Key::X, false);
    assert_eq!(u.app.pixel.colors, [colors[1], colors[0]]);
    key(&mut u, Key::CloseBracket, false);
    key(&mut u, Key::CloseBracket, false);
    assert_eq!(u.app.pixel.brush.size, 3);
    key(&mut u, Key::OpenBracket, false);
    assert_eq!(u.app.pixel.brush.size, 2);
    key(&mut u, Key::Escape, false);
    assert_eq!(u.app.pixel.mode, Mode::Off, "Esc leaves Pixel mode");
}

#[test]
fn the_eyedropper_picks_and_a_shape_drag_draws_a_rectangle() {
    let (mut u, id) = editing();
    u.app.session.execute("pixel.stroke", &json!({"id": id, "points": [[5, 5]], "color": "#00ff00"})).unwrap();
    u.frame(vec![]);
    key(&mut u, Key::I, false);
    let p = at(&u, 5.0, 5.0);
    u.frame(vec![Event::PointerMoved(p)]);
    press(&mut u, p, PointerButton::Primary, true);
    press(&mut u, p, PointerButton::Primary, false);
    assert_eq!(u.app.pixel.colors[0], [0, 255, 0, 255]);
    key(&mut u, Key::U, false);
    let (a, b) = (at(&u, 8.0, 8.0), at(&u, 12.0, 10.0));
    u.frame(vec![Event::PointerMoved(a)]);
    press(&mut u, a, PointerButton::Primary, true);
    u.frame(vec![Event::PointerMoved(b)]);
    press(&mut u, b, PointerButton::Primary, false);
    u.frame(vec![]);
    assert_eq!(pixel(&mut u, id, 8, 8).as_deref(), Some("#00ff00"));
    assert_eq!(pixel(&mut u, id, 12, 10).as_deref(), Some("#00ff00"));
    assert_eq!(pixel(&mut u, id, 10, 9), None, "an outline, not filled");
}

#[test]
fn the_mode_switcher_without_a_pixel_layer_makes_a_new_sprite_by_dragging() {
    let mut u = Ui::new(Session::new());
    for _ in 0..3 {
        u.frame(vec![]);
    }
    super::enter_from_switcher(&mut u.app);
    u.frame(vec![]);
    assert_eq!(u.app.pixel.mode, Mode::New);
    // 64×32 page units at scale 4: a 16×8 sprite.
    let (a, b) = (u.app.to_screen(0.0, 0.0), u.app.to_screen(64.0, 32.0));
    u.frame(vec![Event::PointerMoved(a)]);
    press(&mut u, a, PointerButton::Primary, true);
    for i in 1..=4 {
        u.frame(vec![Event::PointerMoved(a + (b - a) * (i as f32 / 4.0))]);
    }
    press(&mut u, b, PointerButton::Primary, false);
    u.frame(vec![]);
    let Mode::Edit(id) = u.app.pixel.mode else { panic!("editing the new sprite") };
    let doc = u.app.session.doc().unwrap();
    let s = store::load(&doc.node(id).unwrap().plugin_data).unwrap();
    assert_eq!((s.width, s.height, s.scale), (16, 8, 4));
}

#[test]
fn deleting_the_sprite_leaves_pixel_mode() {
    let (mut u, id) = editing();
    u.app.run("edit.delete", json!({"ids": [id]}));
    u.frame(vec![]);
    assert_eq!(u.app.pixel.mode, Mode::Off);
}

#[test]
fn a_stroke_released_off_the_canvas_still_draws() {
    let (mut u, id) = editing();
    let before = undo_labels(&u).len();
    let a = at(&u, 2.0, 4.0);
    u.frame(vec![Event::PointerMoved(a)]);
    press(&mut u, a, PointerButton::Primary, true);
    u.frame(vec![Event::PointerMoved(at(&u, 5.0, 4.0))]);
    // Out over the right panel, and let go there.
    let out = Pos2::new(u.app.canvas.rect.right() + 30.0, a.y);
    u.frame(vec![Event::PointerMoved(out)]);
    press(&mut u, out, PointerButton::Primary, false);
    u.frame(vec![]);
    assert!(u.app.pixel.gesture.is_none(), "the stroke ended");
    for x in 2..=5 {
        assert_eq!(pixel(&mut u, id, x, 4).as_deref(), Some("#1a1c2c"), "pixel {x}");
    }
    assert_eq!(undo_labels(&u).len(), before + 1);
}

#[test]
fn a_press_that_closes_the_colour_picker_does_not_draw() {
    let (mut u, id) = editing();
    u.app.pixel.picking = Some(0);
    u.frame(vec![]);
    let p = at(&u, 4.0, 4.0);
    u.frame(vec![Event::PointerMoved(p)]);
    press(&mut u, p, PointerButton::Primary, true);
    press(&mut u, p, PointerButton::Primary, false);
    u.frame(vec![]);
    assert_eq!(u.app.pixel.picking, None, "the picker closed");
    assert_eq!(pixel(&mut u, id, 4, 4), None, "and nothing was drawn");
    // The next press draws.
    press(&mut u, p, PointerButton::Primary, true);
    press(&mut u, p, PointerButton::Primary, false);
    u.frame(vec![]);
    assert_eq!(pixel(&mut u, id, 4, 4).as_deref(), Some("#1a1c2c"));
}

#[test]
fn holding_x_swaps_the_colours_once() {
    let (mut u, _) = editing();
    let colors = u.app.pixel.colors;
    let ev = |repeat| Event::Key { key: Key::X, physical_key: None, pressed: true, repeat, modifiers: Modifiers::NONE };
    u.frame(vec![ev(false)]);
    u.frame(vec![ev(true)]);
    u.frame(vec![ev(true)]);
    u.frame(vec![Event::Key { key: Key::X, physical_key: None, pressed: false, repeat: false, modifiers: Modifiers::NONE }]);
    assert_eq!(u.app.pixel.colors, [colors[1], colors[0]]);
}

#[test]
fn selecting_another_layer_moves_or_leaves_pixel_mode() {
    let (mut u, id) = editing();
    let other = u.app.session.execute("pixel.create", &json!({"x": 300, "y": 0, "width": 8, "height": 8})).unwrap()["id"].as_u64().unwrap();
    u.frame(vec![]);
    assert_eq!(u.app.pixel.mode, Mode::Edit(figcraft_doc::NodeId(other)), "another sprite selected: it is edited");
    u.app.session.execute("shape.rectangle", &json!({"x": 0, "y": 300, "width": 10, "height": 10})).unwrap();
    assert_ne!(other, id);
    u.frame(vec![]);
    assert_eq!(u.app.pixel.mode, Mode::Off, "a layer that isn't a sprite: Pixel mode ends");
}
