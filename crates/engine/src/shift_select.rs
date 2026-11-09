//! Graybox: Shift-click adds the layer at the same depth as what is already selected, as in Figma.
//! With a frame selected inside one button of a list (list → button → frame), Shift-clicking a frame
//! of another button added the whole list instead (the plain click's level), which swallowed the
//! first frame. Now the clicked layer's ancestor at the selected layer's depth is added, when the
//! two share the same top-level frame.

use figcraft_doc::{Document, NodeId};

/// What a Shift-click on `leaf` adds, with `selection` selected: the layer holding `leaf` at the
/// depth of the deepest selected layer in the same top-level frame, else what a plain click picks.
pub(crate) fn target(doc: &Document, leaf: NodeId, selection: &[NodeId]) -> NodeId {
    let plain = figcraft_doc::geometry::select_target(doc, leaf, selection, false);
    // The chain from the page's child down to the leaf.
    let chain = |id: NodeId| {
        let mut c: Vec<NodeId> = doc.ancestors(id);
        c.reverse();
        c.push(id);
        c.into_iter().skip(1).collect::<Vec<_>>()
    };
    let hit = chain(leaf);
    let Some(&top) = hit.first() else { return plain };
    let depth = selection.iter().map(|s| chain(*s)).filter(|c| c.first() == Some(&top)).map(|c| c.len()).max();
    match depth.and_then(|d| hit.get(d.saturating_sub(1))) {
        // Deeper than a plain click goes, and not a layer holding (or held by) a selected one.
        Some(&same)
            if hit.iter().position(|c| *c == same) > hit.iter().position(|c| *c == plain)
                && !selection.iter().any(|s| doc.is_ancestor(same, *s) || doc.is_ancestor(*s, same)) =>
        {
            same
        }
        _ => plain,
    }
}

#[cfg(test)]
mod tests {
    use crate::Session;
    use crate::tools::{PointerEvent, PointerKind};
    use figcraft_doc::NodeId;
    use serde_json::json;

    fn id(v: &serde_json::Value) -> NodeId {
        NodeId(v["id"].as_u64().unwrap())
    }

    fn frame(s: &mut Session, parent: Option<NodeId>, x: f64, y: f64, w: f64, h: f64) -> NodeId {
        let mut p = json!({"x": x, "y": y, "width": w, "height": h});
        if let Some(parent) = parent {
            p["parent"] = json!(parent);
        }
        id(&s.execute("frame.create", &p).unwrap())
    }

    fn click(s: &mut Session, x: f64, y: f64, shift: bool) {
        for kind in [PointerKind::Down, PointerKind::Up] {
            let mut e = PointerEvent::at(kind, x, y, 1.0);
            e.shift = shift;
            s.pointer(&e).unwrap();
        }
    }

    /// A frame in one button of a list is selected; Shift-clicking a frame in another button adds
    /// that frame (same depth), not the whole list.
    #[test]
    fn shift_click_adds_the_layer_at_the_selected_depth() {
        let mut s = Session::new();
        let screen = frame(&mut s, None, 0.0, 0.0, 400.0, 400.0);
        let list = frame(&mut s, Some(screen), 0.0, 0.0, 300.0, 300.0);
        let b1 = frame(&mut s, Some(list), 0.0, 0.0, 200.0, 50.0);
        let b2 = frame(&mut s, Some(list), 0.0, 100.0, 200.0, 50.0);
        let a = frame(&mut s, Some(b1), 0.0, 0.0, 40.0, 40.0);
        let c = frame(&mut s, Some(b2), 0.0, 0.0, 40.0, 40.0);
        s.execute("select.set", &json!({"ids": [a]})).unwrap();
        click(&mut s, 20.0, 120.0, true);
        let mut sel = s.selection();
        sel.sort();
        assert_eq!(sel, vec![a, c]);
        // A plain click keeps upstream's rule (the list, the top frame's child).
        s.execute("select.set", &json!({"ids": [a]})).unwrap();
        click(&mut s, 20.0, 120.0, false);
        assert_eq!(s.selection(), vec![list]);
    }
}
