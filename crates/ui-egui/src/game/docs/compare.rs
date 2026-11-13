//! Two views of documents besides writing (G13.4): a version compared with the one before it (what
//! was added, removed and changed, block by block) and the decision log (every decision block of
//! the file, with who, when and why). Both read a command's answer once, when they open.

use egui::{Color32, CornerRadius, Rect, Sense, Ui, vec2};
use figcraft_doc::docs::DocId;
use serde_json::{Value, json};

use super::state::{Mode, State};
use crate::i18n::{tr, trf, trn};
use crate::theme::{Tokens, regular, semibold};
use crate::{FigApp, a11y};

/// Compare version `to` of `doc` with the one before it.
pub(super) fn open_diff(app: &mut FigApp, st: &mut State, doc: DocId, to: u32) {
    if let Some(r) = app.run("doc.diff", json!({"doc": doc.0, "to": to})) {
        st.report = Some((doc, r));
        st.mode = Mode::Compare;
    }
}

pub(super) fn open_decisions(app: &mut FigApp, st: &mut State, doc: DocId) {
    if let Some(r) = app.run("doc.decisions", json!({})) {
        st.report = Some((doc, r));
        st.mode = Mode::Decisions;
    }
}

/// The view in place of the blocks, when one is open; false when the blocks show.
pub(super) fn show(ui: &mut Ui, st: &mut State, t: &Tokens, doc: DocId) -> bool {
    let Some((_, report)) = st.report.clone().filter(|(d, _)| *d == doc && st.mode != Mode::Write) else {
        st.mode = Mode::Write;
        st.report = None;
        return false;
    };
    let back = |ui: &mut Ui, st: &mut State| {
        if crate::widgets::button(ui, tr("Back to the document"), false, true).clicked() {
            st.mode = Mode::Write;
            st.report = None;
        }
    };
    match st.mode {
        Mode::Compare => diff(ui, t, &report, |ui| back(ui, st)),
        Mode::Decisions => {
            if let Some(open) = decisions(ui, t, &report, |ui| back(ui, st)) {
                st.open = Some(open);
                st.viewing = None;
                st.mode = Mode::Write;
                st.report = None;
            }
        }
        Mode::Write => {}
    }
    true
}

fn heading(ui: &mut Ui, t: &Tokens, title: &str, sub: &str, back: impl FnOnce(&mut Ui)) {
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.set_width((ui.available_width() - 180.0).max(120.0));
            ui.label(egui::RichText::new(title).font(semibold(18.0)).color(t.text));
            ui.label(egui::RichText::new(sub).font(regular(11.5)).color(t.text_secondary));
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), back);
    });
    ui.add_space(16.0);
}

/// A row of the diff or the log: a coloured bar on the left, then its lines.
fn row(ui: &mut Ui, bar: Color32, fill: Color32, add: impl FnOnce(&mut Ui)) {
    let inner = egui::Frame::NONE
        .fill(fill)
        .corner_radius(CornerRadius::same(6))
        .inner_margin(egui::Margin { left: 16, right: 12, top: 8, bottom: 8 })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui);
        });
    let r = inner.response.rect;
    if bar != Color32::TRANSPARENT {
        ui.painter().rect_filled(Rect::from_min_size(r.min, vec2(3.0, r.height())), CornerRadius { nw: 6, sw: 6, ne: 0, se: 0 }, bar);
    }
    ui.add_space(4.0);
}

