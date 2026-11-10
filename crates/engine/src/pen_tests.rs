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
