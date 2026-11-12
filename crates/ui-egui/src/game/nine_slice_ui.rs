//! Graybox: the 9-slice section of the Design panel (under Layout) and its cuts on the canvas.
//! The section's + turns 9-slice on (− turns it off) and shows the four cuts as fields; on the
//! canvas the selected layer shows its cuts as dashed lines, each one dragged to move it.

use egui::{Id, Pos2, Rect, Stroke, Ui};
use figcraft_doc::nine_slice::NineSlice;
use figcraft_doc::{Document, NodeId, NodeKind};
use serde_json::json;

use crate::FigApp;
use crate::icons::Icon;
use crate::widgets::{self, Lead};

/// Whether 9-slice can be set on `id` (as `game.nineSlice.set` takes it).
fn sliceable(doc: &Document, id: NodeId) -> bool {
    doc.node(id).is_some_and(|n| {
        // A pixel layer slices in art pixels, auto layout or not (G16.3).
        if art_scale(doc, id).is_some() {
            return true;
        }
        !n.is_page() && !matches!(n.kind, NodeKind::Text(_) | NodeKind::Section { .. }) && !n.kind.bounds_follow_children() && n.layout.is_none()
    })
}

/// Page units per art pixel when `id` is a pixel layer (not an instance: its main is sliced).
fn art_scale(doc: &Document, id: NodeId) -> Option<f64> {
    let n = doc.node(id).filter(|n| !matches!(n.kind, NodeKind::Instance { .. }))?;
    figcraft_pixel::store::load(&n.plugin_data).map(|s| f64::from(s.scale.max(1)))
}

/// The Design panel section for one selected layer that can be 9-sliced. Returns whether it drew.
pub(crate) fn section(app: &mut FigApp, ui: &mut Ui, doc: &Document, sel: &[NodeId]) -> bool {
    let [id] = sel else { return false };
    if !sliceable(doc, *id) {
        return false;
    }
    let Some(n) = doc.node(*id) else { return false };
    let slice = NineSlice::of(n);
    let buttons: &[(Icon, &str)] =
        if slice.is_some() { &[(Icon::Minus, "Turn 9-slice off")] } else { &[(Icon::Plus, "9-slice: stretch only the middles")] };
    if widgets::section_header(ui, "9-slice", buttons, slice.is_none()).is_some() {
        app.run("game.nineSlice.set", json!({"ids": [id], "enabled": slice.is_none()}));
    }
    let Some(s) = slice else { return true };
    let col = widgets::col_width(ui.available_width());
    // A pixel layer's cuts read in art pixels.
    let unit = art_scale(doc, *id).unwrap_or(1.0);
    let [t, r, b, l] = s.insets.map(|v| v / unit);
    let set = |app: &mut FigApp, insets: [f64; 4]| {
        app.run("game.nineSlice.set", json!({"ids": [id], "insets": insets}));
    };
    for (row, pair) in [[("Left", 3usize, l), ("Top", 0, t)], [("Right", 1, r), ("Bottom", 2, b)]].into_iter().enumerate() {
        crate::right::row(ui, 32.0, |ui| {
            for (label, k, v) in pair {
                let edit = widgets::number_field(ui, Id::unique(("graybox_nine_slice", row, k)), Lead::Text(label), Some(v), "", col, 1.0);
                if let Some(widgets::FieldEdit::Set(nv) | widgets::FieldEdit::Scrub(nv, true)) = edit {
                    let mut insets = s.insets;
                    if let Some(slot) = insets.get_mut(k) {
                        *slot = nv.max(0.0) * unit;
                    }
                    set(app, insets);
                }
            }
        });
    }
    let (bw, bh) = s.base;
    let note = if art_scale(doc, *id).is_some() {
        "In art pixels: the edges and middle repeat or stretch by whole pixels".to_string()
    } else {
        format!("Drawn at {bw:.0} × {bh:.0}: drag the cuts on the canvas")
    };
    widgets::note(ui, &note, crate::theme::Tokens::get(ui.ctx()).text_secondary);
    true
}

/// The one selected 9-sliced layer, unrotated: its id, world transform, size and 9-slice.
fn target(app: &FigApp) -> Option<(NodeId, figcraft_geom::Affine, (f64, f64), NineSlice)> {
    let ds = app.session.ds().ok()?;
    let [id] = ds.selection[..] else { return None };
    if ds.vector_edit.is_some() || ds.text_edit.is_some() {
        return None;
    }
    let n = ds.doc.node(id)?;
    let s = NineSlice::of(n)?;
    let world = ds.doc.world_transform(id);
    (figcraft_geom::rotation_degrees(world).abs() <= 0.01).then_some((id, world, (n.width, n.height), s))
}

