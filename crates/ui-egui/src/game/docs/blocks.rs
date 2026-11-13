//! The blocks of the document editor (G13.2): how each kind looks (a bullet, a number, a checkbox, a
//! toggle's arrow, a callout, a quote, a decision, a link), the keys that edit them the way Notion
//! does (Markdown marks at the start of a line, Enter, Backspace, Tab, the arrows) and the handle on
//! the left with its menu (turn into, duplicate, colour, delete).

use std::sync::Arc;

use egui::text::{LayoutJob, TextFormat};
use egui::{Align2, Color32, CornerRadius, Id, Key, Modifiers, Rect, Sense, Shape, Stroke, Ui, pos2, vec2};
use figcraft_doc::docs::{Block, BlockId, BlockKind, DocId, DocVersion};
use serde_json::{Value, json};

use super::editor::{commit, queue};
use super::slash;
use super::state::{State, Target};
use super::versions::{self, Ver};
use crate::a11y::{self, Tip};
use crate::i18n::{tr, trf};
use crate::theme::{Tokens, regular, semibold};
use crate::{FigApp, widgets};

/// How far a nested block steps in, per level.
pub(super) const STEP: f32 = 24.0;
/// The room on the left of a list item, a task or a toggle for its mark.
const MARK_W: f32 = 26.0;

/// How a block is drawn.
pub(super) enum Look {
    Paragraph,
    Heading(u8),
    Bulleted,
    /// Its number in the run of numbered blocks it belongs to.
    Numbered(usize),
    Checklist(bool),
    /// Open or not (always open on a version that cannot change).
    Toggle(bool),
    Callout(String),
    Quote,
    Decision {
        who: String,
        when: String,
        why: String,
    },
    Link(String),
    Divider,
    /// A signature slot (G13.7): the role it is for; the text is the name.
    Signature(String),
    /// A kind this editor cannot draw yet: its name, shown dim.
    Other(String),
}

impl Look {
    /// Whether the block is written in (it has a text field).
    pub(super) fn writable(&self) -> bool {
        !matches!(self, Self::Divider | Self::Signature(_) | Self::Other(_))
    }
}

/// A block, copied out of the file so drawing does not hold the file.
pub(super) struct View {
    pub id: BlockId,
    pub depth: usize,
    pub look: Look,
    pub text: String,
    pub color: Option<Color32>,
    pub background: Option<Color32>,
    pub size: Option<f32>,
    /// The block it sits inside, and the one just before it among its siblings.
    pub parent: Option<BlockId>,
    pub before: Option<BlockId>,
}

/// The blocks in reading order. The content of a closed toggle is left out, unless `all_open`.
pub(super) fn flatten(blocks: &[Block], depth: usize, parent: Option<BlockId>, all_open: bool, out: &mut Vec<View>) {
    let mut number = 0;
    let mut before = None;
    for b in blocks {
        number = if matches!(b.kind, BlockKind::Numbered) { number + 1 } else { 0 };
        let look = match &b.kind {
            BlockKind::Paragraph => Look::Paragraph,
            BlockKind::Heading { level } => Look::Heading((*level).clamp(1, 4)),
            BlockKind::Bulleted => Look::Bulleted,
            BlockKind::Numbered => Look::Numbered(number),
            BlockKind::Checklist { checked } => Look::Checklist(*checked),
            BlockKind::Toggle { open } => Look::Toggle(*open || all_open),
            BlockKind::Callout { icon } => Look::Callout(icon.clone()),
            BlockKind::Quote => Look::Quote,
            BlockKind::Decision { who, when, why } => Look::Decision { who: who.clone(), when: when.clone(), why: why.clone() },
            BlockKind::Link { url } => Look::Link(url.clone()),
            BlockKind::Divider => Look::Divider,
            BlockKind::Signature { role } => Look::Signature(role.clone()),
            k => Look::Other(k.name()),
        };
        let open = !matches!(look, Look::Toggle(false));
        out.push(View {
            id: b.id,
            depth,
            look,
            text: b.plain_text(),
            color: b.style.color.as_deref().and_then(parse_hex),
            background: b.style.background.as_deref().and_then(parse_hex),
            size: b.style.size,
            parent,
            before,
        });
        if open {
            flatten(&b.children, depth + 1, Some(b.id), all_open, out);
        }
        before = Some(b.id);
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
        _ => 15.0,
    })
}

/// The Markdown marks that turn a text block into another kind when typed at its start.
const MARKS: [(&str, &str); 15] = [
    ("# ", "heading1"),
    ("## ", "heading2"),
    ("### ", "heading3"),
    ("#### ", "heading4"),
    ("- ", "bulleted"),
    ("* ", "bulleted"),
    ("+ ", "bulleted"),
    ("1. ", "numbered"),
    ("1) ", "numbered"),
    ("[] ", "checklist"),
    ("[ ] ", "checklist"),
    ("[x] ", "done"),
    ("> ", "quote"),
    ("\" ", "quote"),
    ("---", "divider"),
];

