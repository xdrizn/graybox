//! Signing a closed version (G13.4): the dialog where the person draws their signature with the
//! pointer or types their name in one of three handwriting styles, and how a signature looks in the
//! editor. The signature records the name, the time and the version's fingerprint
//! (`doc.version.sign`).

use egui::{Align2, Color32, CornerRadius, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use figcraft_doc::TextStyle;
use figcraft_doc::docs_version::{HAND_STYLES, MAX_POINTS, MAX_STROKES, Mark};
use figcraft_engine::doc_versions::hand_font;
use serde_json::json;

use super::state::{SignDraft, State};
use crate::a11y;
use crate::i18n::{tr, trf};
use crate::theme::{Tokens, regular, semibold};
use crate::{FigApp, widgets};

/// The drawing pad: three times as wide as it is tall, as the page draws the mark.
const PAD_W: f32 = 420.0;
const PAD_H: f32 = PAD_W / 3.0;

/// `text` in handwriting style `style`, as a texture (None while it is being drawn or without fonts).
fn hand(ctx: &egui::Context, text: &str, style: u8, size: f64) -> Option<egui::TextureHandle> {
    let (family, real) = hand_font(style);
    let st = TextStyle { font_family: family, font_style: if real { "Regular" } else { "Italic" }.into(), font_size: size, ..TextStyle::default() };
    let mut budget = 3;
    crate::widgets::font_preview::texture(ctx, &st, text, &mut budget)
}

/// A mark in `r`: drawn strokes fill a 3:1 box standing on the bottom edge; a typed name is written
/// in its handwriting style, left-aligned on the bottom edge.
pub(super) fn paint_mark(ui: &Ui, r: Rect, mark: &Mark, ink: Color32) {
    match mark {
        Mark::Drawn { strokes } => {
            let bw = r.width().min(r.height() * 3.0);
            let b = Rect::from_min_size(pos2(r.min.x, r.max.y - bw / 3.0), vec2(bw, bw / 3.0));
            strokes_in(ui.painter(), b, strokes, ink, 1.8);
        }
        Mark::Typed { text, style } => match hand(ui.ctx(), text, *style, 40.0) {
            Some(tex) => {
                let h = tex.size_vec2().y / ui.ctx().pixels_per_point().max(0.5);
                widgets::font_preview::paint(ui.painter(), &tex, pos2(r.min.x, r.max.y - h.min(r.height()) / 2.0), r.width(), r.height(), ink);
            }
            None => {
                let font = egui::FontId::new(26.0, egui::FontFamily::Proportional);
                ui.painter().text(pos2(r.min.x, r.max.y), Align2::LEFT_BOTTOM, text, font, ink);
            }
        },
    }
}

/// Strokes from 0 to 1 drawn in `b`.
fn strokes_in(painter: &egui::Painter, b: Rect, strokes: &[Vec<[f32; 2]>], ink: Color32, width: f32) {
    let at = |q: &[f32; 2]| pos2(b.min.x + q[0] * b.width(), b.min.y + q[1] * b.height());
    for s in strokes {
        match s.as_slice() {
            [] => {}
            [one] => {
                painter.circle_filled(at(one), width / 2.0, ink);
            }
            many => {
                painter.add(egui::Shape::line(many.iter().map(at).collect(), Stroke::new(width, ink)));
            }
        }
    }
}

fn mark_of(d: &SignDraft) -> Mark {
    if d.draw { Mark::Drawn { strokes: d.strokes.clone() } } else { Mark::Typed { text: d.name.trim().to_string(), style: d.style } }
}

/// The Sign dialog, while one is open.
pub(super) fn dialog(app: &mut FigApp, ctx: &egui::Context, st: &mut State) {
    let Some(mut d) = st.sign.take() else { return };
    let Some(code) = app
        .session
        .doc()
        .ok()
        .and_then(|f| f.docs.get(d.doc))
        .and_then(|doc| doc.version(d.version))
        .filter(|v| v.is_closed())
        .map(|v| figcraft_engine::doc_versions::code_of(v).unwrap_or_default())
    else {
        return;
    };
    let mut open = true;
    let mut decided = None;
    crate::dialogs::window(tr("Sign the version"), ctx).open(&mut open).show(ctx, |ui| {
        let t = Tokens::get(ui.ctx());
        ui.set_width(PAD_W);
        let who = if d.role.trim().is_empty() {
            trf("Sign version {n}", &[("n", &d.version)])
        } else {
            trf("Sign version {n} as {role}", &[("n", &d.version), ("role", &d.role)])
        };
        ui.label(egui::RichText::new(who).font(semibold(13.0)).color(t.text));
        ui.add_space(10.0);
        widgets::label(ui, tr("Your name"), t.text_secondary);
        let field = egui::Id::unique(("gbx_doc_sign_name", d.doc.0, d.version, d.slot));
        // A live field: the handwriting previews follow every letter.
        let r = widgets::text_input(ui, field, &mut d.name, PAD_W, tr("Your full name"));
        a11y::text(&r, tr("Your name"), &d.name);
        if r.changed() {
            d.name = d.name.chars().filter(|c| !c.is_control()).take(figcraft_doc::docs_version::MAX_SIGNER_TEXT).collect();
        }
        ui.add_space(10.0);
        if let Some(i) = widgets::segmented_text(ui, &[tr("Type"), tr("Draw")], usize::from(d.draw)) {
            d.draw = i == 1;
        }
        ui.add_space(8.0);
        if d.draw {
            pad(ui, &t, &mut d);
        } else {
            styles(ui, &t, &mut d);
        }
        ui.add_space(10.0);
        ui.label(
            egui::RichText::new(trf("Signing records your name, the time (UTC) and the version's code {code}.", &[("code", &code)]))
                .font(regular(10.5))
                .color(t.text_tertiary),
        );
        ui.add_space(12.0);
        let ready = !d.name.trim().is_empty() && !mark_of(&d).is_empty();
        ui.horizontal(|ui| {
            if widgets::button(ui, tr("Cancel"), false, true).clicked() {
                decided = Some(false);
            }
            if widgets::button(ui, tr("Sign"), true, ready).clicked() {
                decided = Some(true);
            }
        });
    });
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        decided = Some(false);
    }
    if decided == Some(true) {
        let mark = serde_json::to_value(mark_of(&d)).unwrap_or_default();
        let params = json!({"doc": d.doc.0, "version": d.version, "slot": d.slot, "name": d.name.trim(), "role": d.role, "mark": mark});
        if app.run("doc.version.sign", params).is_some() {
            app.toast(tr("Signed"));
        }
        return;
    }
    if decided.is_none() && open {
        st.sign = Some(d);
    }
}