/// The four cuts on screen: (side 0 top, 1 right, 2 bottom, 3 left; one end; the other).
fn cuts(app: &FigApp, world: figcraft_geom::Affine, (w, h): (f64, f64), s: &NineSlice) -> [(usize, Pos2, Pos2); 4] {
    let [t, r, b, l] = s.insets;
    let to = |x: f64, y: f64| {
        let p = world * figcraft_geom::Point::new(x, y);
        app.to_screen(p.x, p.y)
    };
    [(0, to(0.0, t), to(w, t)), (1, to(w - r, 0.0), to(w - r, h)), (2, to(0.0, h - b), to(w, h - b)), (3, to(l, 0.0), to(l, h))]
}

fn drag_id() -> Id {
    Id::unique("graybox_nine_slice_drag")
}

/// Draw the selected 9-sliced layer's cuts (dashed; the one under the pointer or dragged bolder).
pub(crate) fn draw(app: &FigApp, painter: &egui::Painter) {
    let Some((_, world, size, s)) = target(app) else { return };
    let ctx = painter.ctx();
    let dragged: Option<usize> = ctx.data(|d| d.get_temp(drag_id()));
    let pointer = ctx.input(|i| i.pointer.hover_pos());
    for (side, a, z) in cuts(app, world, size, &s) {
        let near = pointer.is_some_and(|p| Rect::from_two_pos(a, z).expand(4.0).contains(p));
        let hot = near || dragged == Some(side);
        let col = crate::theme::ACCENT;
        dashed(painter, a, z, Stroke::new(if hot { 2.0 } else { 1.0 }, if hot { col } else { col.gamma_multiply(0.8) }));
    }
}

/// The pointer on the canvas at `pos`: a press on a cut drags it, the drag moves it, the release
/// ends it (one undo step). True when the pointer belongs to a cut (the tools leave it alone).
pub(crate) fn input(app: &mut FigApp, ctx: &egui::Context, pos: Pos2) -> bool {
    let (pressed, down, released) = ctx.input(|i| {
        (i.pointer.button_pressed(egui::PointerButton::Primary), i.pointer.primary_down(), i.pointer.button_released(egui::PointerButton::Primary))
    });
    let dragged: Option<usize> = ctx.data(|d| d.get_temp(drag_id()));
    let Some((id, world, (w, h), s)) = target(app) else {
        if dragged.is_some() {
            ctx.data_mut(|d| d.remove::<usize>(drag_id()));
            let _ = app.session.commit_interaction();
        }
        return false;
    };
    if let Some(side) = dragged {
        ctx.set_cursor_icon(if side % 2 == 0 { egui::CursorIcon::ResizeVertical } else { egui::CursorIcon::ResizeHorizontal });
        if down {
            let p = app.to_page(pos);
            let local = world.inverse() * p;
            let v = match side {
                0 => local.y,
                1 => w - local.x,
                2 => h - local.y,
                _ => local.x,
            };
            let mut insets = s.insets;
            if let Some(slot) = insets.get_mut(side) {
                *slot = v.round().max(0.0);
            }
            if insets != s.insets {
                let _ = app.session.execute("game.nineSlice.set", &json!({"ids": [id], "insets": insets}));
            }
        }
        if released || !down {
            ctx.data_mut(|d| d.remove::<usize>(drag_id()));
            let _ = app.session.commit_interaction();
        }
        return true;
    }
    let hit = cuts(app, world, (w, h), &s).into_iter().find(|(_, a, z)| Rect::from_two_pos(*a, *z).expand(4.0).contains(pos)).map(|c| c.0);
    let Some(side) = hit else { return false };
    ctx.set_cursor_icon(if side % 2 == 0 { egui::CursorIcon::ResizeVertical } else { egui::CursorIcon::ResizeHorizontal });
    if pressed {
        let _ = app.session.begin_interaction("9-slice");
        ctx.data_mut(|d| d.insert_temp(drag_id(), side));
    }
    true
}

/// A dashed line from `a` to `z`.
fn dashed(painter: &egui::Painter, a: Pos2, z: Pos2, stroke: Stroke) {
    let len = a.distance(z);
    if len < 1.0 {
        return;
    }
    let dir = (z - a) / len;
    let (dash, gap) = (4.0, 3.0);
    let mut s = 0.0;
    while s < len {
        let e = (s + dash).min(len);
        painter.line_segment([a + dir * s, a + dir * e], stroke);
        s += dash + gap;
    }
}
