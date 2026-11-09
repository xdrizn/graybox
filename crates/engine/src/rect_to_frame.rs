//! Graybox: "Frame selection" on one rectangle turns that rectangle into the frame (same layer:
//! place, size, fills, strokes, corners, effects, name, constraints, prototype), instead of putting
//! it inside a new frame. A frame clips its contents, as a new one does.

use figcraft_doc::{NodeId, NodeKind};
use serde_json::{Value, json};

use crate::{Result, Session};

/// When `ids` is one rectangle (not inside an instance), make it a frame in place: its id, as
/// `object.frameSelection` returns a new frame's. None: not that case, wrap as usual.
pub(crate) fn convert(s: &mut Session, ids: &[NodeId]) -> Result<Option<Value>> {
    let [id] = ids else { return Ok(None) };
    let doc = s.doc_mut()?;
    let Some(n) = doc.node(*id) else { return Ok(None) };
    if !matches!(n.kind, NodeKind::Rectangle) || crate::components::owning_instance(doc, *id).is_some() {
        return Ok(None);
    }
    let n = doc.get_mut(*id)?;
    n.kind = NodeKind::Frame;
    n.clip = true;
    s.set_selection(vec![*id]);
    Ok(Some(json!({"id": id})))
}

#[cfg(test)]
mod tests {
    use crate::Session;
    use figcraft_doc::{NodeId, NodeKind};
    use serde_json::json;

    /// A rectangle framed is the frame: same id, place, size, fill and corners, and nothing inside.
    /// Two layers (or anything else) are still wrapped in a new frame.
    #[test]
    fn a_rectangle_framed_becomes_the_frame() {
        let mut s = Session::new();
        let r = NodeId(
            s.execute("shape.rectangle", &json!({"x": 40, "y": 30, "width": 120, "height": 80, "props": {"fill": "#ff8800", "cornerRadius": 12}}))
                .unwrap()["id"]
                .as_u64()
                .unwrap(),
        );
        let out = s.execute("object.frameSelection", &json!({"ids": [r]})).unwrap();
        assert_eq!(out["id"].as_u64(), Some(r.0));
        let doc = s.doc().unwrap();
        let n = doc.node(r).unwrap();
        assert!(matches!(n.kind, NodeKind::Frame) && n.children.is_empty() && n.clip);
        assert_eq!((n.width, n.height), (120.0, 80.0));
        assert_eq!(doc.world_bounds(r).unwrap().origin(), figcraft_geom::Point::new(40.0, 30.0));
        assert_eq!(n.fills.len(), 1);
        assert_eq!(n.corners.uniform(), Some(12.0));
        assert_eq!(s.selection(), vec![r]);
        // Two rectangles: wrapped, as before.
        let a = s.execute("shape.rectangle", &json!({"x": 300, "width": 10, "height": 10})).unwrap()["id"].as_u64().unwrap();
        let b = s.execute("shape.rectangle", &json!({"x": 320, "width": 10, "height": 10})).unwrap()["id"].as_u64().unwrap();
        let f = s.execute("object.frameSelection", &json!({"ids": [a, b]})).unwrap()["id"].as_u64().unwrap();
        assert_eq!(s.doc().unwrap().node(NodeId(f)).unwrap().children.len(), 2);
        // Undo turns the frame back into the rectangle.
        assert!(s.undo().unwrap() && s.undo().unwrap() && s.undo().unwrap() && s.undo().unwrap());
        assert!(matches!(s.doc().unwrap().node(r).unwrap().kind, NodeKind::Rectangle));
    }
}
