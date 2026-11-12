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
        assert_eq!(pixel(&mut u, id, x, 1).as_deref(), Some("#1b1d29"), "pixel {x}");
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
    assert_eq!(pixel(&mut u, id, 3, 3).as_deref(), Some("#f3f1ea"));
    key(&mut u, Key::G, false);
    assert_eq!(u.app.pixel.tool, Tool::Bucket);
    let q = at(&u, 10.0, 10.0);
    u.frame(vec![Event::PointerMoved(q)]);
    press(&mut u, q, PointerButton::Primary, true);
    press(&mut u, q, PointerButton::Primary, false);
    u.frame(vec![]);
    assert_eq!(pixel(&mut u, id, 0, 0).as_deref(), Some("#1b1d29"));
    assert_eq!(pixel(&mut u, id, 3, 3).as_deref(), Some("#f3f1ea"), "the other colour is a wall");
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
        assert_eq!(pixel(&mut u, id, x, 4).as_deref(), Some("#1b1d29"), "pixel {x}");
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
    assert_eq!(pixel(&mut u, id, 4, 4).as_deref(), Some("#1b1d29"));
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

// G16.2: selection, moving pixels, copy and paste.

/// A drag from art pixel `a` to `b` with the left button.
fn drag(u: &mut Ui, a: (f64, f64), b: (f64, f64)) {
    let (p, q) = (at(u, a.0, a.1), at(u, b.0, b.1));
    u.frame(vec![Event::PointerMoved(p)]);
    press(u, p, PointerButton::Primary, true);
    for i in 1..=4 {
        u.frame(vec![Event::PointerMoved(p + (q - p) * (i as f32 / 4.0))]);
    }
    press(u, q, PointerButton::Primary, false);
    u.frame(vec![]);
}

fn selected(u: &mut Ui, id: u64) -> Option<figcraft_pixel::Mask> {
    let doc = u.app.session.doc().unwrap();
    let n = doc.node(figcraft_doc::NodeId(id)).unwrap();
    let sprite = store::load(&n.plugin_data).unwrap();
    store::selection(&n.plugin_data, &sprite)
}

#[test]
fn the_selection_keys_pick_their_tools() {
    let (mut u, _) = editing();
    for (k, tool) in [(Key::M, Tool::Marquee), (Key::Q, Tool::Lasso), (Key::W, Tool::Wand), (Key::V, Tool::Move), (Key::R, Tool::Replace)] {
        key(&mut u, k, false);
        assert_eq!(u.app.pixel.tool, tool);
    }
}

#[test]
fn a_marquee_drag_selects_and_strokes_stay_inside_until_esc() {
    let (mut u, id) = editing();
    key(&mut u, Key::M, false);
    drag(&mut u, (2.0, 0.0), (4.0, 15.0));
    assert_eq!(selected(&mut u, id).map(|m| m.count()), Some(3 * 16));
    key(&mut u, Key::B, false);
    drag(&mut u, (0.0, 1.0), (8.0, 1.0));
    for x in 0..=8 {
        assert_eq!(pixel(&mut u, id, x, 1).is_some(), (2..=4).contains(&x), "pixel {x}");
    }
    key(&mut u, Key::Escape, false);
    assert!(selected(&mut u, id).is_none(), "the first Esc lets go of the selection");
    assert!(matches!(u.app.pixel.mode, Mode::Edit(_)), "and stays in Pixel mode");
    key(&mut u, Key::Escape, false);
    assert_eq!(u.app.pixel.mode, Mode::Off);
}

#[test]
fn the_move_tool_drags_the_selected_pixels_in_one_undo_step() {
    let (mut u, id) = editing();
    u.app.session.execute("pixel.stroke", &json!({"id": id, "points": [[1, 1], [2, 1]], "color": "#ff0000"})).unwrap();
    u.app.session.execute("pixel.select", &json!({"id": id, "kind": "rect", "from": [1, 1], "to": [2, 1]})).unwrap();
    u.frame(vec![]);
    let before = undo_labels(&u).len();
    key(&mut u, Key::V, false);
    drag(&mut u, (1.0, 1.0), (5.0, 3.0));
    assert_eq!(pixel(&mut u, id, 1, 1), None);
    assert_eq!(pixel(&mut u, id, 5, 3).as_deref(), Some("#ff0000"));
    assert_eq!(pixel(&mut u, id, 6, 3).as_deref(), Some("#ff0000"));
    assert_eq!(undo_labels(&u).len(), before + 1);
    key(&mut u, Key::ArrowRight, false);
    assert_eq!(pixel(&mut u, id, 7, 3).as_deref(), Some("#ff0000"), "an arrow nudges them a pixel");
}