/// The mark `text` starts with, if typing it just finished it (`before` did not start with it and the
/// cursor sits right after it).
fn mark_typed<'a>(before: &str, text: &str, caret: Option<usize>) -> Option<(&'a str, &'a str)> {
    MARKS.iter().copied().find(|(mark, _)| {
        let n = mark.chars().count();
        text.starts_with(mark) && !before.starts_with(mark) && caret == Some(n) && (*mark != "---" || text == "---")
    })
}

/// What a key did in a block, acted on after the block is drawn.
enum Act {
    Enter(usize),
    BackspaceAtStart,
    Indent,
    Outdent,
    Up,
    Down,
    /// Ctrl+Shift+arrows: the block swaps places with its neighbour (`true` upwards).
    Move(bool),
}

#[cfg(test)]
thread_local! {
    static LAST_HANDLE: std::cell::Cell<Option<Rect>> = const { std::cell::Cell::new(None) };
}

/// Where the last handle was drawn (tests).
#[cfg(test)]
pub(super) fn last_handle() -> Option<Rect> {
    LAST_HANDLE.with(std::cell::Cell::get)
}

/// One block, at `views[index]`, and the row it takes (nothing when there is no such block).
#[allow(clippy::too_many_arguments)]
pub(super) fn block_ui(
    app: &mut FigApp,
    ui: &mut Ui,
    st: &mut State,
    t: &Tokens,
    doc: DocId,
    views: &[View],
    index: usize,
    col: Rect,
    ver: &Ver,
    now: f64,
) -> Rect {
    let Some(v) = views.get(index) else { return Rect::NOTHING };
    let editable = ver.editable();
    let indent = v.depth as f32 * STEP;
    ui.add_space(2.0);
    let top = ui.cursor().min.y;
    // Room for a background drawn once the block's height is known.
    let bg = ui.painter().add(Shape::Noop);
    match &v.look {
        Look::Divider => {
            let (r, _) = ui.allocate_exact_size(vec2(col.width(), 20.0), Sense::hover());
            ui.painter().line_segment([pos2(r.min.x + indent, r.center().y), pos2(r.max.x, r.center().y)], Stroke::new(1.0, t.border_strong));
        }
        Look::Signature(role) => versions::signature_block(app, ui, t, doc, ver, v.id, &v.text, role, indent),
        Look::Other(name) => {
            let (r, _) = ui.allocate_exact_size(vec2(col.width() - indent, 30.0), Sense::hover());
            let r = Rect::from_min_max(pos2(r.min.x + indent, r.min.y), r.max);
            ui.painter().rect_filled(r, CornerRadius::same(5), t.field);
            ui.painter().text(pos2(r.min.x + 10.0, r.center().y), Align2::LEFT_CENTER, name, regular(11.0), t.text_tertiary);
        }
        _ => text_block(app, ui, st, t, doc, views, index, col, indent, editable, now),
    }
    let bottom = ui.cursor().min.y;
    let area = Rect::from_min_max(pos2(col.min.x + indent - 6.0, top - 1.0), pos2(col.max.x, bottom + 1.0));
    let fill = match (&v.look, v.background) {
        (_, Some(c)) => Some(c),
        (Look::Callout(_), None) => Some(t.field),
        _ => None,
    };
    if let Some(c) = fill {
        ui.painter().set(bg, Shape::rect_filled(area, CornerRadius::same(6), c));
    }
    let row = Rect::from_min_max(pos2(col.min.x - 36.0, top), pos2(col.max.x, bottom));
    handle(app, ui, st, t, doc, v, row, editable);
    row
}

