//! Layers of the file shown in a document (G13.2): a frame, component or variant drawn in the page,
//! live (it follows the layer as it changes) or pinned (a picture of it at one moment; closing a
//! version pins every live one). An embed with no layer yet asks for one from a searchable list.

use std::hash::{Hash, Hasher};
use std::sync::Arc;

use egui::{Align2, Color32, CornerRadius, Id, Key, Rect, Sense, Stroke, StrokeKind, TextureHandle, Ui, pos2, vec2};
use figcraft_doc::docs::{BlockId, DocId};
use figcraft_doc::{Document, NodeId, NodeKind};
use serde_json::json;

use super::state::State;
use crate::FigApp;
use crate::a11y;
use crate::i18n::tr;
use crate::theme::{Tokens, regular};

/// The tallest a layer shows, in points.
const MAX_H: f32 = 560.0;
/// The most layers the list offers.
const MAX_LISTED: usize = 300;
/// The most layers walked to tell whether a live layer changed (past it, any edit redraws it).
const MAX_WALKED: usize = 5_000;

/// The list of layers while it is open: for which block, under which rectangle, and what is typed.
#[derive(Clone, Debug)]
pub(super) struct Pick {
    pub doc: DocId,
    pub block: BlockId,
    pub under: Rect,
    pub query: String,
    pub focus: bool,
}

/// Whether `node` is a layer an embed can show: on a page, not a page itself.
fn showable(doc: &Document, node: NodeId) -> bool {
    doc.node(node).is_some_and(|n| n.parent.is_some() && !matches!(n.kind, NodeKind::Page { .. }))
}

/// Embed block `block` showing layer `node` (`live`, or the pinned picture `snapshot`), its left edge
/// `indent` into the column `col`.
#[allow(clippy::too_many_arguments)]
pub(super) fn show(
    app: &mut FigApp,
    ui: &mut Ui,
    st: &mut State,
    t: &Tokens,
    doc: DocId,
    block: BlockId,
    (node, live, snapshot): (NodeId, bool, Option<&str>),
    col: Rect,
    indent: f32,
    editable: bool,
) {
    let room = (col.width() - indent).max(40.0);
    let ppp = ui.ctx().pixels_per_point();
    // What to draw: a texture and its size in points, and the layer's name.
    let (drawn, name) = {
        let Ok(file) = app.session.doc() else { return };
        let name = file.node(node).map(|n| n.name.clone());
        let drawn = if live {
            showable(file, node).then(|| live_texture(ui.ctx(), file, node, room, ppp)).flatten()
        } else {
            snapshot.and_then(|key| {
                let i = file.images.get(key)?;
                // Pinned pictures are drawn at twice the layer's size.
                let size = vec2(i.width as f32 / 2.0, i.height as f32 / 2.0);
                let k = fit(size, room);
                let tex = super::media::texture(app, ui.ctx(), key, f64::from(size.x.max(size.y) * k * ppp))?;
                Some((tex, size * k))
            })
        };
        (drawn, name)
    };
    let Some((tex, size)) = drawn else {
        let deleted = node.0 != 0 && name.is_none();
        empty(ui, st, t, doc, block, col, indent, editable, deleted);
        return;
    };
    let (row, resp) = ui.allocate_exact_size(vec2(col.width(), size.y + 30.0), Sense::hover());
    let r = Rect::from_min_size(pos2(row.min.x + indent, row.min.y + 4.0), size);
    let label = name.clone().unwrap_or_default();
    a11y::image(&resp, &label);
    ui.painter().image(tex.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    ui.painter().rect_stroke(r, CornerRadius::same(2), Stroke::new(1.0, t.border), StrokeKind::Outside);
    // Under it: the layer's name, and whether it follows the layer.
    let y = r.max.y + 13.0;
    let (dot, state) = if live { (t.accent, tr("Live")) } else { (t.text_tertiary, tr("Pinned")) };
    ui.painter().circle_filled(pos2(r.min.x + 4.0, y), 3.0, dot);
    let shown = ui.painter().text(pos2(r.min.x + 12.0, y), Align2::LEFT_CENTER, state, regular(11.0), t.text_secondary);
    ui.painter().text(pos2(shown.max.x + 8.0, y), Align2::LEFT_CENTER, label, regular(11.0), t.text_tertiary);
}

/// The scale that makes `size` fit `room` points across and [`MAX_H`] down, never larger.
fn fit(size: egui::Vec2, room: f32) -> f32 {
    if size.x <= 0.0 || size.y <= 0.0 {
        return 1.0;
    }
    (room / size.x).min(MAX_H / size.y).min(1.0)
}

/// Layer `node` drawn now, fitted to `room` points across, kept between frames until it (or what it
/// shows of a component) changes: the texture and its size in points.
fn live_texture(ctx: &egui::Context, file: &Document, node: NodeId, room: f32, ppp: f32) -> Option<(TextureHandle, egui::Vec2)> {
    let b = figcraft_render::export_bounds(file, node)?;
    let size = vec2(b.width() as f32, b.height() as f32);
    if !(size.x.is_finite() && size.y.is_finite()) || size.x <= 0.0 || size.y <= 0.0 {
        return None;
    }
    let k = fit(size, room);
    // Under the graphics card's limit, with room for the renderer rounding its size up.
    let max = (ctx.input(|i| i.max_texture_side) as f32 - 4.0).max(16.0);
    let scale = (k * ppp).min(max / size.x.max(size.y)).max(1e-3);
    let key = {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        changes(file, node, &mut h);
        scale.to_bits().hash(&mut h);
        h.finish()
    };
    let id = Id::unique(("gbx_doc_embed_tex", node.0));
    if let Some((_, tex)) = ctx.data(|d| d.get_temp::<(u64, TextureHandle)>(id)).filter(|(k, _)| *k == key) {
        return Some((tex, size * k));
    }
    let pix = figcraft_render::render_node(file, node, f64::from(scale))?;
    let img = crate::render_worker::screen_image(&pix, file.color_profile);
    let tex = ctx.load_texture(format!("gbx-doc-embed-{}", node.0), img, egui::TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, (key, tex.clone())));
    Some((tex, size * k))
}

