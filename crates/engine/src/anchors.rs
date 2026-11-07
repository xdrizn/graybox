//! Graybox: layers in a frame anchor themselves by where they sit, like a 9-slice. The frame is
//! cut in thirds each way; the third holding most of a layer picks its constraint (left third:
//! Left, middle: Center, right: Right; the same for Top / Center / Bottom), and a layer reaching
//! across the frame (two thirds of it or more, or from the first third into the last) stretches
//! (Left & right, Top & bottom). It happens whenever a layer is placed, moved or resized in a frame,
//! until its constraints are set by hand (`GameSettings::anchored`); "Auto" in the panel hands them back.

use std::collections::BTreeSet;

use figcraft_doc::{Constraints, Document, HConstraint, NodeId, NodeKind, VConstraint};
use figcraft_geom::Rect;

/// One axis: where the layer runs (`lo..hi`) in a frame `size` long.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Slot {
    Start,
    Middle,
    End,
    Stretch,
}

fn slot(lo: f64, hi: f64, size: f64) -> Slot {
    if !(lo.is_finite() && hi.is_finite() && size.is_finite()) || size <= 0.0 {
        return Slot::Start;
    }
    let (a, b) = (size / 3.0, size * 2.0 / 3.0);
    let len = (hi - lo).max(0.0);
    if len >= b || (lo < a && hi > b) {
        return Slot::Stretch;
    }
    // How much of the layer is in each third (a layer outside the frame counts where its middle is).
    let part = |t0: f64, t1: f64| (hi.min(t1) - lo.max(t0)).max(0.0);
    let parts = [part(f64::NEG_INFINITY, a), part(a, b), part(b, f64::INFINITY)];
    let best = if len <= 0.0 {
        let mid = lo;
        if mid < a {
            0
        } else if mid > b {
            2
        } else {
            1
        }
    } else {
        // Ties go to the middle (a layer centred on a third's line).
        let [s, m, e] = parts;
        if m >= s && m >= e {
            1
        } else if s >= e {
            0
        } else {
            2
        }
    };
    [Slot::Start, Slot::Middle, Slot::End].get(best).copied().unwrap_or(Slot::Start)
}

/// The constraints a layer with box `r` (in its frame's space) gets in a `w` × `h` frame.
pub fn guess(r: Rect, w: f64, h: f64) -> Constraints {
    let horizontal = match slot(r.x0, r.x1, w) {
        Slot::Start => HConstraint::Left,
        Slot::Middle => HConstraint::Center,
        Slot::End => HConstraint::Right,
        Slot::Stretch => HConstraint::LeftRight,
    };
    let vertical = match slot(r.y0, r.y1, h) {
        Slot::Start => VConstraint::Top,
        Slot::Middle => VConstraint::Center,
        Slot::End => VConstraint::Bottom,
        Slot::Stretch => VConstraint::TopBottom,
    };
    Constraints { horizontal, vertical }
}

/// Whether layer `id`'s constraints were set by hand.
pub fn manual(doc: &Document, id: NodeId) -> bool {
    doc.game.anchored.contains(&id)
}

/// The frame whose constraints apply to layer `id` (its parent, a frame without auto layout or a
/// layer placed absolutely in one), when the layer's anchoring is its own to guess: not inside an
/// instance (which takes its main's).
fn anchoring_frame(doc: &Document, id: NodeId) -> Option<NodeId> {
    let n = doc.node(id)?;
    let p = n.parent?;
    let pn = doc.node(p)?;
    if !pn.kind.is_frame_like() || pn.kind.bounds_follow_children() || (pn.layout.is_some() && !n.child_layout.absolute) {
        return None;
    }
    let in_instance = std::iter::once(id).chain(doc.ancestors(id)).any(|a| doc.node(a).is_some_and(|x| matches!(x.kind, NodeKind::Instance { .. })));
    (!in_instance).then_some(p)
}

/// Guess layer `id`'s constraints now (it's in an anchoring frame). Returns whether they changed.
pub fn apply(doc: &mut Document, id: NodeId) -> bool {
    let Some(frame) = anchoring_frame(doc, id) else { return false };
    let (Some(n), Some(f)) = (doc.node(id), doc.node(frame)) else { return false };
    let r = figcraft_geom::transform_rect_bbox(n.transform, n.local_rect());
    let c = guess(r, f.width, f.height);
    if n.constraints == c {
        return false;
    }
    doc.node_mut(id).is_some_and(|n| {
        n.constraints = c;
        true
    })
}