/// A block written in: its mark on the left (bullet, number, box, arrow, icon, bar), its words, and
/// the keys that edit it.
#[allow(clippy::too_many_arguments)]
fn text_block(
    app: &mut FigApp,
    ui: &mut Ui,
    st: &mut State,
    t: &Tokens,
    doc: DocId,
    views: &[View],
    index: usize,
    col: Rect,
    indent: f32,
    editable: bool,
    now: f64,
) {
    let Some(v) = views.get(index) else { return };
    let id = Id::unique(("gbx_doc_block", doc.0, v.id.0));
    let size = font_size(&v.look, v.size);
    let font = if matches!(v.look, Look::Heading(_)) { semibold(size) } else { regular(size) };
    let (lead, pad) = match v.look {
        Look::Bulleted | Look::Numbered(_) | Look::Checklist(_) | Look::Toggle(_) => (MARK_W, 0.0),
        Look::Callout(_) => (40.0, 10.0),
        Look::Quote => (16.0, 0.0),
        Look::Decision { .. } | Look::Link(_) => (14.0, 8.0),
        _ => (0.0, 0.0),
    };
    let done = matches!(v.look, Look::Checklist(true));
    let ink = match v.look {
        _ if done => t.text_tertiary,
        Look::Link(_) => v.color.unwrap_or(t.accent),
        _ => v.color.unwrap_or(t.text),
    };
    if pad > 0.0 {
        ui.add_space(pad);
    }
    if matches!(v.look, Look::Decision { .. }) {
        ui.horizontal(|ui| {
            ui.add_space(indent + lead + pad);
            ui.label(egui::RichText::new(tr("Decision")).font(semibold(10.5)).color(t.accent));
        });
    }
    let row_h = ui.fonts_mut(|f| f.row_height(&font));

    // Keys, read before the field so it does not act on them too.
    let focused = ui.memory(|m| m.has_focus(id));
    let len = v.text.chars().count();
    let caret = egui::TextEdit::load_state(ui.ctx(), id)
        .and_then(|s| s.cursor.char_range())
        .map(|r| (r.primary.index.0.min(r.secondary.index.0), r.primary.index.0.max(r.secondary.index.0)));
    let mut act = None;
    // An open `/` menu takes the arrows, Enter and Escape first.
    let slash_at = st.slash.as_ref().filter(|s| s.block == v.id).map(|s| s.at);
    let slash_key = match slash_at {
        Some(at) if focused && editable => slash::keys(ui, slash::lists(&v.text, at, caret.map(|c| c.1))),
        _ => None,
    };
    if focused && editable {
        ui.input_mut(|i| {
            let (from, to) = caret.unwrap_or((len, len));
            // consume_key ignores an extra Shift: Shift+Enter breaks the line and Shift+arrows
            // select, both left to the field.
            let shift = i.modifiers.shift;
            if i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::ArrowUp) {
                act = Some(Act::Move(true));
            } else if i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::ArrowDown) {
                act = Some(Act::Move(false));
            } else if !shift && i.consume_key(Modifiers::NONE, Key::Enter) {
                act = Some(Act::Enter(from));
            } else if from == 0 && to == 0 && i.consume_key(Modifiers::NONE, Key::Backspace) {
                act = Some(Act::BackspaceAtStart);
            } else if i.consume_key(Modifiers::SHIFT, Key::Tab) {
                act = Some(Act::Outdent);
            } else if i.consume_key(Modifiers::NONE, Key::Tab) {
                act = Some(Act::Indent);
            } else if !shift && from == 0 && to == 0 && i.consume_key(Modifiers::NONE, Key::ArrowUp) {
                act = Some(Act::Up);
            } else if !shift && from == len && to == len && i.consume_key(Modifiers::NONE, Key::ArrowDown) {
                act = Some(Act::Down);
            }
        });
    }

    let mut text = v.text.clone();
    let strike = if done { Stroke::new(1.0, ink) } else { Stroke::NONE };
    let layout_font = font.clone();
    let hint_font = font.clone();
    let mut layouter = move |ui: &Ui, buf: &dyn egui::TextBuffer, wrap: f32| -> Arc<egui::Galley> {
        let format = TextFormat { font_id: layout_font.clone(), color: ink, strikethrough: strike, ..TextFormat::default() };
        let mut job = LayoutJob::single_section(buf.as_str().to_owned(), format);
        job.wrap.max_width = wrap;
        ui.fonts_mut(|f| f.layout_job(job))
    };
    let hint = match v.look {
        Look::Heading(n) => trf("Heading {n}", &[("n", &n)]),
        Look::Bulleted | Look::Numbered(_) => tr("List").to_string(),
        Look::Checklist(_) => tr("To-do").to_string(),
        Look::Toggle(_) => tr("Toggle list").to_string(),
        Look::Quote => tr("Quote").to_string(),
        Look::Decision { .. } => tr("What was decided").to_string(),
        Look::Link(_) => tr("Link title").to_string(),
        // Where to start: the only line of a new document, or the line being written.
        Look::Paragraph if editable && (focused || views.len() == 1) => tr("Type “/” for blocks").to_string(),
        _ => String::new(),
    };
    // An empty block says what it is.
    let show_hint = text.is_empty() && !hint.is_empty();
    let left = indent + lead + pad;
    let edit = egui::TextEdit::multiline(&mut text)
        .id(id)
        .frame(egui::Frame::NONE)
        .font(font)
        .layouter(&mut layouter)
        .desired_width((col.width() - left - pad).max(40.0))
        .desired_rows(1)
        .interactive(editable)
        .margin(egui::Margin { left: 0, right: 0, top: 3, bottom: 3 });
    let edit = if show_hint { edit.hint_text(egui::RichText::new(hint).font(hint_font).color(t.text_tertiary)) } else { edit };
    let out = ui
        .horizontal_top(|ui| {
            ui.add_space(left);
            edit.show(ui)
        })
        .inner;
    let resp = out.response;
    a11y::text(&resp, tr("Text"), &text);
    // The middle of the first line of words, where the field put them.
    let first_line = out.galley_pos.y + row_h / 2.0;
    let mark_x = col.min.x + indent;
    mark(app, ui, t, doc, v, pos2(mark_x, first_line), resp.rect, size, ink, editable);
    if st.focus == Some(Target::Block(v.id)) {
        resp.request_focus();
        let mut state = out.state;
        let at = egui::text::CCursor::new(st.caret.take().unwrap_or(usize::MAX).min(text.chars().count()));
        state.cursor.set_char_range(Some(egui::text::CCursorRange::one(at)));
        state.store(ui.ctx(), id);
        st.focus = None;
    }
    // Enter splits where the cursor is now, after any letters typed in this same frame; words
    // selected go first, as typing over them would.
    let mut cut = false;
    if let Some(Act::Enter(at)) = &mut act
        && let Some(r) = out.cursor_range
    {
        let (a, b) = (r.primary.index.0.min(r.secondary.index.0), r.primary.index.0.max(r.secondary.index.0));
        if b > a {
            text = text.chars().take(a).chain(text.chars().skip(b)).collect();
            cut = true;
        }
        *at = a;
    }
    if cut {
        queue(app, st, doc, Target::Block(v.id), text.clone(), now);
    } else if resp.changed() {
        let caret_now = out.cursor_range.map(|r| r.primary.index.0);
        // Marks work in every block of words, as in Notion (not in a decision, a link or a signature).
        let marks_apply = matches!(
            v.look,
            Look::Paragraph
                | Look::Heading(_)
                | Look::Bulleted
                | Look::Numbered(_)
                | Look::Checklist(_)
                | Look::Toggle(_)
                | Look::Callout(_)
                | Look::Quote
        );
        let shortcut = marks_apply.then(|| mark_typed(&v.text, &text, caret_now)).flatten();
        match shortcut {
            Some((m, kind)) => {
                shortcut_turn(app, st, doc, v, &text, m, kind);
                // The mark is gone from the words: an Enter in the same frame counts without it.
                if let Some(Act::Enter(at)) = &mut act {
                    *at = at.saturating_sub(m.chars().count());
                }
            }
            None => {
                queue(app, st, doc, Target::Block(v.id), text.clone(), now);
                if marks_apply && let Some(at) = slash::opened(&v.text, &text, caret_now) {
                    st.slash = Some(slash::Slash { block: v.id, at, sel: 0, rect: None });
                }
            }
        }
    }
    let caret_end = out.cursor_range.map(|r| r.primary.index.0);
    slash::show(app, ui, st, t, doc, v.id, &text, caret_end, resp.has_focus(), &out.galley, out.galley_pos, slash_key);
    if let Look::Decision { who, when, why } = &v.look {
        decision_fields(app, ui, t, doc, v.id, [who, when, why], indent + lead + pad, col, editable);
    }
    if let Look::Link(url) = &v.look {
        link_field(app, ui, t, doc, v.id, url, indent + lead + pad, col, editable);
    }
    if pad > 0.0 {
        ui.add_space(pad);
    }
    if let Some(a) = act {
        act_on(app, st, doc, views, index, a);
    }
}