#[test]
fn copy_and_paste_events_copy_and_paste_pixels_once() {
    let (mut u, id) = editing();
    u.app.session.execute("pixel.stroke", &json!({"id": id, "points": [[0, 0]], "color": "#00ff00"})).unwrap();
    u.app.session.execute("pixel.select", &json!({"id": id, "kind": "rect", "from": [0, 0], "to": [0, 0]})).unwrap();
    u.frame(vec![]);
    u.frame(vec![Event::Copy]);
    assert!(u.app.pixel.clipboard.is_some());
    let before = undo_labels(&u).len();
    let v = Event::Key { key: Key::V, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::COMMAND };
    u.frame(vec![v, Event::Paste(String::new())]);
    assert_eq!(undo_labels(&u).len(), before + 1, "the key and the browser's paste event paste once");
    assert_eq!(u.app.pixel.tool, Tool::Move, "the paste is ready to move");
    u.frame(vec![Event::Cut]);
    assert_eq!(pixel(&mut u, id, 0, 0), None, "cut clears the pasted copy's pixels");
    assert!(u.app.session.doc().unwrap().node(figcraft_doc::NodeId(id)).is_some(), "and not the layer");
}

#[test]
fn delete_clears_only_the_selected_pixels() {
    let (mut u, id) = editing();
    u.app.session.execute("pixel.stroke", &json!({"id": id, "points": [[0, 0], [3, 0]]})).unwrap();
    u.app.session.execute("pixel.select", &json!({"id": id, "kind": "rect", "from": [0, 0], "to": [1, 0]})).unwrap();
    u.frame(vec![]);
    key(&mut u, Key::Delete, false);
    assert_eq!((pixel(&mut u, id, 1, 0), pixel(&mut u, id, 2, 0).is_some()), (None, true));
}

#[test]
fn an_image_paste_after_copying_pixels_pastes_them_into_the_sprite_not_onto_the_page() {
    let (mut u, id) = editing();
    u.app.session.execute("pixel.stroke", &json!({"id": id, "points": [[4, 4]], "color": "#00ff00"})).unwrap();
    u.frame(vec![]);
    u.frame(vec![Event::Copy]);
    let layers = u.app.session.doc().unwrap().nodes().count();
    let before = undo_labels(&u).len();
    u.frame(vec![Event::PasteImage(std::sync::Arc::default())]);
    assert_eq!(u.app.session.doc().unwrap().nodes().count(), layers, "no new image layer");
    assert_eq!(undo_labels(&u).len(), before + 1, "one paste into the sprite");
    assert!(selected(&mut u, id).is_some(), "the pasted pixels are selected");
}

#[test]
fn the_pixel_button_in_the_new_pill_makes_the_component_set_and_leaves_pixel_mode() {
    let mut u = Ui::new(Session::new());
    for _ in 0..3 {
        u.frame(vec![]);
    }
    super::enter_from_switcher(&mut u.app);
    u.frame(vec![]);
    assert_eq!(u.app.pixel.mode, Mode::New);
    super::pill::button_in_view(&mut u.app);
    u.frame(vec![]);
    assert_eq!(u.app.pixel.mode, Mode::Off);
    let doc = u.app.session.doc().unwrap();
    let sel = u.app.session.selection();
    let [set] = sel[..] else { panic!("the set is selected: {sel:?}") };
    assert_eq!(doc.node(set).unwrap().name, "Pixel button");
    assert_eq!(doc.node(set).unwrap().children.len(), 3);
}

fn pixel_in(u: &mut Ui, id: u64, frame: usize, x: i64, y: i64) -> Option<String> {
    let r = u.app.session.execute("pixel.pick", &json!({"id": id, "x": x, "y": y, "frame": frame})).unwrap();
    r["color"].as_str().map(str::to_string)
}

