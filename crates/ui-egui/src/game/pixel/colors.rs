//! Pixel colours: the Design panel section under Selection colours that lists the colours the
//! selected pixel layers (and pixel layers inside the selected frames) are drawn with. Changing one
//! replaces it in every layer and frame of those sprites, as one undo step.

use std::collections::BTreeMap;
use std::sync::Arc;

use egui::{Align2, Color32, CornerRadius, Id, Sense, Stroke, Ui, vec2};
use figcraft_doc::{Document, NodeId};
use figcraft_pixel::{Rgba, canvas, store};
use serde_json::json;

use crate::FigApp;
use crate::theme::Tokens;
use crate::widgets;

/// The most colours the section lists.
const MAX_ROWS: usize = 32;

/// The pixel layers among `sel` and inside it.
fn pixel_layers(doc: &Document, sel: &[NodeId]) -> Vec<NodeId> {
    let mut out = Vec::new();
    let mut todo: Vec<NodeId> = sel.iter().rev().copied().collect();
    let mut seen = std::collections::BTreeSet::new();
    while let Some(id) = todo.pop() {
        if out.len() >= 256 || seen.len() >= 100_000 {
            break;
        }
        if !seen.insert(id) {
            continue;
        }
        let Some(n) = doc.node(id) else { continue };
        if store::is_pixel(&n.plugin_data) {
            out.push(id);
        }
        todo.extend(n.children.iter().rev().copied());
    }
    out
}

/// The colours of those layers' sprites (every layer and frame), most used first.
fn colors_of(doc: &Document, layers: &[NodeId]) -> Vec<Rgba> {
    let mut count: BTreeMap<Rgba, usize> = BTreeMap::new();
    for id in layers {
        let Some(n) = doc.node(*id) else { continue };
        let Some(sprite) = store::load(&n.plugin_data) else { continue };
        for l in &sprite.layers {
            for f in &sprite.frames {
                if !n.plugin_data.contains_key(&store::cel_key(l.id, f.id)) {
                    continue;
                }
                let Some(cel) = store::cel(&n.plugin_data, &sprite, l.id, f.id) else { continue };
                for p in cel.pixels().iter().filter(|p| p[3] > 0) {
                    *count.entry(*p).or_default() += 1;
                }
            }
        }
    }
    let mut v: Vec<(Rgba, usize)> = count.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    v.into_iter().map(|(c, _)| c).collect()
}

/// The open picker: which colour it edits and the colour it has now.
#[derive(Clone, Copy)]
struct Editing {
    from: Rgba,
    to: Rgba,
}

/// The section, when the selection holds pixel layers. Returns whether it drew.
pub(crate) fn section(app: &mut FigApp, ui: &mut Ui, doc: &Arc<Document>, sel: &[NodeId]) -> bool {
    // Worked out again only when the document or the selection changes.
    let (layers, colors) = crate::selection_cache::cached(ui.ctx(), "graybox_pixel_colors", doc, sel, &[], || {
        let layers = pixel_layers(doc, sel);
        let colors = colors_of(doc, &layers);
        (layers, colors)
    });
    if layers.is_empty() || colors.is_empty() {
        return false;
    }
    let t = Tokens::get(ui.ctx());
    widgets::section_header(ui, "Pixel colors", &[], false);
    let edit_id = Id::unique("graybox_pixel_color_edit");
    let mut editing = ui.data(|d| d.get_temp::<Editing>(edit_id));
    let more = colors.len().saturating_sub(MAX_ROWS);
    let mut open_at = None;
    let row = super::panel::padded(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
            for c in colors.iter().take(MAX_ROWS) {
                let (r, resp) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
                let shown = editing.filter(|e| e.from == *c).map_or(*c, |e| e.to);
                ui.painter().rect_filled(r, CornerRadius::same(4), Color32::from_rgba_unmultiplied(shown[0], shown[1], shown[2], shown[3]));
                let open = editing.is_some_and(|e| e.from == *c);
                if open {
                    open_at = Some(r);
                }
                ui.painter().rect_stroke(
                    r,
                    CornerRadius::same(4),
                    Stroke::new(if open { 2.0 } else { 1.0 }, if open { t.accent } else { t.border_strong }),
                    egui::StrokeKind::Inside,
                );
                if resp.on_hover_text(format!("{}  ·  click to change it in the art", canvas::to_hex(*c))).clicked() {
                    editing = if open { None } else { Some(Editing { from: *c, to: *c }) };
                    open_at = editing.map(|_| r);
                }
            }
        })
        .response
        .rect
    });
    if more > 0 {
        widgets::note(ui, &format!("and {more} more"), t.text_secondary);
    }
    if let (Some(e), Some(swatch)) = (editing, open_at) {
        // The picker floats beside the panel, level with the swatch (it is wider than the panel).
        let anchor = egui::pos2(ui.max_rect().left() - 8.0, swatch.top());
        editing = popover(app, ui, &layers, e, anchor, row, &t);
    } else {
        editing = None;
    }
    match editing {
        Some(e) => ui.data_mut(|d| {
            d.insert_temp(edit_id, e);
        }),
        None => ui.data_mut(|d| {
            d.remove::<Editing>(edit_id);
        }),
    }
    true
}

