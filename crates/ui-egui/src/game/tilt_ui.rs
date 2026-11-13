//! Graybox: 3D rotation of layers (G17) in the editor. The Design panel's "3D" section (X, Y,
//! Perspective), outlines and the selection box drawn through the tilt, the round handle that turns
//! a layer in 3D, the canvas picture with tilted layers shown flat while a layer inside them is
//! selected ("edit flat"), and the tilt in smart animate.

use std::sync::{Arc, Mutex, PoisonError};

use egui::{Id, Pos2, Stroke, Ui, Vec2};
use figcraft_doc::tilt::{self, Homography, KEY, Tilt};
use figcraft_doc::{Document, Node, NodeId, NodeKind};
use figcraft_engine::tools::ToolId;
use figcraft_geom::Point;
use serde_json::json;

use crate::FigApp;
use crate::icons::Icon;
use crate::widgets::{self, Lead};

/// The round handle's distance above the layer's drawn top edge, and its reach (screen points).
const KNOB_GAP: f32 = 18.0;
const KNOB_REACH: f32 = 9.0;
/// Degrees a dragged point of the handle turns the layer.
const DEG_PER_POINT: f64 = 0.5;

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
    let note = "Drag the round handle above it to turn it (Shift: 5° steps). Y below 0 brings the left side near, X above 0 tips the top away (as in Unity). Smaller perspective: stronger. Shown flat while you edit inside it.";
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

/// The warp the canvas draws layer `id` (and what is inside it) through while `selection` is
/// selected: [`tilt::page_warp`] without the layers shown flat for editing.
fn shown_warp(doc: &Document, selection: &[NodeId], id: NodeId) -> Option<Homography> {
    // Most layers have no tilt above them: that answer needs no look at the selection.
    tilt::page_warp(doc, id, &[])?;
    tilt::page_warp(doc, id, &tilt::editing_flat(doc, selection))
}

/// [`shown_warp`] for the canvas's selection: the outlines of layer `id` go through it.
pub(crate) fn warp(app: &FigApp, doc: &Document, id: NodeId) -> Option<Homography> {
    let ds = app.session.ds().ok()?;
    shown_warp(doc, &ds.selection, id)
}

/// Page point `p` through warp `h`, where the canvas draws it (itself without one, or where it
/// can't be drawn).
pub(crate) fn drawn(h: Option<&Homography>, p: Point) -> Point {
    h.and_then(|h| h.apply(p)).unwrap_or(p)
}

/// The selection's box `b` (tl, tr, br, bl, flat) where the canvas draws it: one selected tilted
/// layer's box is its drawn corners, so its handles sit on them.
pub(crate) fn drawn_box(app: &FigApp, doc: &Document, sel: &[NodeId], b: Option<[Point; 4]>) -> Option<[Point; 4]> {
    let [id] = sel else { return b };
    let Ok(ds) = app.session.ds() else { return b };
    let h = shown_warp(doc, &ds.selection, *id);
    b.map(|b| b.map(|p| drawn(h.as_ref(), p)))
}

/// The round handle: the one selected layer with 3D on, its tilt and the handle on screen (above
/// its drawn top edge, away from its centre).
fn knob(app: &FigApp) -> Option<(NodeId, Tilt, Pos2, Pos2)> {
    let ds = app.session.ds().ok()?;
    let [id] = ds.selection[..] else { return None };
    if ds.vector_edit.is_some() || ds.text_edit.is_some() || app.session.tool.dev_mode || app.session.tool.tool != ToolId::Move {
        return None;
    }
    if crate::game::pixel::active(app) || figcraft_engine::tools::locked_in(&ds.doc, id) {
        return None;
    }
    let n = ds.doc.node(id).filter(|n| turnable(n))?;
    let t = stored(n)?;
    let world = ds.doc.world_transform(id);
    let h = shown_warp(&ds.doc, &ds.selection, id);
    let at = |x: f64, y: f64| {
        let p = drawn(h.as_ref(), world * Point::new(x, y));
        app.to_screen(p.x, p.y)
    };
    let (top, centre) = (at(n.width / 2.0, 0.0), at(n.width / 2.0, n.height / 2.0));
    let up = (top - centre).normalized();
    let up = if up.is_finite() && up.length() > 0.5 { up } else { Vec2::new(0.0, -1.0) };
    Some((id, t, top, top + up * KNOB_GAP))
}

/// The tilt `start` dragged by `d` screen points, like a trackball: right turns the face right (Y
/// down: the left side comes near), down turns it down (X down: the top comes near). `snap` keeps
/// whole 5° steps.
fn dragged(start: Tilt, d: Vec2, snap: bool) -> Tilt {
    let turn = |from: f64, by: f32| {
        let v = from - f64::from(by) * DEG_PER_POINT;
        if snap { (v / 5.0).round() * 5.0 } else { v.round() }
    };
    Tilt::new(turn(start.x, d.y), turn(start.y, d.x), start.perspective)
}

fn drag_id() -> Id {
    Id::unique("graybox_tilt_knob_drag")
}

/// The round handle is being dragged: it keeps the pointer until the release, wherever that is.
pub(crate) fn dragging(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<(NodeId, Tilt, Pos2)>(drag_id())).is_some()
}

