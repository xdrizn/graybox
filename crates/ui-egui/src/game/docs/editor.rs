//! The document editor: an open document shown in place of the canvas, with its title, its version
//! bar and its blocks (drawn and edited in `blocks.rs`). Typing is kept here and sent as one command
//! after a pause, so a burst of typing is one undo step.

use egui::{Align, Align2, CornerRadius, Id, Key, Layout, Modifiers, Rect, Sense, Ui, UiBuilder, pos2, vec2};
use figcraft_doc::docs::{Block, BlockKind, DocId};
use serde_json::json;

use super::blocks::{self, Look, View};
use super::state::{self, Pending, State, Target};
use super::versions::{self, Ver};
use super::{compare, sign};
use crate::a11y;
use crate::i18n::tr;
use crate::theme::{Tokens, regular, semibold};
use crate::{FigApp, LeftTab};

/// How wide the text column is at most.
const COLUMN: f32 = 760.0;
/// Seconds of quiet after typing before the text is sent as a command.
const IDLE: f64 = 0.6;

/// Draw the open document where the canvas goes. False when no document is open (the canvas shows).
pub(super) fn show(app: &mut FigApp, ui: &mut Ui) -> bool {
    if app.ui.left_tab != LeftTab::Documents {
        return false;
    }
    let Some(uid) = state::uid(app) else { return false };
    let mut st = state::take(uid);
    let shown = match st.open.filter(|d| app.session.doc().is_ok_and(|doc| doc.docs.get(*d).is_some())) {
        Some(doc) => {
            page(app, ui, &mut st, doc, uid);
            true
        }
        None => {
            st.open = None;
            false
        }
    };
    state::put(uid, st);
    shown
}

fn page(app: &mut FigApp, ui: &mut Ui, st: &mut State, doc: DocId, uid: u64) {
    let t = Tokens::get(ui.ctx());
    let area = ui.max_rect();
    // The editor is drawn in the canvas' place: keyboard navigation counts it as the canvas, so Tab
    // in a block nests the block instead of moving the keyboard to the panels.
    a11y::area(&mut app.ui.nav, a11y::Region::Canvas, area);
    ui.painter().rect_filled(area, CornerRadius::ZERO, t.canvas);
    let (title, icon, ver, mut blocks) = {
        let Some(d) = app.session.doc().ok().and_then(|d| d.docs.get(doc)) else { return };
        // The version on screen: the one picked in the version menu, else the last.
        let picked = st.viewing.filter(|(o, _)| *o == doc).and_then(|(_, n)| d.version(n));
        let Some(v) = picked.or_else(|| d.current()) else { return };
        let ver = Ver::of(ui.ctx(), uid, d, v);
        let mut blocks: Vec<View> = Vec::new();
        // A version that cannot change shows the content of every toggle.
        blocks::flatten(&v.blocks, 0, None, !ver.editable(), &mut blocks);
        (d.title.clone(), d.icon.clone(), ver, blocks)
    };
    // What was typed and not yet sent shows in place of what the file has.
    if let Some(p) = st.pending.as_ref().filter(|p| p.doc == doc)
        && let Target::Block(b) = p.target
        && let Some(v) = blocks.iter_mut().find(|v| v.id == b)
    {
        v.text.clone_from(&p.text);
    }
    let title = match st.pending.as_ref() {
        Some(p) if p.doc == doc && p.target == Target::Title => p.text.clone(),
        _ => title,
    };
    let editable = ver.editable();
    let now = ui.input(|i| i.time);

    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        let outer = ui.available_rect_before_wrap();
        let col_w = (outer.width() - 96.0).clamp(240.0, COLUMN);
        let col = Rect::from_min_size(pos2(outer.center().x - col_w / 2.0, outer.min.y + 84.0), vec2(col_w, 100_000.0));
        let mut c = ui.new_child(UiBuilder::new().max_rect(col).layout(Layout::top_down(Align::Min)));
        let at_end = title_ui(app, &mut c, st, &t, doc, &title, &icon, editable, now);
        c.add_space(6.0);
        versions::bar(app, &mut c, st, &t, doc, &ver);
        versions::notes(&mut c, st, &t, &ver);
        c.add_space(18.0);
        if compare::show(&mut c, st, &t, doc) {
            ui.allocate_rect(Rect::from_min_max(col.min, pos2(col.max.x, c.min_rect().max.y + 120.0)), Sense::hover());
            return;
        }
        #[cfg(test)]
        super::inline::forget();
        let rows: Vec<Rect> = (0..blocks.len()).map(|i| blocks::block_ui(app, &mut c, st, &t, doc, &blocks, i, col, &ver, now)).collect();
        super::drag::drop_ui(app, &c, st, &t, doc, &blocks, &rows, col);
        super::select::after(app, &mut c, st, &t, doc, &blocks, &rows, col, editable);
        if editable {
            super::media::take_drops(app, ui.ctx(), doc, &blocks, &rows, area);
        }
        versions::signatures(app, &mut c, st, &t, doc, &ver);
        tail_ui(app, &mut c, st, &t, doc, &blocks, editable);
        if at_end {
            // Enter in the title: the first block takes the keyboard, made first if there is none.
            match blocks.first() {
                Some(first) => st.focus = Some(Target::Block(first.id)),
                None => {
                    blocks::add_block(app, st, doc, None, "paragraph");
                }
            }
        }
        ui.allocate_rect(Rect::from_min_max(col.min, pos2(col.max.x, c.min_rect().max.y + 120.0)), Sense::hover());
    });
    back_button(ui, st, &t, area);
    export_button(app, ui, st, &t, area, doc);
    super::embed::picker(app, ui.ctx(), st, doc);
    settle(app, ui.ctx(), st, doc, now);
    versions::confirm_close(app, ui.ctx(), st);
    sign::dialog(app, ui.ctx(), st);
}

