//! Graybox: a boolean group's box (its selection box and handles, W and H) is the box of its
//! result, as in Figma, not of all its shapes: a square with a circle subtracted from its lower
//! half is as tall as what is left. Its shapes keep their places on the page.

use figcraft_doc::{Document, NodeId, NodeKind};
use figcraft_geom::{Affine, Shape as _};

/// Shrink boolean group `id` (already wrapping its shapes) to the bounds of its result. Nothing
/// changes when it has no result (everything subtracted, or past the path operations' caps).
pub(crate) fn fit_to_result(doc: &mut Document, id: NodeId) {
    let Some(n) = doc.node(id) else { return };
    if !matches!(n.kind, NodeKind::BooleanOp { .. }) {
        return;
    }
    let Some((path, _)) = figcraft_render::node_outline(doc, n) else { return };
    let r = path.bounding_box();
    let eps = 1e-6;
    let finite = [r.x0, r.y0, r.x1, r.y1].iter().all(|v| v.is_finite());
    if !finite || r.width() <= eps || r.height() <= eps {
        return;
    }
    // Already the result's box.
    if r.x0.abs() < eps && r.y0.abs() < eps && (r.width() - n.width).abs() < eps && (r.height() - n.height).abs() < eps {
        return;
    }
    let shift = Affine::translate((r.x0, r.y0));
    let back = Affine::translate((-r.x0, -r.y0));
    let children = n.children.clone();
    if let Ok(g) = doc.get_mut(id) {
        g.transform *= shift;
        g.width = r.width();
        g.height = r.height();
    }
    // The shapes are placed in the group's space: they move back by as much, so they stay put.
    for c in children {
        if let Ok(cn) = doc.get_mut(c) {
            cn.transform = back * cn.transform;
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::Session;
    use figcraft_doc::NodeId;
    use serde_json::json;

    fn id(v: &serde_json::Value) -> NodeId {
        NodeId(v["id"].as_u64().unwrap())
    }

    /// Subtracting a circle that covers a square's lower half: the boolean's box is the upper half
    /// left, on the page where it was; a union of two shapes stays the box of both.
    #[test]
    fn a_boolean_box_is_its_result() {
        let mut s = Session::new();
        let sq = id(&s.execute("shape.rectangle", &json!({"x": 0, "y": 0, "width": 100, "height": 100})).unwrap());
        let cut = id(&s.execute("shape.rectangle", &json!({"x": -10, "y": 50, "width": 120, "height": 80})).unwrap());
        let b = id(&s.execute("bool.subtract", &json!({"ids": [sq, cut]})).unwrap());
        let doc = s.doc().unwrap();
        let n = doc.node(b).unwrap();
        assert!((n.width - 100.0).abs() < 1e-6 && (n.height - 50.0).abs() < 1e-6, "{} × {}", n.width, n.height);
        let bounds = doc.world_bounds(b).unwrap();
        assert!(bounds.x0.abs() < 1e-6 && bounds.y0.abs() < 1e-6, "{bounds:?}");
        // The square stays where it was on the page.
        let sq_bounds = doc.world_bounds(sq).unwrap();
        assert!(sq_bounds.x0.abs() < 1e-6 && sq_bounds.y0.abs() < 1e-6 && (sq_bounds.height() - 100.0).abs() < 1e-6, "{sq_bounds:?}");
        let a = id(&s.execute("shape.rectangle", &json!({"x": 300, "y": 0, "width": 50, "height": 50})).unwrap());
        let c = id(&s.execute("shape.rectangle", &json!({"x": 300, "y": 100, "width": 50, "height": 50})).unwrap());
        let u = id(&s.execute("bool.union", &json!({"ids": [a, c]})).unwrap());
        let un = s.doc().unwrap().node(u).unwrap().clone();
        assert!((un.width - 50.0).abs() < 1e-6 && (un.height - 150.0).abs() < 1e-6);
    }
}
