//! Graybox: 3D rotation of layers (G17) in the editor. The Design panel's "3D" section (X, Y,
//! Perspective), the tilted outline of a selected tilted layer on the canvas, the canvas picture
//! with tilted layers shown flat while a layer inside them is selected ("edit flat"), and the tilt
//! in smart animate.

use std::sync::{Arc, Mutex, PoisonError};

use egui::{Id, Pos2, Stroke, Ui};
use figcraft_doc::tilt::{self, KEY, Tilt};
use figcraft_doc::{Document, Node, NodeId, NodeKind};
use figcraft_geom::Point;
use serde_json::json;

use crate::FigApp;
use crate::icons::Icon;
use crate::widgets::{self, Lead};

/// Whether the 3D section shows for `n`: frames, components, instances and groups.
fn turnable(n: &Node) -> bool {
    !n.is_page() && (n.kind.is_frame_like() || matches!(n.kind, NodeKind::Group)) && !matches!(n.kind, NodeKind::ComponentSet { .. })
}

/// The stored tilt of `n`, flat ones included (the section shows its fields).
fn stored(n: &Node) -> Option<Tilt> {
    n.plugin_data.get(KEY).and_then(|s| Tilt::decode(s))
}

/// The Design panel section for one selected frame, component, instance or group. Returns whether
/// it drew.
pub(crate) fn section(app: &mut FigApp, ui: &mut Ui, doc: &Document, sel: &[NodeId]) -> bool {
    let [id] = sel else { return false };
    let Some(n) = doc.node(*id).filter(|n| turnable(n)) else { return false };
    let t = stored(n);
    let buttons: &[(Icon, &str)] =
        if t.is_some() { &[(Icon::Minus, "Remove 3D rotation")] } else { &[(Icon::Plus, "Turn in 3D (X, Y, perspective)")] };
    if widgets::section_header(ui, "3D", buttons, t.is_none()).is_some() {
        app.run("node.rotation3d.set", json!({"ids": [id], "enabled": t.is_none()}));
    }
    let Some(t) = t else { return true };
    let col = widgets::col_width(ui.available_width());
    let id = *id;
    crate::right::row(ui, 32.0, |ui| {
        let x = widgets::number_field(ui, Id::unique(("graybox_tilt", "x")), Lead::Text("X"), Some(t.x), "°", col, 1.0);
        app.field_run_named(x, "3D rotation", "node.rotation3d.set", |v| json!({"ids": [id], "x": v}));
        let y = widgets::number_field(ui, Id::unique(("graybox_tilt", "y")), Lead::Text("Y"), Some(t.y), "°", col, 1.0);
        app.field_run_named(y, "3D rotation", "node.rotation3d.set", |v| json!({"ids": [id], "y": v}));
    });
    crate::right::row(ui, 32.0, |ui| {
        let d = widgets::number_field(ui, Id::unique(("graybox_tilt", "perspective")), Lead::Text("Persp."), Some(t.perspective), "", col, 10.0);
        app.field_run_named(d, "3D rotation", "node.rotation3d.set", |v| json!({"ids": [id], "perspective": v}));
    });
    let note = "Y below 0 brings the left side near, X above 0 tips the top away (as in Unity). Smaller perspective: stronger. Shown flat while you edit inside it.";
    widgets::note(ui, note, crate::theme::Tokens::get(ui.ctx()).text_secondary);
    true
}

/// The canvas picture's document: tilted layers with a selected layer inside shown flat. The same
/// document comes back while nothing changes (the canvas re-renders when it is a new one).
pub(crate) fn view(doc: Arc<Document>, selection: &[NodeId]) -> Arc<Document> {
    /// The document asked for, its layers shown flat, and the picture given.
    type Shown = (Arc<Document>, Vec<NodeId>, Arc<Document>);
    static LAST: Mutex<Option<Shown>> = Mutex::new(None);
    let flat = tilt::editing_flat(&doc, selection);
    if flat.is_empty() {
        return doc;
    }
    let mut last = LAST.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some((input, ids, out)) = last.as_ref()
        && Arc::ptr_eq(input, &doc)
        && *ids == flat
    {
        return out.clone();
    }
    let out = tilt::view(doc.clone(), selection);
    *last = Some((doc, flat, out.clone()));
    out
}

