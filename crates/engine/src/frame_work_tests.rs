//! Graybox (G9): working inside a top-level frame. With the frame selected (or a layer in it
//! selected, or the view zoomed onto it) a press on a layer in it selects that layer and a drag on
//! its empty background draws a marquee of the layers inside: neither moves the frame. The frame
//! moves by its name label, and an empty frame (or a widget) is still picked and dragged by its
//! body. The frame shows no hover outline over its background while it is worked in.

use figcraft_doc::NodeId;
use figcraft_geom::Point;
use serde_json::{Value, json};

use crate::Session;
use crate::params::id_of;
use crate::tools::{PointerEvent, PointerKind};

fn ex(s: &mut Session, c: &str, p: Value) -> Value {
    s.execute(c, &p).unwrap()
}

fn id(v: &Value) -> NodeId {
    id_of(&v["id"]).unwrap()
}

fn ev(kind: PointerKind, x: f64, y: f64) -> PointerEvent {
    PointerEvent::at(kind, x, y, 1.0)
}

fn drag(s: &mut Session, a: (f64, f64), b: (f64, f64)) {
    s.pointer(&ev(PointerKind::Down, a.0, a.1)).unwrap();
    s.pointer(&ev(PointerKind::Move, b.0, b.1)).unwrap();
    s.pointer(&ev(PointerKind::Up, b.0, b.1)).unwrap();
}

fn click(s: &mut Session, p: (f64, f64)) {
    s.pointer(&ev(PointerKind::Down, p.0, p.1)).unwrap();
    s.pointer(&ev(PointerKind::Up, p.0, p.1)).unwrap();
}

fn hover(s: &mut Session, p: (f64, f64)) -> Option<NodeId> {
    s.pointer(&ev(PointerKind::Hover, p.0, p.1)).unwrap();
    s.ds().unwrap().hover
}

fn origin(s: &Session, n: NodeId) -> Point {
    s.doc().unwrap().world_bounds(n).unwrap().origin()
}

struct Scene {
    /// Top-level frame (0,0 400×300).
    f: NodeId,
    /// Rectangles in it: `a` (20,20 40×40), `b` (120,20 40×40).
    a: NodeId,
    b: NodeId,
    /// A group of two rectangles at (20,120) and (80,120).
    g: NodeId,
    /// An empty top-level frame (600,0 80×80).
    empty: NodeId,
}

fn scene() -> (Session, Scene) {
    let mut s = Session::new();
    let f = id(&ex(&mut s, "frame.create", json!({"x": 0, "y": 0, "width": 400, "height": 300})));
    let rect = |s: &mut Session, x: f64, y: f64| id(&ex(s, "shape.rectangle", json!({"x": x, "y": y, "width": 40, "height": 40, "parent": f})));
    let a = rect(&mut s, 20.0, 20.0);
    let b = rect(&mut s, 120.0, 20.0);
    let g1 = rect(&mut s, 20.0, 120.0);
    let g2 = rect(&mut s, 80.0, 120.0);
    let g = id(&ex(&mut s, "object.group", json!({"ids": [g1, g2]})));
    let empty = id(&ex(&mut s, "frame.create", json!({"x": 600, "y": 0, "width": 80, "height": 80})));
    ex(&mut s, "select.none", json!({}));
    (s, Scene { f, a, b, g, empty })
}

#[test]
fn a_drag_on_the_background_of_the_selected_frame_draws_a_marquee_and_never_moves_it() {
    let (mut s, k) = scene();
    ex(&mut s, "select.set", json!({"ids": [k.f]}));
    let at = origin(&s, k.f);
    drag(&mut s, (300.0, 200.0), (10.0, 10.0));
    assert_eq!(origin(&s, k.f), at, "the frame stays put");
    let mut got = s.selection();
    got.sort();
    let mut want = vec![k.a, k.b, k.g];
    want.sort();
    assert_eq!(got, want, "the marquee selects the layers it touches inside the frame");
}

#[test]
fn a_drag_on_a_layer_while_the_frame_is_selected_moves_that_layer_only() {
    let (mut s, k) = scene();
    ex(&mut s, "select.set", json!({"ids": [k.f]}));
    let (frame_at, a_at) = (origin(&s, k.f), origin(&s, k.a));
    drag(&mut s, (40.0, 40.0), (70.0, 90.0));
    assert_eq!(origin(&s, k.f), frame_at);
    assert_eq!(s.selection(), vec![k.a], "the press selected the layer directly");
    let moved = origin(&s, k.a);
    assert!((moved.x - a_at.x - 30.0).abs() < 1e-6 && (moved.y - a_at.y - 50.0).abs() < 1e-6, "{a_at:?} → {moved:?}");
}

