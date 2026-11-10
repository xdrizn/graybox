//! Graybox: a whole ellipse edited as a vector has four points (right, bottom, left, top) with
//! the usual quarter-curve handles, as in Figma. Its outline is drawn with as many curve pieces as
//! its size needs, which made a big ellipse edit with five or six points.

use figcraft_doc::{Node, NodeKind};
use figcraft_geom::{BezPath, Point};

/// The handle length of a quarter ellipse, as a share of its radius (4/3·(√2−1)).
const KAPPA: f64 = 0.552_284_749_830_793_4;

/// The four-point outline of `n` when it is a whole ellipse (no arc, no hole), in its own box.
pub(crate) fn four(n: &Node) -> Option<BezPath> {
    let NodeKind::Ellipse { sweep, inner, .. } = n.kind else { return None };
    if !figcraft_geom::full_sweep(sweep) || inner > 0.0 {
        return None;
    }
    let (w, h) = (n.width, n.height);
    if !(w.is_finite() && h.is_finite()) || w <= 0.0 || h <= 0.0 {
        return None;
    }
    let (cx, cy) = (w / 2.0, h / 2.0);
    let (kx, ky) = (cx * KAPPA, cy * KAPPA);
    let p = Point::new;
    let mut path = BezPath::new();
    path.move_to(p(w, cy));
    path.curve_to(p(w, cy + ky), p(cx + kx, h), p(cx, h));
    path.curve_to(p(cx - kx, h), p(0.0, cy + ky), p(0.0, cy));
    path.curve_to(p(0.0, cy - ky), p(cx - kx, 0.0), p(cx, 0.0));
    path.curve_to(p(cx + kx, 0.0), p(w, cy - ky), p(w, cy));
    path.close_path();
    Some(path)
}

#[cfg(test)]
mod tests {
    use crate::Session;
    use figcraft_doc::{NodeId, NodeKind};
    use serde_json::json;

    /// A big ellipse entered in vector edit has four points, at its right, bottom, left and top.
    #[test]
    fn an_ellipse_edits_with_four_points() {
        for size in [40.0, 800.0, 3000.0] {
            let mut s = Session::new();
            let e =
                NodeId(s.execute("shape.ellipse", &json!({"x": 0, "y": 0, "width": size, "height": size / 2.0})).unwrap()["id"].as_u64().unwrap());
            s.execute("vector.edit.begin", &json!({"id": e})).unwrap();
            let doc = s.doc().unwrap();
            let NodeKind::Vector { network } = &doc.node(e).unwrap().kind else { panic!("a vector now") };
            assert_eq!(network.vertices.len(), 4, "{size}: {} points", network.vertices.len());
            assert_eq!(network.segments.len(), 4, "{size}");
        }
    }
}