/// The mark left of a block's first line; the box of a task and the arrow of a toggle are buttons.
#[allow(clippy::too_many_arguments)]
fn mark(app: &mut FigApp, ui: &Ui, t: &Tokens, doc: DocId, v: &View, at: egui::Pos2, field: Rect, size: f32, ink: Color32, editable: bool) {
    let p = ui.painter();
    let c = pos2(at.x + MARK_W / 2.0, at.y);
    let button = |what: &str| {
        let r = Rect::from_center_size(c, vec2(20.0, 20.0));
        let resp = ui.interact(r, Id::unique(("gbx_doc_mark", doc.0, v.id.0)), if editable { Sense::click() } else { Sense::hover() });
        a11y::button(&resp, what);
        if resp.hovered() && editable {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        resp
    };
    match &v.look {
        Look::Bulleted => {
            // A dot, a ring, a square, by depth.
            let r = (size * 0.16).clamp(2.2, 5.0);
            match v.depth % 3 {
                0 => {
                    p.circle_filled(c, r, ink);
                }
                1 => {
                    p.circle_stroke(c, r - 0.4, Stroke::new(1.2, ink));
                }
                _ => {
                    p.rect_filled(Rect::from_center_size(c, vec2(r * 1.7, r * 1.7)), CornerRadius::ZERO, ink);
                }
            }
        }
        Look::Numbered(n) => {
            // On the same line as the words: the same font, centred on the same line.
            p.text(pos2(at.x + MARK_W - 6.0, at.y), Align2::RIGHT_CENTER, format!("{n}."), regular(size), ink);
        }
        Look::Checklist(checked) => {
            let resp = button(tr("Done"));
            let r = Rect::from_center_size(c, vec2(15.0, 15.0));
            if *checked {
                p.rect_filled(r, CornerRadius::same(3), t.accent);
                let tick = Stroke::new(1.8, Color32::WHITE);
                p.line_segment([r.left_center() + vec2(3.5, 0.0), r.center_bottom() + vec2(-1.0, -4.0)], tick);
                p.line_segment([r.center_bottom() + vec2(-1.0, -4.0), r.right_top() + vec2(-3.5, 4.0)], tick);
            } else {
                p.rect_stroke(
                    r,
                    CornerRadius::same(3),
                    Stroke::new(1.5, if resp.hovered() { t.text } else { t.icon_secondary }),
                    egui::StrokeKind::Inside,
                );
            }
            if resp.clicked() {
                app.run("doc.block.set", json!({"doc": doc.0, "id": v.id.0, "data": {"checked": !checked}}));
            }
        }
        Look::Toggle(open) => {
            let resp = button(if *open { tr("Close") } else { tr("Open") });
            if resp.hovered() {
                p.rect_filled(Rect::from_center_size(c, vec2(20.0, 20.0)), CornerRadius::same(4), t.hover);
            }
            let tri = if *open {
                vec![c + vec2(-4.5, -2.5), c + vec2(4.5, -2.5), c + vec2(0.0, 3.5)]
            } else {
                vec![c + vec2(-2.5, -4.5), c + vec2(3.5, 0.0), c + vec2(-2.5, 4.5)]
            };
            p.add(Shape::convex_polygon(tri, t.icon, Stroke::NONE));
            if resp.clicked() {
                app.run("doc.block.set", json!({"doc": doc.0, "id": v.id.0, "data": {"open": !open}}));
            }
        }
        Look::Callout(icon) => {
            let icon = if icon.is_empty() { "💡" } else { icon.as_str() };
            p.text(pos2(at.x + 22.0, at.y), Align2::CENTER_CENTER, icon, regular(size * 1.15), t.text);
        }
        Look::Quote => {
            p.line_segment([pos2(at.x + 2.0, field.min.y + 3.0), pos2(at.x + 2.0, field.max.y - 3.0)], Stroke::new(3.0, t.text));
        }
        Look::Decision { .. } | Look::Link(_) => {
            p.line_segment([pos2(at.x + 1.5, field.min.y - 18.0), pos2(at.x + 1.5, field.max.y + 26.0)], Stroke::new(3.0, t.accent));
        }
        _ => {}
    }
}

/// Who, when and why under a decision (the decision log reads them).
#[allow(clippy::too_many_arguments)]
fn decision_fields(app: &mut FigApp, ui: &mut Ui, t: &Tokens, doc: DocId, id: BlockId, values: [&String; 3], left: f32, col: Rect, editable: bool) {
    let labels = [("who", tr("Who")), ("when", tr("When")), ("why", tr("Why"))];
    if !editable {
        let line: Vec<String> = labels.iter().zip(values).filter(|(_, v)| !v.is_empty()).map(|((_, l), v)| format!("{l}: {v}")).collect();
        if !line.is_empty() {
            ui.horizontal(|ui| {
                ui.add_space(left);
                ui.label(egui::RichText::new(line.join(" · ")).font(regular(12.0)).color(t.text_secondary));
            });
        }
        return;
    }
    ui.horizontal(|ui| {
        ui.add_space(left);
        let w = ((col.width() - left - 16.0) / 3.0).max(60.0);
        for ((key, label), value) in labels.iter().zip(values) {
            if let Some(new) = widgets::text_field(ui, Id::unique(("gbx_doc_decision", doc.0, id.0, *key)), value, w, label) {
                app.run("doc.block.set", json!({"doc": doc.0, "id": id.0, "data": {*key: new}}));
            }
        }
    });
}

/// The address of a link block, under its title.
#[allow(clippy::too_many_arguments)]
fn link_field(app: &mut FigApp, ui: &mut Ui, t: &Tokens, doc: DocId, id: BlockId, url: &str, left: f32, col: Rect, editable: bool) {
    ui.horizontal(|ui| {
        ui.add_space(left);
        if editable {
            if let Some(new) =
                widgets::text_field(ui, Id::unique(("gbx_doc_link", doc.0, id.0)), url, (col.width() - left - 8.0).max(80.0), "https://…")
            {
                app.run("doc.block.set", json!({"doc": doc.0, "id": id.0, "data": {"url": new.trim()}}));
            }
        } else {
            ui.label(egui::RichText::new(url).font(regular(12.0)).color(t.text_secondary));
        }
    });
}

/// A Markdown mark was just typed at the start of `v`: the block turns into its kind and the mark goes,
/// in one step.
fn shortcut_turn(app: &mut FigApp, st: &mut State, doc: DocId, v: &View, text: &str, mark: &str, kind: &str) {
    // The words with the mark were never sent, and never will be.
    st.pending.take_if(|p| p.doc == doc && p.target == Target::Block(v.id));
    let rest: String = text.chars().skip(mark.chars().count()).collect();
    let kind = match kind {
        "done" => json!({"type": "checklist", "checked": true}),
        k => json!(k),
    };
    app.run("doc.block.turn_into", json!({"doc": doc.0, "id": v.id.0, "kind": kind, "text": rest}));
    if mark == "---" {
        // A line, and writing goes on below it.
        let place = place_after(app, doc, v.id);
        add_block(app, st, doc, place, "paragraph");
    } else {
        st.focus = Some(Target::Block(v.id));
        st.caret = Some(0);
    }
}

/// The version of `doc` that edits go to.
pub(super) fn current(app: &FigApp, doc: DocId) -> Option<&DocVersion> {
    app.session.doc().ok()?.docs.get(doc)?.current()
}

/// Right after block `id`, among the blocks it sits with.
pub(super) fn place_after(app: &FigApp, doc: DocId, id: BlockId) -> Option<(Option<BlockId>, usize)> {
    current(app, doc)?.locate(id).map(|(parent, at)| (parent, at + 1))
}

fn kind_of(app: &FigApp, doc: DocId, id: BlockId) -> Option<BlockKind> {
    current(app, doc)?.block(id).map(|b| b.kind.clone())
}

/// Move block `v` out of the block it sits in, to just after it.
fn outdent(app: &mut FigApp, doc: DocId, v: &View) {
    let Some(parent) = v.parent else { return };
    let Some((grand, at)) = current(app, doc).and_then(|c| c.locate(parent)) else {
        return;
    };
    let mut params = json!({"doc": doc.0, "id": v.id.0, "index": at + 1});
    if let Some(g) = grand {
        params["parent"] = json!(g.0);
    }
    app.run("doc.block.move", params);
}

/// What a key asked for, done once the block is drawn.
fn act_on(app: &mut FigApp, st: &mut State, doc: DocId, views: &[View], index: usize, act: Act) {
    let Some(v) = views.get(index) else { return };
    // The words typed so far go out first, so the edit works on them.
    if let Some(p) = st.pending.take() {
        commit(app, &p);
    }
    let Some(kind) = kind_of(app, doc, v.id) else { return };
    let text_now = current(app, doc).and_then(|c| c.block(v.id)).map(Block::plain_text).unwrap_or_default();
    let keep = |st: &mut State, caret: usize| {
        st.focus = Some(Target::Block(v.id));
        st.caret = Some(caret);
    };
    match act {
        Act::Enter(at) => {
            let list =
                matches!(kind, BlockKind::Bulleted | BlockKind::Numbered | BlockKind::Checklist { .. } | BlockKind::Toggle { .. } | BlockKind::Quote);
            if text_now.is_empty() && list {
                // An empty list item ends the list: out one level, else plain text.
                if v.parent.is_some() {
                    outdent(app, doc, v);
                } else {
                    app.run("doc.block.turn_into", json!({"doc": doc.0, "id": v.id.0, "kind": "paragraph"}));
                }
                keep(st, 0);
                return;
            }
            if let Some(r) = app.run("doc.block.split", json!({"doc": doc.0, "id": v.id.0, "at": at}))
                && let Some(focus) = r.get("focus").and_then(Value::as_u64)
            {
                st.focus = Some(Target::Block(BlockId(focus)));
                st.caret = Some(0);
            }
        }
        Act::BackspaceAtStart => {
            // A decision, a link or a signature keeps what it holds: Backspace at its start turns it
            // into text only when it holds nothing.
            if !matches!(kind, BlockKind::Paragraph) {
                let turns = match &kind {
                    BlockKind::Heading { .. }
                    | BlockKind::Bulleted
                    | BlockKind::Numbered
                    | BlockKind::Checklist { .. }
                    | BlockKind::Toggle { .. }
                    | BlockKind::Quote
                    | BlockKind::Callout { .. } => true,
                    BlockKind::Decision { who, when, why } => text_now.is_empty() && who.is_empty() && when.is_empty() && why.is_empty(),
                    BlockKind::Link { url } => text_now.is_empty() && url.is_empty(),
                    _ => false,
                };
                if turns {
                    app.run("doc.block.turn_into", json!({"doc": doc.0, "id": v.id.0, "kind": "paragraph"}));
                }
                keep(st, 0);
                return;
            }
            if v.parent.is_some() {
                outdent(app, doc, v);
                keep(st, 0);
                return;
            }
            let above = index.checked_sub(1).and_then(|i| views.get(i));
            match above {
                // Joined to the block above when it is written in and shows all it holds.
                Some(a) if a.look.writable() && !matches!(a.look, Look::Toggle(false)) => {
                    if let Some(r) = app.run("doc.block.merge", json!({"doc": doc.0, "id": v.id.0}))
                        && let Some(into) = r.get("id").and_then(Value::as_u64)
                    {
                        st.focus = Some(Target::Block(BlockId(into)));
                        st.caret = r.get("at").and_then(Value::as_u64).and_then(|n| usize::try_from(n).ok());
                    }
                }
                _ if text_now.is_empty() => {
                    app.run("doc.block.delete", json!({"doc": doc.0, "id": v.id.0}));
                    if let Some(a) = above.filter(|a| a.look.writable()) {
                        st.focus = Some(Target::Block(a.id));
                        st.caret = None;
                    }
                }
                _ => keep(st, 0),
            }
        }
        Act::Indent => {
            // Only under a block that is written in (not a divider, an image or an embed).
            if let Some(b) = v.before.filter(|b| views.iter().any(|w| w.id == *b && w.look.writable())) {
                app.run("doc.block.move", json!({"doc": doc.0, "id": v.id.0, "parent": b.0, "index": u32::MAX}));
                if matches!(kind_of(app, doc, b), Some(BlockKind::Toggle { open: false })) {
                    app.run("doc.block.set", json!({"doc": doc.0, "id": b.0, "data": {"open": true}}));
                }
            }
        }
        Act::Outdent => outdent(app, doc, v),
        Act::Move(up) => {
            let Some((parent, i)) = current(app, doc).and_then(|c| c.locate(v.id)) else { return };
            let count = current(app, doc).and_then(|c| match parent {
                Some(p) => c.block(p).map(|b| b.children.len()),
                None => Some(c.blocks.len()),
            });
            // The index counts the blocks once this one is taken out of its list.
            let to = if up { i.checked_sub(1) } else { Some(i + 1).filter(|n| count.is_some_and(|c| *n < c)) };
            if let Some(to) = to {
                let mut params = json!({"doc": doc.0, "id": v.id.0, "index": to});
                if let Some(p) = parent {
                    params["parent"] = json!(p.0);
                }
                app.run("doc.block.move", params);
            }
        }
        Act::Up | Act::Down => {
            let up = matches!(act, Act::Up);
            let next = if up {
                views.get(..index).and_then(|s| s.iter().rev().find(|w| w.look.writable()))
            } else {
                views.iter().skip(index + 1).find(|w| w.look.writable())
            };
            if let Some(n) = next {
                st.focus = Some(Target::Block(n.id));
                st.caret = if up { None } else { Some(0) };
            }
        }
    }
}

/// Add a block of `kind` at `place` (inside which block, at which index; the end of the top when
/// `None`) and give it the keyboard.
pub(super) fn add_block(app: &mut FigApp, st: &mut State, doc: DocId, place: Option<(Option<BlockId>, usize)>, kind: &str) {
    let mut params = json!({"doc": doc.0, "kind": kind});
    if let Some((parent, index)) = place {
        params["index"] = json!(index);
        if let Some(p) = parent {
            params["parent"] = json!(p.0);
        }
    }
    if let Some(id) = app.run("doc.block.insert", params).and_then(|r| r.get("id").and_then(Value::as_u64)) {
        st.focus = Some(Target::Block(BlockId(id)));
        st.caret = None;
    }
}

/// The kinds a block can be turned into, as the handle menu lists them.
pub(super) fn kinds() -> [(&'static str, &'static str); 13] {
    [
        (tr("Text"), "paragraph"),
        (tr("Heading 1"), "heading1"),
        (tr("Heading 2"), "heading2"),
        (tr("Heading 3"), "heading3"),
        (tr("Bulleted list"), "bulleted"),
        (tr("Numbered list"), "numbered"),
        (tr("To-do list"), "checklist"),
        (tr("Toggle list"), "toggle"),
        (tr("Callout"), "callout"),
        (tr("Quote"), "quote"),
        (tr("Decision"), "decision"),
        (tr("Link"), "link"),
        (tr("Signature"), "signature"),
    ]
}

/// The colours a block's words or background can take: (name, words, background). Chosen to read on
/// the light page and the dark editor alike; backgrounds are see-through.
fn colours() -> [(&'static str, &'static str, &'static str); 9] {
    [
        (tr("Gray"), "#8B8F96", "#8B8F9633"),
        (tr("Brown"), "#A0704F", "#A0704F33"),
        (tr("Orange"), "#DB7B2B", "#DB7B2B33"),
        (tr("Yellow"), "#C9A227", "#E0B92A40"),
        (tr("Green"), "#3E9A62", "#3E9A6233"),
        (tr("Blue"), "#3A7FD5", "#3A7FD533"),
        (tr("Purple"), "#8E5CC9", "#8E5CC933"),
        (tr("Pink"), "#CF4F93", "#CF4F9333"),
        (tr("Red"), "#D9483B", "#D9483B33"),
    ]
}

/// The handle on the left of a block: it shows while the pointer is on the block; its menu turns the
/// block into another kind, duplicates it, colours it or deletes it.
#[allow(clippy::too_many_arguments)]
fn handle(app: &mut FigApp, ui: &mut Ui, st: &mut State, t: &Tokens, doc: DocId, v: &View, row: Rect, editable: bool) {
    if !editable {
        return;
    }
    let id = Id::unique(("gbx_doc_handle", doc.0, v.id.0));
    let menu_id = id.with("menu");
    let open = egui::Popup::is_id_open(ui.ctx(), menu_id);
    let dragged = st.block_drag == Some(v.id);
    // While a block is dragged, only its own handle shows (it holds the drag).
    let hover = (ui.rect_contains_pointer(row) && st.block_drag.is_none()) || open || dragged;
    if !hover {
        return;
    }
    let r = Rect::from_min_size(pos2(row.min.x + 6.0 + v.depth as f32 * STEP, row.min.y + 3.0), vec2(22.0, 24.0));
    #[cfg(test)]
    LAST_HANDLE.with(|h| h.set(Some(r)));
    let resp = ui.interact(r, id, Sense::click_and_drag()).tip(tr("Click for the menu, drag to move"));
    if resp.drag_started() {
        st.block_drag = Some(v.id);
    }
    if resp.drag_stopped() && dragged {
        // egui ends a drag on Escape too: that one is cancelled, not dropped.
        if ui.input(|i| i.key_pressed(Key::Escape)) {
            st.block_drag = None;
        } else {
            st.block_drop = true;
        }
    }
    if resp.dragged() || dragged {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    } else if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
    }
    if resp.hovered() || open || dragged {
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
        ui.set_min_width(256.0);
        let tk = Tokens::get(ui.ctx());
        if menu_item(ui, tr("Duplicate"), "") {
            app.run("doc.block.duplicate", json!({"doc": doc.0, "id": v.id.0}));
            ui.close();
        }
        if menu_item(ui, tr("Delete"), "") {
            app.run("doc.block.delete", json!({"doc": doc.0, "id": v.id.0}));
            ui.close();
        }
        ui.separator();
        widgets::label(ui, tr("Turn into"), tk.text_secondary);
        for (label, kind) in kinds() {
            if menu_item(ui, label, "") {
                app.run("doc.block.turn_into", json!({"doc": doc.0, "id": v.id.0, "kind": kind}));
                ui.close();
            }
        }
        ui.separator();
        if v.look.writable() {
            widgets::label(ui, tr("Colour"), tk.text_secondary);
            if let Some(c) = swatches(ui, &tk, false) {
                app.run("doc.block.set", json!({"doc": doc.0, "id": v.id.0, "style": {"color": c}}));
            }
        }
        widgets::label(ui, tr("Background"), tk.text_secondary);
        if let Some(c) = swatches(ui, &tk, true) {
            app.run("doc.block.set", json!({"doc": doc.0, "id": v.id.0, "style": {"background": c}}));
        }
    });
}