/// Feed `h` with what layer `node` shows: its layers (a copy-on-write file gives a changed layer a new
/// address) and the main components its instances show. Past [`MAX_WALKED`] layers, the whole file.
fn changes(file: &Document, node: NodeId, h: &mut impl Hasher) {
    let mut stack = vec![node];
    let mut seen = std::collections::BTreeSet::new();
    while let Some(i) = stack.pop() {
        if !seen.insert(i) {
            continue;
        }
        if seen.len() > MAX_WALKED {
            (std::ptr::from_ref(file) as usize).hash(h);
            file.nodes().count().hash(h);
            return;
        }
        let Some(n) = file.node_arc(i) else { continue };
        (Arc::as_ptr(n) as usize).hash(h);
        stack.extend(n.children.iter().copied());
        if let NodeKind::Instance { component, .. } = n.kind {
            stack.push(component);
        }
    }
    file.images.len().hash(h);
}

/// An embed with no layer to show: a button to choose one (saying so when its layer was deleted), or
/// a dim box on a version that cannot change.
#[allow(clippy::too_many_arguments)]
fn empty(ui: &mut Ui, st: &mut State, t: &Tokens, doc: DocId, block: BlockId, col: Rect, indent: f32, editable: bool, deleted: bool) {
    let (row, _) = ui.allocate_exact_size(vec2(col.width(), 80.0), Sense::hover());
    let r = Rect::from_min_max(pos2(row.min.x + indent, row.min.y + 4.0), pos2(row.max.x, row.max.y - 4.0));
    ui.painter().rect_filled(r, CornerRadius::same(6), t.field);
    ui.painter().rect_stroke(r, CornerRadius::same(6), Stroke::new(1.0, t.border), StrokeKind::Inside);
    let note = if deleted { tr("This layer was deleted") } else { tr("Show a frame or component of this file") };
    if !editable {
        ui.painter().text(r.center(), Align2::CENTER_CENTER, note, regular(12.0), t.text_tertiary);
        return;
    }
    let label = tr("Choose a layer");
    let galley = ui.painter().layout_no_wrap(label.to_string(), regular(12.0), t.text);
    let button = Rect::from_center_size(pos2(r.center().x, r.center().y - 9.0), vec2(galley.size().x + 28.0, 28.0));
    // A list opened as the block was made goes under its button.
    if let Some(p) = st.embed_pick.as_mut().filter(|p| p.doc == doc && p.block == block && !p.under.is_finite()) {
        p.under = button;
    }
    let resp = ui.interact(button, Id::unique(("gbx_doc_embed_choose", doc.0, block.0)), Sense::click());
    a11y::button(&resp, label);
    ui.painter().rect_filled(button, CornerRadius::same(6), if resp.hovered() { t.hover } else { t.panel });
    ui.painter().rect_stroke(button, CornerRadius::same(6), Stroke::new(1.0, t.border_strong), StrokeKind::Inside);
    ui.painter().galley(button.center() - galley.size() / 2.0, galley, t.text);
    ui.painter().text(pos2(r.center().x, button.max.y + 12.0), Align2::CENTER_CENTER, note, regular(11.0), t.text_tertiary);
    if resp.clicked() {
        open(st, doc, block, button);
    }
}