/// The drawing pad: drag to draw; Clear starts over.
fn pad(ui: &mut Ui, t: &Tokens, d: &mut SignDraft) {
    let (r, resp) = ui.allocate_exact_size(vec2(PAD_W, PAD_H), Sense::drag());
    a11y::container_bounds(ui, r);
    ui.painter().rect_filled(r, CornerRadius::same(8), Color32::WHITE);
    ui.painter().rect_stroke(r, CornerRadius::same(8), Stroke::new(1.0, t.border_strong), StrokeKind::Inside);
    let base = r.max.y - PAD_H * 0.22;
    ui.painter().line_segment([pos2(r.min.x + 18.0, base), pos2(r.max.x - 18.0, base)], Stroke::new(1.0, Color32::from_gray(200)));
    if d.strokes.is_empty() {
        ui.painter().text(r.center(), Align2::CENTER_CENTER, tr("Sign here with the mouse or a pen"), regular(12.0), Color32::from_gray(150));
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
    }
    let to_pad = |p: egui::Pos2| [((p.x - r.min.x) / r.width()).clamp(0.0, 1.0), ((p.y - r.min.y) / r.height()).clamp(0.0, 1.0)];
    let points: usize = d.strokes.iter().map(Vec::len).sum();
    if resp.drag_started()
        && let Some(p) = resp.interact_pointer_pos()
        && d.strokes.len() < MAX_STROKES
        && points < MAX_POINTS
    {
        d.strokes.push(vec![to_pad(p)]);
    } else if resp.dragged()
        && let Some(p) = resp.interact_pointer_pos()
        && points < MAX_POINTS
        && let Some(s) = d.strokes.last_mut()
    {
        let q = to_pad(p);
        // Points closer than half a point add nothing but weight.
        if s.last().is_none_or(|l| ((l[0] - q[0]) * PAD_W).hypot((l[1] - q[1]) * PAD_H) > 0.5) {
            s.push(q);
        }
    }
    strokes_in(ui.painter(), r, &d.strokes, Color32::from_rgb(0x1B, 0x22, 0x33), 2.2);
    ui.horizontal(|ui| {
        if widgets::button(ui, tr("Clear"), false, !d.strokes.is_empty()).clicked() {
            d.strokes.clear();
        }
    });
}

/// The three handwriting styles, the name written in each; a click picks one.
fn styles(ui: &mut Ui, t: &Tokens, d: &mut SignDraft) {
    let name = if d.name.trim().is_empty() { tr("Your name").to_string() } else { d.name.trim().to_string() };
    for style in 0..HAND_STYLES {
        let (r, resp) = ui.allocate_exact_size(vec2(PAD_W, 54.0), Sense::click());
        let on = d.style == style;
        let (family, real) = hand_font(style);
        let label = if real { family } else { tr("Italic").to_string() };
        a11y::toggle(&resp, &label, on);
        ui.painter().rect_filled(
            r,
            CornerRadius::same(8),
            if on {
                t.selected_row
            } else if resp.hovered() {
                t.hover
            } else {
                t.field
            },
        );
        if on {
            ui.painter().rect_stroke(r, CornerRadius::same(8), Stroke::new(1.5, t.accent), StrokeKind::Inside);
        }
        let ink = if d.name.trim().is_empty() { t.text_tertiary } else { t.text };
        let mark = Mark::Typed { text: name.clone(), style };
        paint_mark(ui, Rect::from_min_max(pos2(r.min.x + 14.0, r.min.y + 6.0), pos2(r.max.x - 150.0, r.max.y - 8.0)), &mark, ink);
        ui.painter().text(
            pos2(r.max.x - 12.0, r.center().y),
            Align2::RIGHT_CENTER,
            widgets::elide(ui, &label, 130.0),
            regular(10.0),
            t.text_tertiary,
        );
        if resp.clicked() {
            d.style = style;
        }
        ui.add_space(4.0);
    }
}