/// The Export button at the top right: the document as a PDF or Markdown file, or as a card on the canvas.
fn export_button(app: &mut FigApp, ui: &mut Ui, st: &mut State, t: &Tokens, area: Rect, doc: DocId) {
    use crate::right::menu_item;
    let label = tr("Export");
    let galley = ui.painter().layout_no_wrap(label.to_string(), regular(12.0), t.text);
    let w = galley.size().x + 28.0;
    let r = Rect::from_min_size(pos2(area.max.x - 32.0 - w, area.min.y + 16.0), vec2(w, 28.0));
    let id = ui.scope_id().with(("gbx_docs_export", doc.0));
    let resp = ui.interact(r, id, Sense::click());
    a11y::button(&resp, label);
    let open = egui::Popup::is_id_open(ui.ctx(), id.with("menu"));
    ui.painter().rect_filled(r, CornerRadius::same(6), if resp.hovered() || open { t.hover } else { t.panel });
    ui.painter().text(r.center(), Align2::CENTER_CENTER, label, regular(12.0), t.text);
    crate::right::menu_popup(&resp).id(id.with("menu")).show(|ui| {
        ui.set_min_width(200.0);
        if menu_item(ui, tr("Put on the canvas"), "") {
            super::export::put_on_canvas(app, st, doc, None);
            ui.close();
        }
        if menu_item(ui, tr("Export as PDF…"), "") {
            super::export::export(app, doc, "pdf");
            ui.close();
        }
        if menu_item(ui, tr("Export as Markdown…"), "") {
            super::export::export(app, doc, "md");
            ui.close();
        }
    });
}

fn back_button(ui: &mut Ui, st: &mut State, t: &Tokens, area: Rect) {
    let label = tr("Back to canvas");
    let galley = ui.painter().layout_no_wrap(label.to_string(), regular(12.0), t.text);
    let r = Rect::from_min_size(pos2(area.min.x + 16.0, area.min.y + 16.0), vec2(galley.size().x + 36.0, 28.0));
    let resp = ui.interact(r, Id::unique("gbx_docs_back"), Sense::click());
    a11y::button(&resp, label);
    ui.painter().rect_filled(r, CornerRadius::same(6), if resp.hovered() { t.hover } else { t.panel });
    // A chevron pointing back, then the words.
    let c = pos2(r.min.x + 16.0, r.center().y);
    let ink = egui::Stroke::new(1.5, t.icon);
    ui.painter().line_segment([c + vec2(2.5, -4.5), c + vec2(-2.0, 0.0)], ink);
    ui.painter().line_segment([c + vec2(-2.0, 0.0), c + vec2(2.5, 4.5)], ink);
    ui.painter().text(pos2(r.min.x + 28.0, r.center().y), Align2::LEFT_CENTER, label, regular(12.0), t.text);
    if resp.clicked() {
        st.open = None;
    }
}