/// After an edit: layers placed, moved or resized in a frame (not by their frame resizing, which
/// moves them by their constraints) and not anchored by hand take the anchoring of where they sit.
pub fn settle(doc: &mut Document, before: &Document, touched: &BTreeSet<NodeId>) {
    for &id in touched {
        if manual(doc, id) {
            continue;
        }
        let Some(n) = doc.node(id) else { continue };
        let Some(frame) = anchoring_frame(doc, id) else { continue };
        let placed = match before.node(id) {
            None => true,
            Some(b) => b.parent != n.parent || b.transform != n.transform || b.width != n.width || b.height != n.height,
        };
        let frame_resized =
            before.node(frame).zip(doc.node(frame)).is_some_and(|(b, f)| b.width != f.width || b.height != f.height || b.layout != f.layout);
        if placed && !frame_resized {
            apply(doc, id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Session;
    use serde_json::json;

    #[test]
    fn the_third_holding_most_of_a_layer_picks_its_anchoring() {
        let g = |x0, y0, x1, y1| guess(Rect::new(x0, y0, x1, y1), 300.0, 300.0);
        assert_eq!(g(10.0, 10.0, 60.0, 60.0), Constraints { horizontal: HConstraint::Left, vertical: VConstraint::Top });
        assert_eq!(g(130.0, 130.0, 170.0, 170.0), Constraints { horizontal: HConstraint::Center, vertical: VConstraint::Center });
        assert_eq!(g(250.0, 250.0, 290.0, 290.0), Constraints { horizontal: HConstraint::Right, vertical: VConstraint::Bottom });
        // Mostly in the right third, a little in the middle: Right.
        assert_eq!(g(180.0, 10.0, 290.0, 40.0).horizontal, HConstraint::Right);
        // A bar across the bottom stretches.
        assert_eq!(g(0.0, 270.0, 300.0, 300.0), Constraints { horizontal: HConstraint::LeftRight, vertical: VConstraint::Bottom });
        // From the first third into the last: stretches.
        assert_eq!(g(80.0, 10.0, 230.0, 30.0).horizontal, HConstraint::LeftRight);
    }

    fn id(v: &serde_json::Value) -> NodeId {
        NodeId(v["id"].as_u64().unwrap())
    }

    /// A layer drawn in a frame's bottom-right corner anchors there, and follows when moved; once
    /// its constraints are set by hand, moving it keeps them; "Auto" hands them back.
    #[test]
    fn layers_anchor_by_position_until_set_by_hand() {
        let mut s = Session::new();
        let f = id(&s.execute("frame.create", &json!({"x": 0, "y": 0, "width": 300, "height": 300})).unwrap());
        let r = id(&s.execute("shape.rectangle", &json!({"parent": f, "x": 250, "y": 250, "width": 40, "height": 40})).unwrap());
        let c = |s: &Session| s.doc().unwrap().node(r).unwrap().constraints;
        assert_eq!(c(&s), Constraints { horizontal: HConstraint::Right, vertical: VConstraint::Bottom });
        s.execute("node.set", &json!({"ids": [r], "props": {"x": 130, "y": 10}})).unwrap();
        assert_eq!(c(&s), Constraints { horizontal: HConstraint::Center, vertical: VConstraint::Top });
        s.execute("node.set", &json!({"ids": [r], "props": {"constraints": {"horizontal": "scale"}}})).unwrap();
        s.execute("node.set", &json!({"ids": [r], "props": {"x": 250}})).unwrap();
        assert_eq!(c(&s).horizontal, HConstraint::Scale, "set by hand: kept");
        s.execute("game.anchors.auto", &json!({"ids": [r]})).unwrap();
        assert_eq!(c(&s), Constraints { horizontal: HConstraint::Right, vertical: VConstraint::Top }, "Auto: by position again");
        // Resizing the frame moves the layer by its constraints: its anchoring stays.
        s.execute("node.set", &json!({"ids": [f], "props": {"width": 120}})).unwrap();
        assert_eq!(c(&s), Constraints { horizontal: HConstraint::Right, vertical: VConstraint::Top });
        // Auto on a frame anchors every layer in it by position (those set by hand too: asked for).
        let l = id(&s.execute("shape.rectangle", &json!({"parent": f, "x": 0, "y": 0, "width": 10, "height": 10})).unwrap());
        s.execute("node.set", &json!({"ids": [l], "props": {"constraints": {"horizontal": "right", "vertical": "bottom"}}})).unwrap();
        s.execute("game.anchors.auto", &json!({"ids": [f]})).unwrap();
        assert_eq!(s.doc().unwrap().node(l).unwrap().constraints.horizontal, HConstraint::Left, "a frame's Auto covers its layers");
    }
}