/// Open the list of layers for embed block `block`, under `under`.
pub(super) fn open(st: &mut State, doc: DocId, block: BlockId, under: Rect) {
    st.embed_pick = Some(Pick { doc, block, under, query: String::new(), focus: true });
}

/// The layers an embed offers: on every page, the top layers, and the frames and variants inside
/// sections and component sets: (layer, its name, its page's name).
pub(super) fn layers(file: &Document) -> Vec<(NodeId, String, String)> {
    let shows =
        |k: &NodeKind| k.is_frame_like() || matches!(k, NodeKind::Component { .. } | NodeKind::ComponentSet { .. } | NodeKind::Instance { .. });
    let mut out = Vec::new();
    for page in &file.pages {
        let Some(p) = file.node(*page) else { continue };
        for c in &p.children {
            let Some(n) = file.node(*c) else { continue };
            if shows(&n.kind) {
                out.push((n.id, n.name.clone(), p.name.clone()));
            }
            if matches!(n.kind, NodeKind::Section { .. } | NodeKind::ComponentSet { .. }) {
                for g in &n.children {
                    if let Some(m) = file.node(*g).filter(|m| shows(&m.kind)) {
                        out.push((m.id, m.name.clone(), p.name.clone()));
                    }
                }
            }
            if out.len() >= MAX_LISTED {
                return out;
            }
        }
    }
    out
}

/// The list of layers, while open: a search field and the layers whose name holds what is typed.
/// Picking one shows it in the block; Escape or a click elsewhere closes the list.
pub(super) fn picker(app: &mut FigApp, ctx: &egui::Context, st: &mut State, doc: DocId) {
    let Some(mut pick) = st.embed_pick.clone().filter(|p| p.doc == doc) else {
        st.embed_pick = None;
        return;
    };
    // Not placed yet (the block is drawn next frame).
    if !pick.under.is_finite() {
        return;
    }
    let t = Tokens::get(ctx);
    let all = app.session.doc().map(layers).unwrap_or_default();
    let q = pick.query.to_lowercase();
    let found: Vec<_> = all.into_iter().filter(|(_, n, p)| q.is_empty() || n.to_lowercase().contains(&q) || p.to_lowercase().contains(&q)).collect();
    let mut chosen = None;
    let mut close = ctx.input(|i| i.key_pressed(Key::Escape));
    let area = egui::Area::new(Id::unique(("gbx_doc_embed_pick", doc.0, pick.block.0)))
        .order(egui::Order::Foreground)
        .fixed_pos(pos2(pick.under.center().x - 160.0, pick.under.max.y + 6.0))
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).fill(t.panel).show(ui, |ui| {
                ui.set_width(320.0);
                let field = ui.add(egui::TextEdit::singleline(&mut pick.query).hint_text(tr("Search layers")).desired_width(f32::INFINITY));
                if pick.focus {
                    field.request_focus();
                    pick.focus = false;
                }
                if field.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                    chosen = found.first().map(|f| f.0);
                }
                ui.add_space(4.0);
                if found.is_empty() {
                    ui.label(egui::RichText::new(tr("No frames or components")).font(regular(12.0)).color(t.text_tertiary));
                }
                egui::ScrollArea::vertical().max_height(280.0).show(ui, |ui| {
                    for (id, name, page) in &found {
                        let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::click());
                        a11y::list_item(&resp, name, false);
                        if resp.hovered() {
                            ui.painter().rect_filled(r, CornerRadius::same(4), t.hover);
                        }
                        let n = ui.painter().text(pos2(r.min.x + 8.0, r.center().y), Align2::LEFT_CENTER, name, regular(12.0), t.text);
                        ui.painter().text(pos2(n.max.x + 8.0, r.center().y), Align2::LEFT_CENTER, page, regular(11.0), t.text_tertiary);
                        if resp.clicked() {
                            chosen = Some(*id);
                        }
                    }
                });
            });
        });
    // A press outside the list closes it (not the press that opened it: the list was not there yet).
    if ctx.input(|i| i.pointer.any_pressed())
        && !area.response.contains_pointer()
        && !pick.under.contains(ctx.input(|i| i.pointer.interact_pos()).unwrap_or_default())
    {
        close = true;
    }
    if let Some(node) = chosen {
        app.run("doc.block.embed", json!({"doc": doc.0, "id": pick.block.0, "node": node.0}));
        close = true;
    }
    st.embed_pick = if close { None } else { Some(pick) };
}