/// A row of colour dots (the first one clears): the picked colour, `Value::Null` to clear.
fn swatches(ui: &mut Ui, t: &Tokens, background: bool) -> Option<Value> {
    let mut picked = None;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
        let none = dot(ui, t, None, tr("Default"));
        if none {
            picked = Some(Value::Null);
        }
        for (name, words, back) in colours() {
            let hex = if background { back } else { words };
            if dot(ui, t, parse_hex(hex), name) {
                picked = Some(json!(hex));
            }
        }
    });
    picked
}

fn dot(ui: &mut Ui, t: &Tokens, c: Option<Color32>, name: &str) -> bool {
    let (r, resp) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::click());
    let resp = resp.tip(name);
    a11y::button(&resp, name);
    if resp.hovered() {
        ui.painter().rect_filled(r, CornerRadius::same(4), t.hover);
    }
    match c {
        Some(c) => {
            ui.painter().circle_filled(r.center(), 7.0, c);
        }
        None => {
            ui.painter().circle_stroke(r.center(), 6.5, Stroke::new(1.0, t.border_strong));
            ui.painter().line_segment([r.center() + vec2(-4.0, 4.0), r.center() + vec2(4.0, -4.0)], Stroke::new(1.0, t.border_strong));
        }
    }
    resp.clicked()
}