fn diff(ui: &mut Ui, t: &Tokens, r: &Value, back: impl FnOnce(&mut Ui)) {
    let num = |k: &str| r[k].as_u64().unwrap_or(0);
    let title = trf("Version {to} against version {from}", &[("to", &num("to")), ("from", &num("from"))]);
    let sub = trf(
        "{added} added · {removed} removed · {changed} changed",
        &[("added", &num("added")), ("removed", &num("removed")), ("changed", &num("changed"))],
    );
    heading(ui, t, &title, &sub, back);
    let blocks = r["blocks"].as_array().cloned().unwrap_or_default();
    if blocks.is_empty() {
        ui.label(egui::RichText::new(tr("Both versions are empty.")).font(regular(12.0)).color(t.text_tertiary));
    }
    let tint = |c: Color32| c.gamma_multiply(if t.dark { 0.18 } else { 0.10 });
    for b in &blocks {
        let text = b["text"].as_str().unwrap_or_default();
        let shown = if text.trim().is_empty() { format!("({})", b["kind"].as_str().unwrap_or_default()) } else { text.to_string() };
        match b["change"].as_str().unwrap_or_default() {
            "added" => row(ui, t.success, tint(t.success), |ui| {
                tag(ui, tr("Added"), t.success);
                ui.label(egui::RichText::new(&shown).font(regular(13.0)).color(t.text));
            }),
            "removed" => row(ui, t.danger, tint(t.danger), |ui| {
                tag(ui, tr("Removed"), t.danger);
                ui.label(egui::RichText::new(&shown).font(regular(13.0)).color(t.text_secondary).strikethrough());
            }),
            "changed" => row(ui, t.warning, tint(t.warning), |ui| {
                tag(ui, tr("Changed"), t.warning);
                let before = b["before"].as_str().unwrap_or_default();
                if before != text {
                    ui.label(egui::RichText::new(before).font(regular(12.0)).color(t.text_tertiary).strikethrough());
                }
                ui.label(egui::RichText::new(&shown).font(regular(13.0)).color(t.text));
            }),
            _ => row(ui, Color32::TRANSPARENT, Color32::TRANSPARENT, |ui| {
                ui.label(egui::RichText::new(&shown).font(regular(13.0)).color(t.text_secondary));
            }),
        }
    }
}

/// A small word in colour above a row.
fn tag(ui: &mut Ui, word: &str, color: Color32) {
    ui.label(egui::RichText::new(word.to_uppercase()).font(semibold(9.5)).color(color));
}

/// The decision log; returns the document to open when a row's "Open" is clicked.
fn decisions(ui: &mut Ui, t: &Tokens, r: &Value, back: impl FnOnce(&mut Ui)) -> Option<DocId> {
    let list = r.as_array().cloned().unwrap_or_default();
    let sub = if list.is_empty() {
        tr("No decisions yet. Turn a block into a Decision to record one.").to_string()
    } else {
        trn("{n} decisions in this file", list.len(), &[])
    };
    heading(ui, t, tr("Decision log"), &sub, back);
    let mut open = None;
    for d in &list {
        let s = |k: &str| d[k].as_str().unwrap_or_default().trim().to_string();
        row(ui, t.accent, t.field, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(if s("text").is_empty() { tr("(no words)").to_string() } else { s("text") })
                        .font(semibold(13.0))
                        .color(t.text),
                );
            });
            let mut meta = Vec::new();
            if !s("who").is_empty() {
                meta.push(s("who"));
            }
            if !s("when").is_empty() {
                meta.push(s("when"));
            }
            if !meta.is_empty() {
                ui.label(egui::RichText::new(meta.join(" · ")).font(regular(11.0)).color(t.text_secondary));
            }
            if !s("why").is_empty() {
                ui.label(egui::RichText::new(trf("Why: {why}", &[("why", &s("why"))])).font(regular(11.5)).color(t.text_secondary));
            }
            let title = if s("title").is_empty() { tr("Untitled").to_string() } else { s("title") };
            let link = trf("In “{title}” · version {n}", &[("title", &title), ("n", &d["version"].as_u64().unwrap_or(1))]);
            let resp = ui.add(egui::Label::new(egui::RichText::new(link).font(regular(11.0)).color(t.accent_text)).sense(Sense::click()));
            a11y::link(&resp, &title);
            if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if resp.clicked() {
                open = d["doc"].as_u64().map(DocId);
            }
        });
    }
    open
}