fn click(u: &mut Ui, p: Pos2) {
    u.frame(vec![Event::PointerMoved(p)]);
    press(u, p, PointerButton::Primary, true);
    press(u, p, PointerButton::Primary, false);
    u.frame(vec![]);
}

#[test]
fn the_period_and_comma_keys_step_through_frames_and_the_pencil_draws_on_the_frame_shown() {
    let (mut u, id) = editing();
    u.app.run("pixel.frame.add", json!({"id": id}));
    u.frame(vec![]);
    key(&mut u, Key::Period, false);
    assert_eq!(u.app.pixel.frame, 1);
    let p = at(&u, 2.0, 2.0);
    click(&mut u, p);
    assert_eq!(pixel_in(&mut u, id, 1, 2, 2).as_deref(), Some("#1b1d29"), "drawn in frame 2");
    assert_eq!(pixel_in(&mut u, id, 0, 2, 2), None, "frame 1 untouched");
    key(&mut u, Key::Period, false);
    assert_eq!(u.app.pixel.frame, 0, "past the last frame it wraps round");
    key(&mut u, Key::Comma, false);
    assert_eq!(u.app.pixel.frame, 1);
    // With the onion skin on, the frame before shows faintly (its picture is made once).
    u.app.pixel.onion = true;
    u.frame(vec![]);
    u.frame(vec![]);
    assert_eq!(u.app.pixel.onion_tex.len(), 1, "frame 1 is the only neighbour of frame 2");
}

#[test]
fn play_runs_the_frames_of_the_current_tag_and_any_click_in_the_timeline_stops_it() {
    let (mut u, id) = editing();
    for _ in 0..3 {
        u.app.run("pixel.frame.add", json!({"id": id}));
    }
    u.app.run("pixel.tag.add", json!({"id": id, "from": 2, "to": 3, "name": "blink"}));
    u.app.pixel.frame = 3;
    super::timeline::toggle_play(&mut u.app);
    assert_eq!(u.app.pixel.playing.as_ref().map(|p| p.seq.clone()), Some(vec![2, 3]), "the tag the frame is in");
    u.frame(vec![]);
    assert!(u.app.pixel.playing.as_ref().is_some_and(|p| p.since.is_some()), "the clock starts on the first frame drawn");
    assert!((2..=3).contains(&u.app.pixel.frame));
    key(&mut u, Key::Period, false);
    assert!(u.app.pixel.playing.is_none(), "stepping stops playing");
    u.app.pixel.frame = 0;
    super::timeline::toggle_play(&mut u.app);
    assert_eq!(u.app.pixel.playing.as_ref().map(|p| p.seq.len()), Some(4), "outside a tag: every frame");
    super::timeline::toggle_play(&mut u.app);
    assert!(u.app.pixel.playing.is_none());
}

#[test]
fn shade_mode_steps_along_the_palette_with_either_button() {
    let (mut u, id) = editing();
    u.app.session.execute("pixel.palette.set", &json!({"id": id, "colors": ["#1e143c", "#5a3c8c", "#c8aaf0"]})).unwrap();
    let rows = vec!["mmmm"; 2];
    u.app.session.execute("pixel.paint", &json!({"id": id, "palette": {"m": "#5a3c8c"}, "rows": rows})).unwrap();
    u.app.pixel.shade = true;
    drag(&mut u, (0.0, 0.0), (3.0, 0.0));
    let p = at(&u, 0.0, 1.0);
    u.frame(vec![Event::PointerMoved(p)]);
    press(&mut u, p, PointerButton::Secondary, true);
    press(&mut u, p, PointerButton::Secondary, false);
    u.frame(vec![]);
    assert_eq!(pixel(&mut u, id, 3, 0).as_deref(), Some("#c8aaf0"), "the main button: the next colour");
    assert_eq!(pixel(&mut u, id, 0, 1).as_deref(), Some("#1e143c"), "the other button: the one before");
    assert_eq!(pixel(&mut u, id, 1, 1).as_deref(), Some("#5a3c8c"));
    assert_eq!(pixel(&mut u, id, 5, 5), None, "shading never paints empty pixels");
    // Shade is the pencil's: the eraser still erases.
    key(&mut u, Key::E, false);
    drag(&mut u, (1.0, 1.0), (1.0, 1.0));
    assert_eq!(pixel(&mut u, id, 1, 1), None);
}
