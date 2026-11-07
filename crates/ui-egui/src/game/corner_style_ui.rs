//! Graybox: the Corner style picker in the Design panel, under Corner radius. A dropdown whose
//! rows each show a small drawing of the corner (drawn here from lines, original art) and its
//! name; picking one sets `cornerStyle` on the selected rectangles and frames. Shown only when the
//! selection has corners to shape (a radius above 0, or a style already set).

use egui::{Color32, CornerRadius, Painter, Pos2, Rect, Sense, Shape, Stroke, Ui, pos2, vec2};
use figcraft_doc::{CornerStyle, Document, NodeId};
use serde_json::json;

use crate::FigApp;
use crate::right::{common, menu_popup};
use crate::theme::{Tokens, regular};
use crate::widgets::{self, DropLead};

/// The Corner style rows for `sel` (every layer has a radius per corner), `col` a panel column wide.
pub(crate) fn section(app: &mut FigApp, ui: &mut Ui, doc: &Document, sel: &[NodeId], col: f32) {
    let shaped = sel.iter().filter_map(|id| doc.node(*id)).any(|n| !n.corners.style.is_round() || n.corners.radii.iter().any(|r| *r > 0.0));
    if !shaped {
        return;
    }
    let ids = sel.to_vec();
    let style = common(doc, sel, |n| n.corners.style);
    widgets::sublabels(ui, "Corner style", None);
    crate::right::row(ui, 32.0, |ui| {
        let label = style.map_or("Mixed", CornerStyle::label);
        // A transparent dot keeps the room for the drawing painted over it.
        let resp = widgets::dropdown_lead(ui, label, col * 2.0 + 8.0, Some(DropLead::Dot(Color32::TRANSPARENT)));
        if let Some(s) = style {
            let t = Tokens::get(ui.ctx());
            let at = Rect::from_center_size(pos2(resp.rect.min.x + 13.0, resp.rect.center().y), vec2(14.0, 14.0));
            paint_icon(ui.painter(), at, s, t.icon_secondary);
        }
        menu_popup(&resp).width((col * 2.0 + 8.0).max(140.0)).show(|ui| {
            for s in CornerStyle::ALL {
                if style_row(ui, s, style == Some(s)).clicked() {
                    app.set_props(&ids, json!({"cornerStyle": s.id()}));
                    ui.close();
                }
            }
        });
    });
}

/// Whether corner smoothing does anything for `sel`: it shapes round corners only (polygons and
/// stars have no style).
pub(crate) fn smoothing_applies(doc: &Document, sel: &[NodeId]) -> bool {
    sel.iter().filter_map(|id| doc.node(*id)).any(|n| n.corners.style.is_round() || !n.kind.has_independent_corners())
}

/// A menu row: a check mark when `on`, the corner's drawing and its name.
fn style_row(ui: &mut Ui, style: CornerStyle, on: bool) -> egui::Response {
    let t = Tokens::get(ui.ctx());
    let label = style.label();
    let text = ui.painter().layout_no_wrap(label.to_owned(), regular(11.5), Color32::PLACEHOLDER);
    let w = ui.available_width().max(8.0 + 14.0 + 20.0 + text.size().x + 8.0);
    let (r, resp) = ui.allocate_exact_size(vec2(w, 26.0), Sense::click());
    crate::a11y::menu_item(&resp, label, "", Some(on));
    crate::a11y::own_focus(&resp);
    let hovered = resp.hovered() || resp.has_focus();
    if hovered {
        ui.painter().rect_filled(r, CornerRadius::same(5), t.menu_hover);
    }
    let col = if hovered { t.menu_text } else { t.text };
    if on {
        let check = ui.painter().layout_no_wrap("✓".to_owned(), regular(11.5), col);
        ui.painter().galley(pos2(r.min.x + 8.0, r.center().y - check.size().y / 2.0), check, col);
    }
    paint_icon(ui.painter(), Rect::from_center_size(pos2(r.min.x + 29.0, r.center().y), vec2(14.0, 14.0)), style, col);
    ui.painter().galley(pos2(r.min.x + 42.0, r.center().y - text.size().y / 2.0), text, col);
    resp
}

/// The top-left corner of a shape in `style`, drawn as one line in `rect`: up the left side,
/// round the corner, along the top.
pub(crate) fn paint_icon(painter: &Painter, rect: Rect, style: CornerStyle, color: Color32) {
    let side = rect.width().min(rect.height());
    let o = rect.min + vec2(1.0, 1.0);
    let len = side - 2.0;
    let r = len * 0.7;
    let at = |x: f32, y: f32| -> Pos2 { o + vec2(x, y) };
    let mut pts = vec![at(0.0, len), at(0.0, r)];
    let quarter = std::f32::consts::FRAC_PI_2;
    match style {
        CornerStyle::Round => {
            pts.extend((1..8).map(|i| {
                let a = std::f32::consts::PI + quarter * i as f32 / 8.0;
                at(r + r * a.cos(), r + r * a.sin())
            }));
            pts.push(at(r, 0.0));
        }
        CornerStyle::Chamfer => pts.push(at(r, 0.0)),
        CornerStyle::Scoop => {
            pts.extend((1..8).map(|i| {
                let a = quarter - quarter * i as f32 / 8.0;
                at(r * a.cos(), r * a.sin())
            }));
            pts.push(at(r, 0.0));
        }
        CornerStyle::Notch => pts.extend([at(r, r), at(r, 0.0)]),
        CornerStyle::VNotch => {
            pts.pop();
            pts.extend([at(0.0, r / 2.0), at(r, r), at(r / 2.0, 0.0)]);
        }
    }
    pts.push(at(len, 0.0));
    painter.add(Shape::line(pts, Stroke::new(1.3, color)));
}