/// The title row and the version line. True when Enter was pressed in the title.
#[allow(clippy::too_many_arguments)]
fn title_ui(app: &mut FigApp, ui: &mut Ui, st: &mut State, t: &Tokens, doc: DocId, title: &str, icon: &str, editable: bool, now: f64) -> bool {
    let mut text = title.to_string();
    let id = Id::unique(("gbx_doc_title", doc.0));
    let mut enter = false;
    ui.horizontal_top(|ui| {
        if !icon.is_empty() {
            let (r, _) = ui.allocate_exact_size(vec2(40.0, 40.0), Sense::hover());
            ui.painter().text(r.center(), Align2::CENTER_CENTER, icon, regular(28.0), t.text);
        }
        let focused = ui.memory(|m| m.has_focus(id));
        if focused && ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter)) {
            enter = true;
        }
        let resp = egui::TextEdit::singleline(&mut text)
            .id(id)
            .frame(egui::Frame::NONE)
            .font(semibold(32.0))
            .text_color(t.text)
            .hint_text(egui::RichText::new(tr("Untitled")).font(semibold(32.0)).color(t.text_tertiary))
            .desired_width(ui.available_width())
            .interactive(editable)
            .show(ui);
        let resp = resp.response;
        a11y::text(&resp, tr("Title"), &text);
        if st.focus == Some(Target::Title) {
            resp.request_focus();
            st.focus = None;
        }
        if resp.changed() {
            queue(app, st, doc, Target::Title, text.clone(), now);
        }
    });
    enter
}

/// Remember text typed in a field. Text typed in another field first goes out as a command.
pub(super) fn queue(app: &mut FigApp, st: &mut State, doc: DocId, target: Target, text: String, now: f64) {
    if let Some(p) = st.pending.take_if(|p| p.doc != doc || p.target != target) {
        commit(app, &p);
    }
    st.pending = Some(Pending { doc, target, text, at: now });
}

/// Send typed text as a command when it differs from what the file has.
pub(super) fn commit(app: &mut FigApp, p: &Pending) {
    let Ok(d) = app.session.doc() else { return };
    let Some(doc) = d.docs.get(p.doc) else { return };
    match p.target {
        Target::Title => {
            if doc.title != p.text {
                app.run("doc.rename", json!({"id": p.doc.0, "title": p.text}));
            }
        }
        Target::Block(b) => {
            let current = doc.current().and_then(|v| v.block(b)).map(Block::plain_text);
            if current.is_some_and(|c| c != p.text) {
                app.run("doc.block.set", json!({"doc": p.doc.0, "id": b.0, "text": p.text}));
            }
        }
        Target::Cell(b, row, col) => {
            let current = doc.current().and_then(|v| v.block(b)).and_then(|blk| match &blk.kind {
                BlockKind::Table { table } => Some(table.cell_text(row, col)),
                _ => None,
            });
            if current.is_some_and(|c| c != p.text) {
                app.run("doc.table.cell", json!({"doc": p.doc.0, "id": b.0, "row": row, "col": col, "text": p.text}));
            }
        }
    }
}

/// Send the pending text when its field lost the keyboard or the person paused.
fn settle(app: &mut FigApp, ctx: &egui::Context, st: &mut State, doc: DocId, now: f64) {
    let Some(p) = st.pending.as_ref() else { return };
    let field = match p.target {
        Target::Title => Id::unique(("gbx_doc_title", p.doc.0)),
        Target::Block(b) => Id::unique(("gbx_doc_block", p.doc.0, b.0)),
        Target::Cell(b, row, col) => super::table::cell_id(p.doc, b, row, col),
    };
    let focused = ctx.memory(|m| m.has_focus(field));
    let idle = now - p.at;
    if p.doc != doc || !focused || idle >= IDLE {
        if let Some(p) = st.pending.take() {
            commit(app, &p);
        }
    } else {
        ctx.request_repaint_after(std::time::Duration::from_secs_f64(IDLE - idle));
    }
}

/// Under the last block: a click there starts a new one (or goes to the empty last one).
fn tail_ui(app: &mut FigApp, ui: &mut Ui, st: &mut State, t: &Tokens, doc: DocId, blocks: &[View], editable: bool) {
    if !editable {
        return;
    }
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 160.0), Sense::click());
    if blocks.is_empty() {
        ui.painter().text(pos2(r.min.x, r.min.y + 12.0), Align2::LEFT_CENTER, tr("Click here and start writing"), regular(13.0), t.text_tertiary);
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
    }
    if resp.clicked() {
        match blocks.last() {
            Some(last) if matches!(last.look, Look::Paragraph) && last.text.is_empty() => st.focus = Some(Target::Block(last.id)),
            _ => {
                blocks::add_block(app, st, doc, None, "paragraph");
            }
        }
    }
}