/// Outline the selected tilted layers where they are drawn (their flat box keeps its handles).
pub(crate) fn draw(app: &FigApp, painter: &egui::Painter) {
    let Ok(ds) = app.session.ds() else { return };
    if ds.selection.len() > 64 {
        return;
    }
    let flat = tilt::editing_flat(&ds.doc, &ds.selection);
    for id in &ds.selection {
        if flat.contains(id) {
            continue;
        }
        let Some(n) = ds.doc.node(*id) else { continue };
        let Some(h) = tilt::local(n) else { continue };
        let world = ds.doc.world_transform(*id);
        let corners = [(0.0, 0.0), (n.width, 0.0), (n.width, n.height), (0.0, n.height)];
        let pts: Vec<Pos2> = corners
            .iter()
            .filter_map(|(x, y)| h.apply(Point::new(*x, *y)))
            .map(|p| {
                let q = world * p;
                app.to_screen(q.x, q.y)
            })
            .collect();
        if pts.len() == 4 {
            painter.add(egui::Shape::closed_line(pts, Stroke::new(1.5, crate::theme::ACCENT)));
        }
    }
}

/// Smart animate: matched layer `to` (in the step being drawn) takes the tilt `t` of the way from
/// `from`'s. A layer without one counts as flat.
pub(crate) fn lerp(from: &Node, to: &mut Node, t: f64) {
    let (a, b) = (stored(from), stored(to));
    if a.is_none() && b.is_none() {
        return;
    }
    let flat = |o: Option<Tilt>, other: Option<Tilt>| o.unwrap_or(Tilt { x: 0.0, y: 0.0, ..other.unwrap_or_default() });
    let mid = Tilt::lerp(flat(a, b), flat(b, a), t);
    to.plugin_data.insert(KEY.into(), mid.encode());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with(t: Option<Tilt>) -> Node {
        let mut d = Document::new("t");
        let p = d.first_page().unwrap();
        let f = d.create(p, NodeKind::Frame, "F", 0.0, 0.0, 10.0, 10.0).unwrap();
        let mut n = d.node(f).unwrap().clone();
        if let Some(t) = t {
            n.plugin_data.insert(KEY.into(), t.encode());
        }
        n
    }

    #[test]
    fn smart_animate_turns_halfway_from_flat() {
        let from = with(None);
        let mut to = with(Some(Tilt::new(20.0, -40.0, 600.0)));
        lerp(&from, &mut to, 0.5);
        assert_eq!(stored(&to), Some(Tilt::new(10.0, -20.0, 600.0)));
        let mut plain = with(None);
        lerp(&from, &mut plain, 0.5);
        assert_eq!(stored(&plain), None);
    }

    #[test]
    fn the_canvas_view_is_the_same_document_until_something_changes() {
        let mut d = Document::new("t");
        let p = d.first_page().unwrap();
        let f = d.create(p, NodeKind::Frame, "F", 0.0, 0.0, 100.0, 40.0).unwrap();
        let r = d.create(f, NodeKind::Rectangle, "R", 0.0, 0.0, 10.0, 10.0).unwrap();
        d.get_mut(f).unwrap().plugin_data.insert(KEY.into(), Tilt::new(0.0, -30.0, 500.0).encode());
        let doc = Arc::new(d);
        assert!(Arc::ptr_eq(&view(doc.clone(), &[f]), &doc), "the tilted layer itself selected: shown tilted");
        let a = view(doc.clone(), &[r]);
        assert!(tilt::of(a.node(f).unwrap()).is_none(), "shown flat while its layer is selected");
        assert!(Arc::ptr_eq(&a, &view(doc.clone(), &[r])), "the same picture again");
    }
}