/// The round handle that turns the selected layer in 3D (with its stem), and while it is dragged
/// the angles.
pub(crate) fn draw(app: &FigApp, painter: &egui::Painter) {
    let Some((_, t, top, at)) = knob(app) else { return };
    let col = crate::theme::ACCENT;
    let ctx = painter.ctx();
    let turning = dragging(ctx);
    let hot = turning || ctx.input(|i| i.pointer.hover_pos()).is_some_and(|p| p.distance(at) <= KNOB_REACH);
    painter.line_segment([top, at], Stroke::new(1.0, col));
    painter.circle_filled(at, if hot { 7.0 } else { 6.0 }, egui::Color32::WHITE);
    painter.circle_stroke(at, if hot { 7.0 } else { 6.0 }, Stroke::new(1.5, col));
    // An orbit through it: what the handle does.
    painter.line_segment([at - Vec2::new(4.0, 0.0), at + Vec2::new(4.0, 0.0)], Stroke::new(1.0, col));
    painter.circle_filled(at, 1.8, col);
    if turning {
        crate::handles::pink_label(painter, format!("X {}°  Y {}°", t.x, t.y), at - Vec2::new(0.0, 18.0));
    }
}

/// The pointer on the canvas at `pos`: a press on the round handle starts turning the layer, the
/// drag turns it, the release ends it (one undo step). True when the pointer belongs to it.
pub(crate) fn input(app: &mut FigApp, ctx: &egui::Context, pos: Pos2) -> bool {
    let (pressed, down, shift) = ctx.input(|i| (i.pointer.button_pressed(egui::PointerButton::Primary), i.pointer.primary_down(), i.modifiers.shift));
    let held: Option<(NodeId, Tilt, Pos2)> = ctx.data(|d| d.get_temp(drag_id()));
    if let Some((id, start, from)) = held {
        ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
        let current = app.session.doc().ok().and_then(|d| d.node(id)).and_then(stored);
        if down && let Some(now) = current {
            let t = dragged(start, pos - from, shift);
            if t != now {
                let _ = app.session.execute("node.rotation3d.set", &json!({"ids": [id], "x": t.x, "y": t.y}));
            }
        } else {
            ctx.data_mut(|d| d.remove::<(NodeId, Tilt, Pos2)>(drag_id()));
            let _ = app.session.commit_interaction();
        }
        return true;
    }
    let Some((id, t, _, at)) = knob(app) else { return false };
    if at.distance(pos) > KNOB_REACH {
        return false;
    }
    ctx.set_cursor_icon(egui::CursorIcon::Grab);
    if pressed && app.session.begin_interaction("3D rotation").is_ok() {
        ctx.data_mut(|d| d.insert_temp(drag_id(), (id, t, pos)));
    }
    true
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

    /// A 100 × 40 frame at the origin, tilted Y −30° at perspective 500, with a rectangle in it.
    fn tilted() -> (Arc<Document>, NodeId, NodeId) {
        let mut d = Document::new("t");
        let p = d.first_page().unwrap();
        let f = d.create(p, NodeKind::Frame, "F", 0.0, 0.0, 100.0, 40.0).unwrap();
        let r = d.create(f, NodeKind::Rectangle, "R", 0.0, 0.0, 10.0, 10.0).unwrap();
        d.get_mut(f).unwrap().plugin_data.insert(KEY.into(), Tilt::new(0.0, -30.0, 500.0).encode());
        (Arc::new(d), f, r)
    }

    #[test]
    fn the_canvas_view_is_the_same_document_until_something_changes() {
        let (doc, f, r) = tilted();
        assert!(Arc::ptr_eq(&view(doc.clone(), &[f]), &doc), "the tilted layer itself selected: shown tilted");
        let a = view(doc.clone(), &[r]);
        assert!(tilt::of(a.node(f).unwrap()).is_none(), "shown flat while its layer is selected");
        assert!(Arc::ptr_eq(&a, &view(doc.clone(), &[r])), "the same picture again");
    }

    /// Outlines go through the tilt the canvas shows: a layer inside the tilted frame too, but not
    /// while it (or another layer inside) is selected and the frame shows flat.
    #[test]
    fn outlines_follow_the_tilt_the_canvas_shows() {
        let (doc, f, r) = tilted();
        let corner = Point::new(0.0, 0.0);
        let on_frame = drawn(shown_warp(&doc, &[f], f).as_ref(), corner);
        assert!(on_frame.y < -1.0, "the near (left) side drawn taller: {on_frame:?}");
        assert_eq!(drawn(shown_warp(&doc, &[], r).as_ref(), corner), on_frame, "a hovered layer inside goes through it too");
        assert_eq!(drawn(shown_warp(&doc, &[r], r).as_ref(), corner), corner, "flat while editing inside it");
        assert_eq!(drawn(shown_warp(&doc, &[r], f).as_ref(), corner), corner);
    }

    #[test]
    fn the_handle_turns_like_a_trackball_with_5_degree_steps_on_shift() {
        let t = Tilt::new(10.0, -20.0, 600.0);
        assert_eq!(dragged(t, Vec2::new(40.0, 0.0), false), Tilt::new(10.0, -40.0, 600.0), "right: the left side comes near");
        assert_eq!(dragged(t, Vec2::new(0.0, 30.0), false), Tilt::new(-5.0, -20.0, 600.0), "down: the top comes near");
        assert_eq!(dragged(t, Vec2::new(7.0, -9.0), true), Tilt::new(15.0, -25.0, 600.0));
        assert_eq!(dragged(t, Vec2::new(-1000.0, 1000.0), false), Tilt::new(-80.0, 80.0, 600.0), "kept within ±80°");
    }
}
