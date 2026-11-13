//! The document editor: an open document shown in place of the canvas. G13.1 is the basic one: an
//! editable title and a list of text and heading blocks you can type into (Enter adds a block,
//! Backspace on an empty one removes it, the handle on the left turns a block into another kind or
//! deletes it). G13.2 grows this module into the full Notion-like editor.

use egui::{Align, Align2, Color32, CornerRadius, Id, Key, Layout, Modifiers, Rect, Sense, Ui, UiBuilder, pos2, vec2};
use figcraft_doc::docs::{Block, BlockId, BlockKind, DocId};
use serde_json::{Value, json};

use super::state::{self, Pending, State, Target};
use crate::a11y::{self, Tip};
use crate::i18n::{tr, trf};
use crate::theme::{Tokens, regular, semibold};
use crate::{FigApp, LeftTab, widgets};

/// How wide the text column is at most.
const COLUMN: f32 = 760.0;
/// Seconds of quiet after typing before the text is sent as a command.
const IDLE: f64 = 0.6;

/// How a block is drawn in this first editor.
enum Look {
    Paragraph,
    Heading(u8),
    Divider,
    /// A kind this editor cannot draw yet: its name, shown dim.
    Other(String),
}

/// A block, copied out of the file so drawing does not hold the file.
struct View {
    id: BlockId,
    depth: usize,
    look: Look,
    text: String,
    color: Option<Color32>,
    size: Option<f32>,
}

fn flatten(blocks: &[Block], depth: usize, out: &mut Vec<View>) {
    for b in blocks {
        let look = match &b.kind {
            BlockKind::Heading { level } => Look::Heading((*level).clamp(1, 4)),
            BlockKind::Divider => Look::Divider,
            k if k.has_text() => Look::Paragraph,
            k => Look::Other(k.name()),
        };
        out.push(View { id: b.id, depth, look, text: b.plain_text(), color: b.style.color.as_deref().and_then(parse_hex), size: b.style.size });
        flatten(&b.children, depth + 1, out);
    }
}

/// `#RGB`, `#RRGGBB` or `#RRGGBBAA` as a colour.
pub(super) fn parse_hex(s: &str) -> Option<Color32> {
    let h = s.strip_prefix('#')?;
    let digit = |i: usize| h.get(i..=i).and_then(|d| u8::from_str_radix(d, 16).ok());
    let byte = |i: usize| h.get(i..i + 2).and_then(|d| u8::from_str_radix(d, 16).ok());
    match h.len() {
        3 | 4 => {
            let (r, g, b) = (digit(0)? * 17, digit(1)? * 17, digit(2)? * 17);
            Some(Color32::from_rgba_unmultiplied(r, g, b, if h.len() == 4 { digit(3)? * 17 } else { 255 }))
        }
        6 | 8 => Some(Color32::from_rgba_unmultiplied(byte(0)?, byte(2)?, byte(4)?, if h.len() == 8 { byte(6)? } else { 255 })),
        _ => None,
    }
}

fn font_size(look: &Look, own: Option<f32>) -> f32 {
    own.unwrap_or(match look {
        Look::Heading(1) => 28.0,
        Look::Heading(2) => 22.0,
        Look::Heading(3) => 18.0,
        Look::Heading(_) => 15.0,
        _ => 14.0,
    })
}

