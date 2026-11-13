//! Graybox (owner's report): the Eraser works in a design file too: it removes the strokes it is
//! dragged over (pencil and other open, unfilled paths), never shapes, frames or locked layers.

use crate::Session;
use crate::tools::{PointerEvent, PointerKind};
use figcraft_doc::NodeId;
use serde_json::json;

fn ev(kind: PointerKind, x: f64, y: f64) -> PointerEvent {
    PointerEvent::at(kind, x, y, 1.0)
}

fn id(v: serde_json::Value) -> NodeId {
    crate::params::id_of(&v["id"]).unwrap()
}

/// A pencil stroke from (`x0`, 50) to (`x0` + 100, 50).
fn pencil(s: &mut Session, x0: f64) {
    s.execute("tool.set", &json!({"tool": "pencil"})).unwrap();
    s.pointer(&ev(PointerKind::Down, x0, 50.0)).unwrap();
    for k in 1..=10 {
        s.pointer(&ev(PointerKind::Move, x0 + 10.0 * f64::from(k), 50.0)).unwrap();
    }
    s.pointer(&ev(PointerKind::Up, x0 + 100.0, 50.0)).unwrap();
}

fn count(s: &Session) -> usize {
    let doc = s.doc().unwrap();
    doc.node(s.page().unwrap()).unwrap().children.len()
}

#[test]
fn the_eraser_removes_pencil_strokes_in_a_design_file() {
    let mut s = Session::new();
    pencil(&mut s, 0.0);
    pencil(&mut s, 300.0);
    let rect = id(s.execute("shape.rectangle", &json!({"x": 0, "y": 200, "width": 100, "height": 100})).unwrap());
    assert_eq!(count(&s), 3);
    s.execute("tool.eraser", &json!({})).expect("the eraser is not only for boards");
    // One drag across the first stroke and the rectangle: the stroke goes, the rectangle stays.
    s.pointer(&ev(PointerKind::Down, 50.0, 40.0)).unwrap();
    s.pointer(&ev(PointerKind::Move, 50.0, 50.0)).unwrap();
    s.pointer(&ev(PointerKind::Move, 50.0, 250.0)).unwrap();
    s.pointer(&ev(PointerKind::Up, 50.0, 250.0)).unwrap();
    assert_eq!(count(&s), 2, "one stroke erased");
    assert!(s.doc().unwrap().node(rect).is_some(), "shapes stay");
    // One undo step brings it back.
    assert!(s.undo().unwrap());
    assert_eq!(count(&s), 3);
}

#[test]
fn the_eraser_can_be_chosen_with_tool_set_in_a_design_file() {
    let mut s = Session::new();
    s.execute("tool.set", &json!({"tool": "eraser"})).expect("eraser in a design file");
    assert_eq!(s.tool.tool, crate::tools::ToolId::Eraser);
    // The other board tools still need a board.
    assert!(s.execute("tool.set", &json!({"tool": "sticky"})).is_err());
}