#[test]
fn a_click_on_a_layer_while_the_frame_is_selected_selects_it_without_cmd() {
    let (mut s, k) = scene();
    ex(&mut s, "select.set", json!({"ids": [k.f]}));
    click(&mut s, (140.0, 40.0));
    assert_eq!(s.selection(), vec![k.b]);
    // A group still selects as a whole.
    ex(&mut s, "select.set", json!({"ids": [k.f]}));
    click(&mut s, (30.0, 130.0));
    assert_eq!(s.selection(), vec![k.g]);
}

#[test]
fn a_drag_on_the_background_with_a_layer_of_the_frame_selected_draws_a_marquee() {
    let (mut s, k) = scene();
    ex(&mut s, "select.set", json!({"ids": [k.a]}));
    let at = origin(&s, k.f);
    drag(&mut s, (300.0, 200.0), (130.0, 30.0));
    assert_eq!(origin(&s, k.f), at);
    assert_eq!(s.selection(), vec![k.b]);
}

#[test]
fn the_name_label_still_moves_the_selected_frame() {
    let (mut s, k) = scene();
    // The label strip the canvas would publish at zoom 1: just above the frame's top-left corner.
    s.tool.labels = vec![(k.f, [Point::new(0.0, -18.0), Point::new(60.0, -18.0), Point::new(60.0, -4.0), Point::new(0.0, -4.0)])];
    ex(&mut s, "select.set", json!({"ids": [k.f]}));
    let at = origin(&s, k.f);
    drag(&mut s, (30.0, -11.0), (80.0, 19.0));
    let to = origin(&s, k.f);
    assert!((to.x - at.x - 50.0).abs() < 1e-6 && (to.y - at.y - 30.0).abs() < 1e-6, "{at:?} → {to:?}");
    assert_eq!(s.selection(), vec![k.f]);
}

#[test]
fn an_empty_selected_frame_is_still_dragged_by_its_body() {
    let (mut s, k) = scene();
    ex(&mut s, "select.set", json!({"ids": [k.empty]}));
    let at = origin(&s, k.empty);
    drag(&mut s, (640.0, 40.0), (660.0, 70.0));
    let to = origin(&s, k.empty);
    assert!((to.x - at.x - 20.0).abs() < 1e-6 && (to.y - at.y - 30.0).abs() < 1e-6, "{at:?} → {to:?}");
}

#[test]
fn several_selected_layers_are_still_dragged_together_from_a_gap() {
    let (mut s, k) = scene();
    ex(&mut s, "select.set", json!({"ids": [k.a, k.b]}));
    let (a_at, b_at) = (origin(&s, k.a), origin(&s, k.b));
    // Between the two rectangles, inside the selection's box, over the frame's background.
    drag(&mut s, (90.0, 40.0), (100.0, 60.0));
    assert!((origin(&s, k.a).x - a_at.x - 10.0).abs() < 1e-6 && (origin(&s, k.b).y - b_at.y - 20.0).abs() < 1e-6);
}

#[test]
fn a_click_on_the_background_keeps_or_selects_the_frame() {
    let (mut s, k) = scene();
    click(&mut s, (390.0, 290.0));
    assert_eq!(s.selection(), vec![k.f]);
    click(&mut s, (300.0, 200.0));
    assert_eq!(s.selection(), vec![k.f], "a second click keeps it");
    click(&mut s, (300.0, 600.0));
    assert!(s.selection().is_empty(), "empty canvas deselects");
}

#[test]
fn the_selected_frame_shows_no_hover_over_its_background_but_its_layers_do() {
    let (mut s, k) = scene();
    // Not worked in: the background hints that a click selects the frame (the owner's G9 rule).
    assert_eq!(hover(&mut s, (300.0, 200.0)), Some(k.f));
    ex(&mut s, "select.set", json!({"ids": [k.f]}));
    assert_eq!(hover(&mut s, (300.0, 200.0)), None, "selected: no outline over its whole area");
    assert_eq!(hover(&mut s, (140.0, 40.0)), Some(k.b), "a layer inside still lights up");
    // A layer of the frame selected: the frame is worked in too.
    ex(&mut s, "select.set", json!({"ids": [k.a]}));
    assert_eq!(hover(&mut s, (300.0, 200.0)), None);
    // An empty frame has nothing to work inside: it keeps its hover.
    ex(&mut s, "select.set", json!({"ids": [k.empty]}));
    assert_eq!(hover(&mut s, (640.0, 40.0)), Some(k.empty));
}