/// The picker for the colour being changed: Replace swaps it in the art, Cancel (or a click
/// elsewhere) closes it. The colour still being edited, or None when it closed.
fn popover(app: &mut FigApp, ui: &Ui, layers: &[NodeId], mut e: Editing, anchor: egui::Pos2, row: egui::Rect, t: &Tokens) -> Option<Editing> {
    let mut done = false;
    let mut cancel = false;
    let area = egui::Area::new(Id::unique("graybox_pixel_color_popover")).order(egui::Order::Foreground).pivot(Align2::RIGHT_TOP).fixed_pos(anchor);
    let shown = area.show(ui.ctx(), |ui| {
        egui::Frame::popup(ui.style()).fill(t.panel).show(ui, |ui| {
            ui.set_width(220.0);
            ui.label(format!("Replace {} in the art", canvas::to_hex(e.from)));
            let mut c = Color32::from_rgba_unmultiplied(e.to[0], e.to[1], e.to[2], e.to[3]);
            if egui::color_picker::color_picker_color32(ui, &mut c, egui::color_picker::Alpha::OnlyBlend) {
                e.to = c.to_srgba_unmultiplied();
            }
            ui.horizontal(|ui| {
                let (r, resp) = ui.allocate_exact_size(vec2(96.0, 24.0), Sense::click());
                ui.painter().rect_filled(r, CornerRadius::same(5), if resp.hovered() { t.accent.gamma_multiply(0.85) } else { t.accent });
                ui.painter().text(r.center(), Align2::CENTER_CENTER, "Replace", crate::theme::semibold(11.5), Color32::WHITE);
                done = resp.clicked();
                let (r2, resp2) = ui.allocate_exact_size(vec2(64.0, 24.0), Sense::click());
                ui.painter().rect_filled(r2, CornerRadius::same(5), if resp2.hovered() { t.hover } else { t.field });
                ui.painter().text(r2.center(), Align2::CENTER_CENTER, "Cancel", crate::theme::regular(11.5), t.text);
                cancel = resp2.clicked();
            });
        });
    });
    let outside =
        ui.input(|i| i.pointer.any_pressed() && i.pointer.press_origin().is_some_and(|p| !shown.response.rect.contains(p) && !row.contains(p)));
    let escape = ui.input(|i| i.key_pressed(egui::Key::Escape));
    if done {
        replace(app, layers, e.from, e.to);
        return None;
    }
    (!cancel && !outside && !escape).then_some(e)
}
/// `from` becomes `to` in every sprite of `layers` (one undo step).
fn replace(app: &mut FigApp, layers: &[NodeId], from: Rgba, to: Rgba) {
    if from == to {
        return;
    }
    let ids: Vec<u64> = layers.iter().map(|id| id.0).collect();
    app.run("pixel.replaceColor", json!({"ids": ids, "from": canvas::to_hex(from), "to": canvas::to_hex(to)}));
}