/// Draw the open document where the canvas goes. False when no document is open (the canvas shows).
pub(super) fn show(app: &mut FigApp, ui: &mut Ui) -> bool {
    if app.ui.left_tab != LeftTab::Documents {
        return false;
    }
    let Some(uid) = state::uid(app) else { return false };
    let mut st = state::take(uid);
    let shown = match st.open.filter(|d| app.session.doc().is_ok_and(|doc| doc.docs.get(*d).is_some())) {
        Some(doc) => {
            page(app, ui, &mut st, doc);
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

fn page(app: &mut FigApp, ui: &mut Ui, st: &mut State, doc: DocId) {
    let t = Tokens::get(ui.ctx());
    let area = ui.max_rect();
    ui.painter().rect_filled(area, CornerRadius::ZERO, t.canvas);
    let (title, icon, number, closed, mut blocks) = {
        let Some(d) = app.session.doc().ok().and_then(|d| d.docs.get(doc)) else { return };
        let v = d.current();
        let mut blocks = Vec::new();
        if let Some(v) = v {
            flatten(&v.blocks, 0, &mut blocks);
        }
        (d.title.clone(), d.icon.clone(), v.map_or(1, |v| v.number), v.is_some_and(|v| v.is_closed()), blocks)
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
    let editable = !closed;
    let now = ui.input(|i| i.time);

    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        let outer = ui.available_rect_before_wrap();
        let col_w = (outer.width() - 96.0).clamp(240.0, COLUMN);
        let col = Rect::from_min_size(pos2(outer.center().x - col_w / 2.0, outer.min.y + 84.0), vec2(col_w, 100_000.0));
        let mut c = ui.new_child(UiBuilder::new().max_rect(col).layout(Layout::top_down(Align::Min)));
        let at_end = title_ui(app, &mut c, st, &t, doc, &title, &icon, number, closed, editable, now);
        c.add_space(18.0);
        let ids: Vec<BlockId> = blocks.iter().map(|b| b.id).collect();
        for (i, v) in blocks.iter().enumerate() {
            block_ui(app, &mut c, st, &t, doc, v, i, &ids, col, editable, now);
        }
        tail_ui(app, &mut c, st, &t, doc, &blocks, editable);
        if at_end {
            // Enter in the title: the first block takes the keyboard, made first if there is none.
            match blocks.first() {
                Some(first) => st.focus = Some(Target::Block(first.id)),
                None => add_block(app, st, doc, None, "paragraph"),
            }
        }
        ui.allocate_rect(Rect::from_min_max(col.min, pos2(col.max.x, c.min_rect().max.y + 120.0)), Sense::hover());
    });
    back_button(ui, st, &t, area);
    export_button(app, ui, st, &t, area, doc);
    settle(app, ui.ctx(), st, doc, now);
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
fn title_ui(
    app: &mut FigApp,
    ui: &mut Ui,
    st: &mut State,
    t: &Tokens,
    doc: DocId,
    title: &str,
    icon: &str,
    number: u32,
    closed: bool,
    editable: bool,
    now: f64,
) -> bool {
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
    let line = if closed {
        trf("Version {n} · closed, it can no longer change", &[("n", &number)])
    } else {
        trf("Version {n} · draft", &[("n", &number)])
    };
    ui.label(egui::RichText::new(line).font(regular(11.0)).color(t.text_tertiary));
    enter
}

/// Remember text typed in a field. Text typed in another field first goes out as a command.
fn queue(app: &mut FigApp, st: &mut State, doc: DocId, target: Target, text: String, now: f64) {
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
    }
}

/// Send the pending text when its field lost the keyboard or the person paused.
fn settle(app: &mut FigApp, ctx: &egui::Context, st: &mut State, doc: DocId, now: f64) {
    let Some(p) = st.pending.as_ref() else { return };
    let field = match p.target {
        Target::Title => Id::unique(("gbx_doc_title", p.doc.0)),
        Target::Block(b) => Id::unique(("gbx_doc_block", p.doc.0, b.0)),
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

#[allow(clippy::too_many_arguments)]
fn block_ui(
    app: &mut FigApp,
    ui: &mut Ui,
    st: &mut State,
    t: &Tokens,
    doc: DocId,
    v: &View,
    index: usize,
    ids: &[BlockId],
    col: Rect,
    editable: bool,
    now: f64,
) {
    // Nested blocks step in by 16 points a level (at most 8 levels, so it fits the text margin).
    let indent = v.depth as f32 * 16.0;
    ui.add_space(2.0);
    let top = ui.cursor().min.y;
    match &v.look {
        Look::Divider => {
            let (r, _) = ui.allocate_exact_size(vec2(col.width(), 20.0), Sense::hover());
            ui.painter().line_segment([pos2(r.min.x + indent, r.center().y), pos2(r.max.x, r.center().y)], egui::Stroke::new(1.0, t.border_strong));
        }
        Look::Other(name) => {
            let (r, _) = ui.allocate_exact_size(vec2(col.width() - indent, 30.0), Sense::hover());
            let r = Rect::from_min_max(pos2(r.min.x + indent, r.min.y), r.max);
            ui.painter().rect_filled(r, CornerRadius::same(5), t.field);
            ui.painter().text(pos2(r.min.x + 10.0, r.center().y), Align2::LEFT_CENTER, name, regular(11.0), t.text_tertiary);
        }
        look => {
            text_block(app, ui, st, t, doc, v, look, index, ids, col, indent, editable, now);
        }
    }
    let bottom = ui.cursor().min.y;
    handle(app, ui, t, doc, v, Rect::from_min_max(pos2(col.min.x - 36.0, top), pos2(col.max.x, bottom)), editable);
}

#[allow(clippy::too_many_arguments)]
fn text_block(
    app: &mut FigApp,
    ui: &mut Ui,
    st: &mut State,
    t: &Tokens,
    doc: DocId,
    v: &View,
    look: &Look,
    index: usize,
    ids: &[BlockId],
    col: Rect,
    indent: f32,
    editable: bool,
    now: f64,
) {
    let id = Id::unique(("gbx_doc_block", doc.0, v.id.0));
    let size = font_size(look, v.size);
    let font = if matches!(look, Look::Heading(_)) { semibold(size) } else { regular(size) };
    let mut text = v.text.clone();
    let focused = ui.memory(|m| m.has_focus(id));
    let mut enter = false;
    let mut backspace_on_empty = false;
    if focused && editable {
        enter = ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
        backspace_on_empty = v.text.is_empty() && ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Backspace));
    }
    let out = egui::TextEdit::multiline(&mut text)
        .id(id)
        .frame(egui::Frame::NONE)
        .font(font)
        .text_color(v.color.unwrap_or(t.text))
        .desired_width(col.width() - indent)
        .desired_rows(1)
        .interactive(editable)
        .margin(egui::Margin { left: indent as i8, right: 0, top: 3, bottom: 3 })
        .show(ui);
    let resp = out.response;
    a11y::text(&resp, tr("Text"), &text);
    if st.focus == Some(Target::Block(v.id)) {
        resp.request_focus();
        // The cursor goes to the end of the text.
        let mut state = out.state;
        let end = egui::text::CCursor::new(text.chars().count());
        state.cursor.set_char_range(Some(egui::text::CCursorRange::one(end)));
        state.store(ui.ctx(), id);
        st.focus = None;
    }
    if resp.changed() {
        queue(app, st, doc, Target::Block(v.id), text.clone(), now);
    }
    if enter {
        // The words typed so far go out first so the new block lands after them.
        if let Some(p) = st.pending.take() {
            commit(app, &p);
        }
        let current = app.session.doc().ok().and_then(|d| d.docs.get(doc)).and_then(|d| d.current());
        let kind = match current.and_then(|c| c.block(v.id)) {
            Some(b) if matches!(b.kind, BlockKind::Bulleted | BlockKind::Numbered | BlockKind::Checklist { .. }) => b.kind.name(),
            _ => "paragraph".to_string(),
        };
        // Right after this block, among the blocks it sits with (which may be inside another block).
        let after = current.and_then(|c| c.locate(v.id)).map(|(parent, at)| (parent, at + 1));
        add_block(app, st, doc, after, &kind);
    }
    if backspace_on_empty {
        // An empty block goes, and the one above takes the keyboard.
        app.run("doc.block.delete", json!({"doc": doc.0, "id": v.id.0}));
        if let Some(prev) = index.checked_sub(1).and_then(|p| ids.get(p)) {
            st.focus = Some(Target::Block(*prev));
        }
    }
}

/// Add a block of `kind` at `place` (inside which block, at which index; the end of the top when
/// `None`) and give it the keyboard.
fn add_block(app: &mut FigApp, st: &mut State, doc: DocId, place: Option<(Option<BlockId>, usize)>, kind: &str) {
    let mut params = json!({"doc": doc.0, "kind": kind});
    if let Some((parent, index)) = place {
        params["index"] = json!(index);
        if let Some(p) = parent {
            params["parent"] = json!(p.0);
        }
    }
    if let Some(id) = app.run("doc.block.insert", params).and_then(|r| r.get("id").and_then(Value::as_u64)) {
        st.focus = Some(Target::Block(BlockId(id)));
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
            _ => add_block(app, st, doc, None, "paragraph"),
        }
    }
}

/// The handle on the left of a block: it shows while the pointer is on the block; its menu turns the
/// block into another kind or deletes it.
fn handle(app: &mut FigApp, ui: &mut Ui, t: &Tokens, doc: DocId, v: &View, row: Rect, editable: bool) {
    if !editable {
        return;
    }
    let id = Id::unique(("gbx_doc_handle", doc.0, v.id.0));
    let menu_id = id.with("menu");
    let open = egui::Popup::is_id_open(ui.ctx(), menu_id);
    let hover = ui.rect_contains_pointer(row) || open;
    if !hover {
        return;
    }
    let r = Rect::from_min_size(pos2(row.min.x + 6.0, row.min.y + 3.0), vec2(22.0, 24.0));
    let resp = ui.interact(r, id, Sense::click()).tip(tr("Turn into, delete"));
    if resp.hovered() || open {
        ui.painter().rect_filled(r, CornerRadius::same(4), t.hover);
    }
    // Six dots, two by three.
    for dx in [-3.0, 3.0] {
        for dy in [-5.0, 0.0, 5.0] {
            ui.painter().circle_filled(r.center() + vec2(dx, dy), 1.3, t.icon_secondary);
        }
    }
    crate::right::menu_popup(&resp).id(menu_id).show(|ui| {
        use crate::right::menu_item;
        ui.set_min_width(190.0);
        widgets::label(ui, tr("Turn into"), Tokens::get(ui.ctx()).text_secondary);
        for (label, kind) in [(tr("Text"), "paragraph"), (tr("Heading 1"), "heading1"), (tr("Heading 2"), "heading2"), (tr("Heading 3"), "heading3")]
        {
            if menu_item(ui, label, "") {
                app.run("doc.block.turn_into", json!({"doc": doc.0, "id": v.id.0, "kind": kind}));
                ui.close();
            }
        }
        ui.add_space(4.0);
        if menu_item(ui, tr("Delete"), "") {
            app.run("doc.block.delete", json!({"doc": doc.0, "id": v.id.0}));
            ui.close();
        }
    });
}
