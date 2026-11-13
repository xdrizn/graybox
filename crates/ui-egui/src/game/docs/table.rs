//! A table block in the document editor (G13.2): a grid of cells written in like blocks (with the
//! looks of their words), Tab and Shift+Tab to the next and previous cell, Enter to the cell below
//! (each adds a row past the end), + bars under and right of the table, and a cell menu to insert or
//! delete rows and columns and switch the header row.

use std::sync::Arc;

use egui::{Align, CornerRadius, Id, Key, Layout, Modifiers, Rect, Sense, Shape, Stroke, Ui, pos2, vec2};
use figcraft_doc::docs::{BlockId, DocId, Table};
use serde_json::json;

use super::editor::{commit, queue};
use super::inline;
use super::state::{State, Target};
use crate::FigApp;
use crate::a11y::{self, Tip};
use crate::i18n::tr;
use crate::theme::{Tokens, regular, semibold};

/// The room on the right of an editable table for its + bar.
const BAR: f32 = 18.0;

/// Where the keyboard goes after a key in a cell.
#[derive(Clone, Copy)]
enum Go {
    Next,
    Prev,
    Down,
}

/// A change asked for from a cell's menu or a + bar.
#[derive(Clone, Copy)]
enum Change {
    Op(&'static str, usize),
    Header(bool),
}

#[cfg(test)]
thread_local! {
    static BARS: std::cell::RefCell<Vec<(&'static str, Rect)>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Where the + bar `name` (`row` or `column`) was last drawn (tests).
#[cfg(test)]
pub(super) fn last_bar(name: &str) -> Option<Rect> {
    BARS.with(|b| b.borrow().iter().rev().find(|(n, _)| *n == name).map(|(_, r)| *r))
}

/// The id of the field of a cell.
pub(super) fn cell_id(doc: DocId, block: BlockId, row: usize, col: usize) -> Id {
    Id::unique(("gbx_doc_cell", doc.0, block.0, row, col))
}

/// Table `block` (`table` as the file has it), its left edge `indent` into the column `col`.
#[allow(clippy::too_many_arguments)]
pub(super) fn show(
    app: &mut FigApp,
    ui: &mut Ui,
    st: &mut State,
    t: &Tokens,
    doc: DocId,
    block: BlockId,
    table: &Table,
    col: Rect,
    indent: f32,
    editable: bool,
    now: f64,
) {
    let (h, w) = (table.rows.len(), table.width().max(1));
    let x0 = col.min.x + indent;
    let width = (col.width() - indent - if editable { BAR } else { 0.0 }).max(60.0);
    let cell_w = width / w as f32;
    let top = ui.cursor().min.y;
    let mut go = None;
    let mut change = None;
    let mut row_lines = Vec::with_capacity(h);
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        for r in 0..h {
            let header = table.header && r == 0;
            let bg = ui.painter().add(Shape::Noop);
            let row_top = ui.cursor().min.y;
            ui.horizontal_top(|ui| {
                ui.add_space(indent);
                for c in 0..w {
                    ui.allocate_ui_with_layout(vec2(cell_w, 0.0), Layout::top_down(Align::Min), |ui| {
                        ui.set_width(cell_w);
                        if let Some(asked) = cell(app, ui, st, t, doc, block, table, (r, c), cell_w, editable, now) {
                            match asked {
                                Ok(g) => go = Some((r, c, g)),
                                Err(ch) => change = Some(ch),
                            }
                        }
                    });
                }
            });
            let row_bottom = ui.min_rect().max.y.max(row_top + 24.0);
            if header {
                ui.painter()
                    .set(bg, Shape::rect_filled(Rect::from_min_max(pos2(x0, row_top), pos2(x0 + width, row_bottom)), CornerRadius::ZERO, t.field));
            }
            row_lines.push(row_bottom);
        }
    });
    let bottom = row_lines.last().copied().unwrap_or(top);
    // The grid: the outline, a line under each row, a line between columns.
    let line = Stroke::new(1.0, t.border_strong);
    let outline = Rect::from_min_max(pos2(x0, top), pos2(x0 + width, bottom));
    ui.painter().rect_stroke(outline, CornerRadius::same(3), line, egui::StrokeKind::Inside);
    for y in row_lines.iter().take(h.saturating_sub(1)) {
        ui.painter().line_segment([pos2(x0, *y), pos2(x0 + width, *y)], line);
    }
    for c in 1..w {
        let x = x0 + cell_w * c as f32;
        ui.painter().line_segment([pos2(x, top), pos2(x, bottom)], line);
    }
    if editable && let Some(ch) = bars(ui, t, doc, block, outline, (h, w)) {
        change = Some(ch);
    }
    ui.add_space(if editable { BAR + 2.0 } else { 6.0 });
    if let Some(ch) = change {
        let _ = apply(app, st, doc, block, ch);
    }
    if let Some((r, c, g)) = go {
        move_to(app, st, doc, block, (h, w), (r, c), g);
    }
}

/// The + bars, shown while the pointer is near the table: one under it adds a row, one on its right
/// adds a column.
fn bars(ui: &mut Ui, t: &Tokens, doc: DocId, block: BlockId, outline: Rect, (h, w): (usize, usize)) -> Option<Change> {
    let near = outline.expand2(vec2(BAR + 4.0, BAR + 4.0));
    if !ui.rect_contains_pointer(near) {
        return None;
    }
    let row = Rect::from_min_size(pos2(outline.min.x, outline.max.y + 3.0), vec2(outline.width(), BAR - 4.0));
    let column = Rect::from_min_size(pos2(outline.max.x + 3.0, outline.min.y), vec2(BAR - 4.0, outline.height()));
    let mut picked = None;
    for (name, r, tip, ch) in
        [("row", row, tr("Add row"), Change::Op("insertRow", h)), ("column", column, tr("Add column"), Change::Op("insertColumn", w))]
    {
        #[cfg(test)]
        BARS.with(|b| b.borrow_mut().push((name, r)));
        let resp = ui.interact(r, Id::unique(("gbx_doc_table_bar", doc.0, block.0, name)), Sense::click()).tip(tip);
        a11y::button(&resp, tip);
        let fill = if resp.hovered() { t.hover } else { t.field };
        ui.painter().rect_filled(r, CornerRadius::same(3), fill);
        let c = r.center();
        let ink = Stroke::new(1.2, t.text_secondary);
        ui.painter().line_segment([c - vec2(4.0, 0.0), c + vec2(4.0, 0.0)], ink);
        ui.painter().line_segment([c - vec2(0.0, 4.0), c + vec2(0.0, 4.0)], ink);
        if resp.clicked() {
            picked = Some(ch);
        }
    }
    picked
}

/// One cell: its field, the keys that move between cells, and its menu. `Ok` says where the keyboard
/// goes, `Err` asks for a change of the table.
#[allow(clippy::too_many_arguments)]
fn cell(
    app: &mut FigApp,
    ui: &mut Ui,
    st: &mut State,
    t: &Tokens,
    doc: DocId,
    block: BlockId,
    table: &Table,
    (r, c): (usize, usize),
    cell_w: f32,
    editable: bool,
    now: f64,
) -> Option<Result<Go, Change>> {
    let id = cell_id(doc, block, r, c);
    let target = Target::Cell(block, r, c);
    let header = table.header && r == 0;
    let runs = table.rows.get(r).and_then(|row| row.get(c)).cloned().unwrap_or_default();
    let mut text = match &st.pending {
        Some(p) if p.doc == doc && p.target == target => p.text.clone(),
        _ => table.cell_text(r, c),
    };
    let focused = ui.memory(|m| m.has_focus(id));
    let mut asked = None;
    if focused && editable {
        // Cells have no looks of their own to set yet, but the format keys must not reach the field:
        // egui reads Ctrl+U and Ctrl+K as deleting to the line's start or end.
        let _ = inline::keys(ui);
        ui.input_mut(|i| {
            if i.consume_key(Modifiers::SHIFT, Key::Tab) {
                asked = Some(Ok(Go::Prev));
            } else if i.consume_key(Modifiers::NONE, Key::Tab) {
                asked = Some(Ok(Go::Next));
            } else if !i.modifiers.shift && i.consume_key(Modifiers::NONE, Key::Enter) {
                asked = Some(Ok(Go::Down));
            }
        });
    }
    let font = if header { semibold(14.0) } else { regular(14.0) };
    let looks = inline::Ink { font: font.clone(), color: t.text, strike: false, link: t.accent, code: t.field };
    let mut layouter = move |ui: &Ui, buf: &dyn egui::TextBuffer, wrap: f32| -> Arc<egui::Galley> {
        let job = inline::job(&runs, buf.as_str(), &looks, wrap);
        ui.fonts_mut(|f| f.layout_job(job))
    };
    // The room around the words comes from a frame: a field without its own frame keeps none.
    let pad = egui::Margin { left: 8, right: 8, top: 5, bottom: 5 };
    let out = egui::Frame::NONE
        .inner_margin(pad)
        .show(ui, |ui| {
            egui::TextEdit::multiline(&mut text)
                .id(id)
                .frame(egui::Frame::NONE)
                .font(font)
                .layouter(&mut layouter)
                .desired_width((cell_w - 16.0).max(20.0))
                .desired_rows(1)
                .interactive(editable)
                .margin(egui::Margin::ZERO)
                .show(ui)
        })
        .inner;
    let resp = out.response;
    a11y::text(&resp, tr("Table"), &text);
    if st.focus == Some(target) {
        resp.request_focus();
        let mut state = out.state;
        let end = egui::text::CCursor::new(text.chars().count());
        state.cursor.set_char_range(Some(egui::text::CCursorRange::one(end)));
        state.store(ui.ctx(), id);
        st.focus = None;
        st.caret = None;
    }
    if resp.changed() {
        queue(app, st, doc, target, text, now);
    }
    if editable {
        crate::right::context_popup(&resp).show(|ui| {
            use crate::right::menu_item;
            let (h, w) = (table.rows.len(), table.width());
            let items = [
                (tr("Insert row above"), Change::Op("insertRow", r)),
                (tr("Insert row below"), Change::Op("insertRow", r + 1)),
                (tr("Insert column left"), Change::Op("insertColumn", c)),
                (tr("Insert column right"), Change::Op("insertColumn", c + 1)),
            ];
            for (label, ch) in items {
                if menu_item(ui, label, "") {
                    asked = Some(Err(ch));
                    ui.close();
                }
            }
            ui.separator();
            if h > 1 && menu_item(ui, tr("Delete row"), "") {
                asked = Some(Err(Change::Op("deleteRow", r)));
                ui.close();
            }
            if w > 1 && menu_item(ui, tr("Delete column"), "") {
                asked = Some(Err(Change::Op("deleteColumn", c)));
                ui.close();
            }
            ui.separator();
            if crate::right::menu_check(ui, table.header, tr("Header row"), "") {
                asked = Some(Err(Change::Header(!table.header)));
                ui.close();
            }
        });
    }
    asked
}

/// Send what was typed, then change the table. Whether the table took the change.
fn apply(app: &mut FigApp, st: &mut State, doc: DocId, block: BlockId, change: Change) -> bool {
    if let Some(p) = st.pending.take() {
        commit(app, &p);
    }
    let params = match change {
        Change::Op(op, at) => json!({"doc": doc.0, "id": block.0, "op": op, "at": at}),
        Change::Header(on) => json!({"doc": doc.0, "id": block.0, "op": "header", "on": on}),
    };
    app.run("doc.table.change", params).is_some()
}

/// Give the keyboard to the cell `go` points at from (`r`, `c`) in a table of `h` rows and `w`
/// columns, adding a row past the last one.
fn move_to(app: &mut FigApp, st: &mut State, doc: DocId, block: BlockId, (h, w): (usize, usize), (r, c): (usize, usize), go: Go) {
    let to = match go {
        Go::Next if c + 1 < w => (r, c + 1),
        Go::Next => (r + 1, 0),
        Go::Prev if c > 0 => (r, c - 1),
        Go::Prev if r > 0 => (r - 1, w - 1),
        Go::Prev => (r, c),
        Go::Down => (r + 1, c),
    };
    // A full table takes no row: the keyboard stays where it is.
    if to.0 >= h && !apply(app, st, doc, block, Change::Op("insertRow", h)) {
        return;
    }
    st.focus = Some(Target::Cell(block, to.0, to.1));
}