#[test]
fn zoomed_onto_the_frame_it_shows_no_hover_either() {
    let (mut s, k) = scene();
    assert_eq!(hover(&mut s, (300.0, 200.0)), Some(k.f), "at 100% the frame is a small part of the view");
    {
        let v = &mut s.ds_mut().unwrap().view;
        v.zoom = 2.0;
        v.cx = 200.0;
        v.cy = 150.0;
    }
    let mut e = ev(PointerKind::Hover, 300.0, 200.0);
    e.zoom = 2.0;
    s.pointer(&e).unwrap();
    assert_eq!(s.ds().unwrap().hover, None, "the frame fills the view");
    // Its press still marquees, and a click selects the frame.
    let mut down = ev(PointerKind::Down, 300.0, 200.0);
    down.zoom = 2.0;
    s.pointer(&down).unwrap();
    s.pointer(&PointerEvent { kind: PointerKind::Up, ..down }).unwrap();
    assert_eq!(s.selection(), vec![k.f]);
}

#[test]
fn shift_click_adds_to_the_selection_inside_the_frame() {
    let (mut s, k) = scene();
    ex(&mut s, "select.set", json!({"ids": [k.a]}));
    let mut e = ev(PointerKind::Down, 140.0, 40.0);
    e.shift = true;
    s.pointer(&e).unwrap();
    s.pointer(&PointerEvent { kind: PointerKind::Up, ..e }).unwrap();
    let mut got = s.selection();
    got.sort();
    let mut want = vec![k.a, k.b];
    want.sort();
    assert_eq!(got, want, "Shift-click adds the layer at the depth of the selected one");
    // Shift-drag on the selected frame's background adds a marquee to the selection.
    ex(&mut s, "select.set", json!({"ids": [k.f]}));
    let mut d = ev(PointerKind::Down, 300.0, 200.0);
    d.shift = true;
    s.pointer(&d).unwrap();
    let mut m = ev(PointerKind::Move, 130.0, 30.0);
    m.shift = true;
    s.pointer(&m).unwrap();
    s.pointer(&PointerEvent { kind: PointerKind::Up, ..m }).unwrap();
    assert!(s.selection().contains(&k.b), "{:?}", s.selection());
}

#[test]
fn a_selected_component_or_instance_is_still_dragged_by_its_body() {
    let (mut s, k) = scene();
    let f = id(&ex(&mut s, "frame.create", json!({"x": 800, "y": 0, "width": 200, "height": 150})));
    id(&ex(&mut s, "shape.rectangle", json!({"x": 10, "y": 10, "width": 40, "height": 40, "parent": f})));
    let comp = id(&ex(&mut s, "component.create", json!({"ids": [f]})));
    let inst = id(&ex(&mut s, "instance.create", json!({"component": comp, "x": 800, "y": 400})));
    for item in [comp, inst] {
        ex(&mut s, "select.set", json!({"ids": [item]}));
        let at = origin(&s, item);
        // The body, clear of the layer inside and of the resize handles.
        drag(&mut s, (at.x + 120.0, at.y + 75.0), (at.x + 150.0, at.y + 95.0));
        let to = origin(&s, item);
        assert!((to.x - at.x - 30.0).abs() < 1e-6 && (to.y - at.y - 20.0).abs() < 1e-6, "{item:?}: {at:?} → {to:?}");
    }
    assert_eq!(origin(&s, k.f), Point::new(0.0, 0.0));
}

#[test]
fn shift_click_on_the_background_of_the_selected_frame_takes_it_out() {
    let (mut s, k) = scene();
    ex(&mut s, "select.set", json!({"ids": [k.f]}));
    let mut e = ev(PointerKind::Down, 300.0, 200.0);
    e.shift = true;
    s.pointer(&e).unwrap();
    s.pointer(&PointerEvent { kind: PointerKind::Up, ..e }).unwrap();
    assert!(s.selection().is_empty());
}

#[test]
fn zooming_to_fit_a_wide_frame_counts_as_zoomed_onto_it() {
    let (mut s, _) = scene();
    {
        // A 400 × 300 frame at 2.0 in a tall view: 800 px of 1000 wide, 600 of 2000 high.
        let v = &mut s.ds_mut().unwrap().view;
        v.zoom = 2.0;
        v.vw = 1000.0;
        v.vh = 2000.0;
    }
    let mut e = ev(PointerKind::Hover, 300.0, 200.0);
    e.zoom = 2.0;
    s.pointer(&e).unwrap();
    assert_eq!(s.ds().unwrap().hover, None);
}
