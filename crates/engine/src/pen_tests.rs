//! Graybox: line and pen behaviour. The Pen's second click with a little wobble (a high polling rate mouse) makes a straight
//! segment, not a curve; a real drag still pulls out handles.

use crate::Session;
use crate::tools::{PointerEvent, PointerKind};
use figcraft_doc::NodeKind;
use serde_json::json;

fn ev(kind: PointerKind, x: f64, y: f64) -> PointerEvent {
    PointerEvent::at(kind, x, y, 1.0)
}

/// Two clicks; the second one moves `wobble` points while pressed. The segment's handles.
fn segment_handles(wobble: f64) -> ((f64, f64), (f64, f64)) {
    let mut s = Session::new();
    s.execute("tool.set", &json!({"tool": "pen"})).unwrap();
    s.pointer(&ev(PointerKind::Down, 0.0, 0.0)).unwrap();
    s.pointer(&ev(PointerKind::Up, 0.0, 0.0)).unwrap();
    s.pointer(&ev(PointerKind::Down, 100.0, 0.0)).unwrap();
    s.pointer(&ev(PointerKind::Move, 100.0 + wobble, wobble / 2.0)).unwrap();
    s.pointer(&ev(PointerKind::Up, 100.0 + wobble, wobble / 2.0)).unwrap();
    s.pen_end().unwrap();
    let doc = s.doc().unwrap();
    let net = doc
        .nodes()
        .find_map(|n| match &n.kind {
            NodeKind::Vector { network } if !network.segments.is_empty() => Some(network.clone()),
            _ => None,
        })
        .expect("a path");
    let seg = net.segments.first().cloned().unwrap();
    (seg.ta, seg.tb)
}

#[test]
fn a_wobbly_second_click_makes_a_straight_segment() {
    assert_eq!(segment_handles(5.0), ((0.0, 0.0), (0.0, 0.0)), "5 px of wobble: straight");
    let (_, tb) = segment_handles(40.0);
    assert!(tb != (0.0, 0.0), "a real drag pulls out a handle");
}

/// Graybox: Shift-dragging a horizontal line's end lengthens it along itself.
#[test]
fn shift_dragging_a_lines_end_lengthens_it() {
    let mut s = Session::new();
    let l = s.execute("shape.line", &json!({"x": 0, "y": 0, "width": 100})).unwrap()["id"].as_u64().unwrap();
    let l = figcraft_doc::NodeId(l);
    s.execute("select.set", &json!({"ids": [l]})).unwrap();
    for (kind, x, y) in
        [(PointerKind::Down, 100.0, 0.0), (PointerKind::Move, 120.0, 2.0), (PointerKind::Move, 150.0, 3.0), (PointerKind::Up, 150.0, 3.0)]
    {
        let mut e = ev(kind, x, y);
        e.shift = true;
        s.pointer(&e).unwrap();
    }
    let doc = s.doc().unwrap();
    let n = doc.node(l).unwrap();
    assert!(
        (n.width - 150.0).abs() < 1.0 && figcraft_geom::rotation_degrees(n.transform).abs() < 0.5,
        "w={} rot={}",
        n.width,
        figcraft_geom::rotation_degrees(n.transform)
    );
}

fn pev(kind: PointerKind, x: f64, y: f64, shift: bool) -> PointerEvent {
    let mut e = ev(kind, x, y);
    e.shift = shift;
    e
}

type Handles = ((f64, f64), (f64, f64));

/// The pen's first path: its segments' handles and its points.
fn pen_path(s: &Session) -> (Vec<Handles>, Vec<(f64, f64)>) {
    let doc = s.doc().unwrap();
    let net = doc
        .nodes()
        .find_map(|n| match &n.kind {
            NodeKind::Vector { network } if !network.segments.is_empty() => Some(network.clone()),
            _ => None,
        })
        .expect("a path");
    (net.segments.iter().map(|g| (g.ta, g.tb)).collect(), net.vertices.iter().map(|v| (v.x, v.y)).collect())
}

/// Graybox (owner's report): with Shift held the Pen never makes a curve. A Shift-click a little
/// off a horizontal line used to curve: the point was put on the 45° line, and the wobble was
/// measured from there, not from where the press was.
#[test]
fn a_shift_click_off_a_horizontal_line_makes_a_straight_segment() {
    let mut s = Session::new();
    s.execute("tool.set", &json!({"tool": "pen"})).unwrap();
    s.pointer(&ev(PointerKind::Down, 0.0, 0.0)).unwrap();
    s.pointer(&ev(PointerKind::Up, 0.0, 0.0)).unwrap();
    s.pointer(&pev(PointerKind::Down, 200.0, 12.0, true)).unwrap();
    s.pointer(&pev(PointerKind::Move, 201.0, 12.0, true)).unwrap();
    s.pointer(&pev(PointerKind::Up, 201.0, 12.0, true)).unwrap();
    s.pen_end().unwrap();
    let (segs, pts) = pen_path(&s);
    assert_eq!(segs, vec![((0.0, 0.0), (0.0, 0.0))], "straight");
    assert!(pts.iter().all(|p| p.1.abs() < 1e-6), "on the horizontal: {pts:?}");
}

