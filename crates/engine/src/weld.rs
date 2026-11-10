//! Graybox: "Join selection" on several layers (lines or paths drawn apart, the three strokes of
//! an arrow, say) welds the ends that meet into one point, so the result is one connected path. Upstream
//! merged the layers into one vector but left every end loose.

use figcraft_doc::{NodeId, NodeKind, VectorNetwork};

use crate::{Result, Session};

/// How close two points must be (in the layer's units) to become one.
pub(crate) const TOLERANCE: f64 = 2.0;

/// The most points welded (pairs are compared): beyond it, nothing is welded.
const MAX_POINTS: usize = 4096;

/// Weld the points of `net` closer than `tol` into the earliest of them; returns how many went.
pub(crate) fn weld(net: &mut VectorNetwork, tol: f64) -> usize {
    let n = net.vertices.len();
    if n > MAX_POINTS || !tol.is_finite() || tol <= 0.0 {
        return 0;
    }
    let mut to: Vec<usize> = (0..n).collect();
    for i in 0..n {
        let Some(vi) = net.vertices.get(i).map(|v| (v.x, v.y)) else { continue };
        for j in 0..i {
            if to.get(j) != Some(&j) {
                continue;
            }
            let Some(vj) = net.vertices.get(j) else { continue };
            if (vi.0 - vj.x).hypot(vi.1 - vj.y) < tol {
                if let Some(slot) = to.get_mut(i) {
                    *slot = j;
                }
                break;
            }
        }
    }
    // The points kept, in order, and where each old point went.
    let kept: Vec<usize> = (0..n).filter(|i| to.get(*i) == Some(i)).collect();
    let mut index = vec![0usize; n];
    for (new, old) in kept.iter().enumerate() {
        if let Some(slot) = index.get_mut(*old) {
            *slot = new;
        }
    }
    let new_of = |old: usize| to.get(old).and_then(|t| index.get(*t)).copied().unwrap_or(0);
    for s in &mut net.segments {
        s.a = new_of(s.a);
        s.b = new_of(s.b);
    }
    let gone = n - kept.len();
    net.vertices = kept.iter().filter_map(|i| net.vertices.get(*i).cloned()).collect();
    // A straight segment shorter than the tolerance shrank to nothing: drop it (only where no
    // fill region lists segments by index).
    if net.regions.is_empty() {
        net.segments.retain(|s| !(s.a == s.b && s.is_straight()));
    }
    gone
}

/// After `vector.join` made layer `id` from several: weld its meeting ends.
pub(crate) fn after_join(s: &mut Session, id: NodeId) -> Result<()> {
    let doc = s.doc_mut()?;
    if let NodeKind::Vector { network } = &mut doc.get_mut(id)?.kind {
        weld(network, TOLERANCE);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::Session;
    use figcraft_doc::{NodeId, NodeKind};
    use serde_json::json;

    /// An arrow drawn as three lines meeting at its tip (one a pixel off) joins into one path whose
    /// tip is a single point.
    #[test]
    fn joined_lines_meet_at_one_point() {
        let mut s = Session::new();
        let path = |s: &mut Session, d: &str, x: f64, y: f64| {
            NodeId(
                s.execute("node.create", &json!({"type": "vector", "x": x, "y": y, "props": {"path": d, "strokes": ["#000000"]}})).unwrap()["id"]
                    .as_u64()
                    .unwrap(),
            )
        };
        // Shaft from (0, 50) to (100, 50); two barbs from the tip back up and down (one a pixel off).
        let shaft = path(&mut s, "M0 50 L100 50", 0.0, 50.0);
        let up = path(&mut s, "M100 50 L80 30", 80.0, 30.0);
        let down = path(&mut s, "M101 50 L80 70", 80.0, 50.0);
        let out = s.execute("vector.join", &json!({"ids": [shaft, up, down]})).unwrap();
        let id = NodeId(out["id"].as_u64().unwrap());
        let doc = s.doc().unwrap();
        let NodeKind::Vector { network } = &doc.node(id).unwrap().kind else { panic!("a vector") };
        assert_eq!(network.vertices.len(), 4, "tail, tip and two barb ends: {:?}", network.vertices.iter().map(|v| (v.x, v.y)).collect::<Vec<_>>());
        assert_eq!(network.segments.len(), 3);
        // The tip is one point that all three segments use.
        let tip = (0..network.vertices.len()).find(|v| network.incident(*v).len() == 3);
        assert!(tip.is_some(), "a point where the three strokes meet");
    }
}
