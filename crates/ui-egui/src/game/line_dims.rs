//! Graybox: a line turned upright (rotated 90° either way) shows its length as H in Dimensions, and
//! editing H changes its length (W is its thickness, 0). A line is stored as a length along its own
//! x axis, so upstream showed a vertical line's length as W.

use figcraft_doc::{Document, NodeId, NodeKind};

/// Whether every layer of `sel` is a line turned upright.
pub(crate) fn vertical(doc: &Document, sel: &[NodeId]) -> bool {
    !sel.is_empty()
        && sel.iter().all(|id| {
            doc.node(*id).is_some_and(|n| {
                let deg = figcraft_geom::rotation_degrees(n.transform).abs();
                matches!(n.kind, NodeKind::Line) && (deg - 90.0).abs() < 0.5
            })
        })
}

#[cfg(test)]
mod tests {
    use figcraft_doc::NodeId;
    use figcraft_engine::Session;
    use serde_json::json;

    #[test]
    fn an_upright_line_is_vertical() {
        let mut s = Session::new();
        let l = NodeId(s.execute("shape.line", &json!({"x": 0, "y": 0, "width": 100})).unwrap()["id"].as_u64().unwrap());
        assert!(!super::vertical(s.doc().unwrap(), &[l]));
        s.execute("node.set", &json!({"ids": [l], "props": {"rotation": 90}})).unwrap();
        assert!(super::vertical(s.doc().unwrap(), &[l]));
        s.execute("node.set", &json!({"ids": [l], "props": {"rotation": -90}})).unwrap();
        assert!(super::vertical(s.doc().unwrap(), &[l]));
    }
}