#[test]
fn a_click_off_its_snapped_place_still_needs_a_real_drag_to_curve() {
    // Without Shift too: the wobble counts from the press, so a press snapped (or put on 45°)
    // away from the pointer is still a click.
    let mut s = Session::new();
    s.execute("tool.set", &json!({"tool": "pen"})).unwrap();
    s.pointer(&ev(PointerKind::Down, 0.0, 0.0)).unwrap();
    s.pointer(&ev(PointerKind::Up, 0.0, 0.0)).unwrap();
    s.pointer(&pev(PointerKind::Down, 200.0, 12.0, true)).unwrap();
    s.pointer(&pev(PointerKind::Move, 203.0, 13.0, false)).unwrap();
    s.pointer(&pev(PointerKind::Up, 203.0, 13.0, false)).unwrap();
    s.pen_end().unwrap();
    assert_eq!(pen_path(&s).0, vec![((0.0, 0.0), (0.0, 0.0))]);
}

#[test]
fn a_shift_drag_with_the_pen_makes_no_curve() {
    let mut s = Session::new();
    s.execute("tool.set", &json!({"tool": "pen"})).unwrap();
    // A plain drag at the first point pulls out a handle...
    s.pointer(&ev(PointerKind::Down, 0.0, 0.0)).unwrap();
    s.pointer(&ev(PointerKind::Move, 0.0, 60.0)).unwrap();
    s.pointer(&ev(PointerKind::Up, 0.0, 60.0)).unwrap();
    // ...but with Shift every segment is straight, also the one leaving that handle.
    for (x, y) in [(100.0, 0.0), (200.0, 100.0)] {
        s.pointer(&pev(PointerKind::Down, x, y, true)).unwrap();
        s.pointer(&pev(PointerKind::Move, x + 50.0, y + 40.0, true)).unwrap();
        s.pointer(&pev(PointerKind::Up, x + 50.0, y + 40.0, true)).unwrap();
    }
    s.pen_end().unwrap();
    let (segs, pts) = pen_path(&s);
    assert_eq!(segs, vec![((0.0, 0.0), (0.0, 0.0)); 2], "straight: {segs:?}");
    assert_eq!(pts.len(), 3);
}

#[test]
fn a_shift_click_in_vector_edit_mode_makes_no_curve() {
    let mut s = Session::new();
    let r = s.execute("vector.create", &json!({"points": [[0, 0], [100, 0]], "closed": false})).unwrap();
    let id = crate::params::id_of(&r["id"]).unwrap();
    s.execute("vector.edit.begin", &json!({"id": id})).unwrap();
    s.execute("vector.edit.tool", &json!({"tool": "pen"})).unwrap();
    s.pointer(&ev(PointerKind::Down, 100.0, 0.0)).unwrap();
    s.pointer(&ev(PointerKind::Up, 100.0, 0.0)).unwrap();
    s.pointer(&pev(PointerKind::Down, 200.0, 8.0, true)).unwrap();
    s.pointer(&pev(PointerKind::Move, 240.0, 40.0, true)).unwrap();
    s.pointer(&pev(PointerKind::Up, 240.0, 40.0, true)).unwrap();
    let net = match &s.doc().unwrap().node(id).unwrap().kind {
        NodeKind::Vector { network } => network.clone(),
        _ => panic!("vector"),
    };
    assert!(net.segments.iter().all(|g| g.ta == (0.0, 0.0) && g.tb == (0.0, 0.0)), "{:?}", net.segments);
    assert!(net.vertices.iter().all(|v| v.y.abs() < 1e-6), "on the horizontal");
}

#[test]
fn a_small_wobble_in_vector_edit_mode_makes_no_curve() {
    let mut s = Session::new();
    let r = s.execute("vector.create", &json!({"points": [[0, 0], [100, 0]], "closed": false})).unwrap();
    let id = crate::params::id_of(&r["id"]).unwrap();
    s.execute("vector.edit.begin", &json!({"id": id})).unwrap();
    s.execute("vector.edit.tool", &json!({"tool": "pen"})).unwrap();
    s.pointer(&ev(PointerKind::Down, 100.0, 0.0)).unwrap();
    s.pointer(&ev(PointerKind::Up, 100.0, 0.0)).unwrap();
    s.pointer(&ev(PointerKind::Down, 200.0, 0.0)).unwrap();
    s.pointer(&ev(PointerKind::Move, 205.0, 2.0)).unwrap();
    s.pointer(&ev(PointerKind::Up, 205.0, 2.0)).unwrap();
    let net = match &s.doc().unwrap().node(id).unwrap().kind {
        NodeKind::Vector { network } => network.clone(),
        _ => panic!("vector"),
    };
    assert!(net.segments.iter().all(|g| g.ta == (0.0, 0.0) && g.tb == (0.0, 0.0)), "{:?}", net.segments);
}
