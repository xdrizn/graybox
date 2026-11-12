//! The Documents tab: the tree of documents, Notion-like. Make one with +, click to open, double-click
//! to rename, drag to reorder or nest, the row menu for icon, rename and delete (with a question first).

use std::collections::BTreeSet;

use egui::{Align2, CornerRadius, Pos2, Rect, Sense, Ui, pos2, vec2};
use figcraft_doc::docs::{DocId, Documents};
use serde_json::{Value, json};

use super::state::{self, State, Target};
use crate::a11y;
use crate::game::tree_list::{self, DragEnd};
use crate::i18n::{tr, trf, trn};
use crate::icons::{self, Icon};
use crate::theme::{Tokens, regular, semibold};
use crate::{FigApp, widgets};

/// The icons a document can wear: simple symbols and emoji (the person can pick any of them).
pub(super) const ICONS: [&str; 24] =
    ["📄", "📝", "📋", "📌", "📎", "⭐", "✅", "⚠", "💡", "🎮", "🎨", "🧩", "🔒", "📦", "🗂", "🎯", "🚀", "❤", "🔥", "👥", "📅", "🔧", "📈", "❓"];

/// What a title shows when it is empty.
pub(super) fn shown_title(title: &str) -> &str {
    if title.trim().is_empty() { tr("Untitled") } else { title }
}

/// A row of the tree, copied out of the file so drawing it does not hold the file.
struct Row {
    id: DocId,
    depth: usize,
    title: String,
    icon: String,
    has_children: bool,
    expanded: bool,
}

fn rows(docs: &Documents, expanded: &BTreeSet<DocId>) -> Vec<Row> {
    let mut out = Vec::new();
    let mut stack: Vec<(DocId, usize)> = docs.roots().iter().rev().map(|d| (*d, 0)).collect();
    while let Some((id, depth)) = stack.pop() {
        let Some(d) = docs.get(id) else { continue };
        let open = expanded.contains(&id);
        out.push(Row { id, depth, title: d.title.clone(), icon: d.icon.clone(), has_children: !d.children.is_empty(), expanded: open });
        if open {
            stack.extend(d.children.iter().rev().map(|c| (*c, depth + 1)));
        }
    }
    out
}

/// Where a dragged document would land.
pub(super) type Drop = tree_list::Drop<DocId>;

pub(super) fn panel(app: &mut FigApp, ui: &mut Ui) {
    let Some(uid) = state::uid(app) else { return };
    let mut st = state::take(uid);
    body(app, ui, &mut st);
    state::put(uid, st);
}

fn body(app: &mut FigApp, ui: &mut Ui, st: &mut State) {
    let t = Tokens::get(ui.ctx());
    // A document that went away (undo, delete) is not open any more.
    let alive = |id: DocId| app.session.doc().is_ok_and(|d| d.docs.get(id).is_some());
    if st.open.is_some_and(|d| !alive(d)) {
        st.open = None;
    }
    if st.renaming.is_some_and(|d| !alive(d)) {
        st.renaming = None;
    }
    let w = ui.available_width();
    new_document_row(app, ui, st, &t, w);
    let list = app.session.doc().map(|d| rows(&d.docs, &st.expanded)).unwrap_or_default();
    if list.is_empty() {
        ui.add_space(12.0);
        let r = ui.allocate_space(vec2(w, 80.0)).1;
        ui.painter().text(pos2(r.min.x + 16.0, r.min.y), Align2::LEFT_TOP, tr("No documents yet"), semibold(12.0), t.text);
        // The hint wraps onto as many lines as the panel needs.
        let galley = ui.painter().layout(
            tr("Decisions, specs and notes live here, in this file.").to_string(),
            regular(11.0),
            t.text_secondary,
            (w - 32.0).max(40.0),
        );
        ui.painter().galley(pos2(r.min.x + 16.0, r.min.y + 20.0), galley, t.text_secondary);
    }
    let mut rects: Vec<(DocId, Rect)> = Vec::new();
    let scroll = egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        let w = ui.available_width();
        for row in &list {
            let rect = row_ui(app, ui, st, &t, row, w);
            rects.push((row.id, rect));
        }
    });
    // Rows scrolled out of view still have a rect; only what can be seen is a place to drop.
    let visible = scroll.inner_rect;
    rects.retain(|(_, r)| r.intersects(visible));
    drag_and_drop(app, ui, st, &t, &list, &rects, visible);
    confirm_delete(app, ui.ctx(), st);
}

fn new_document_row(app: &mut FigApp, ui: &mut Ui, st: &mut State, t: &Tokens, w: f32) {
    ui.add_space(6.0);
    let (r, resp) = ui.allocate_exact_size(vec2(w, 32.0), Sense::click());
    a11y::button(&resp, tr("New document"));
    if resp.hovered() {
        ui.painter().rect_filled(r.shrink2(vec2(6.0, 0.0)), CornerRadius::same(5), t.hover);
    }
    let icon = Rect::from_center_size(pos2(r.min.x + 22.0, r.center().y), vec2(18.0, 18.0));
    icons::paint(ui.painter(), icon, Icon::Plus, t.icon);
    ui.painter().text(pos2(r.min.x + 38.0, r.center().y), Align2::LEFT_CENTER, tr("New document"), regular(12.0), t.text);
    if resp.clicked() {
        create(app, st, None);
    }
    ui.add_space(4.0);
}

/// Make a document (inside `parent`), open it and put the keyboard in its title.
fn create(app: &mut FigApp, st: &mut State, parent: Option<DocId>) {
    let params = parent.map_or_else(|| json!({"title": ""}), |p| json!({"title": "", "parent": p.0}));
    let Some(id) = app.run("doc.create", params).and_then(|v| v.get("id").and_then(Value::as_u64)) else { return };
    if let Some(p) = parent {
        st.expanded.insert(p);
    }
    st.open = Some(DocId(id));
    st.focus = Some(Target::Title);
}

/// One row of the tree; returns the room it took.
fn row_ui(app: &mut FigApp, ui: &mut Ui, st: &mut State, t: &Tokens, row: &Row, w: f32) -> Rect {
    let f = tree_list::row(ui, w, Sense::click_and_drag(), row.depth);
    let resp = f.resp.clone();
    let (rect, mid) = (f.rect, f.mid);
    let name = shown_title(&row.title).to_string();
    a11y::button(&resp, &name);
    let selected = st.open == Some(row.id);
    let dragging = st.drag == Some(row.id);
    let hover = tree_list::hovered(ui, &f) && st.drag.is_none();
    tree_list::paint_background(ui, t, &f, selected, hover);
    if row.has_children && tree_list::chevron(ui, t, &f, row.expanded) && !st.expanded.remove(&row.id) {
        st.expanded.insert(row.id);
    }
    // The icon.
    let ink = if dragging { t.text_tertiary } else { t.icon };
    if row.icon.is_empty() {
        tree_list::paint_icon(ui, &f, Icon::Document, ink);
    } else {
        tree_list::paint_glyph(ui, &f, &row.icon, ink);
    }
    // The buttons at the right end, while the pointer is on the row (or its menu is open).
    let menu_id = resp.id.with("menu");
    let menu_open = egui::Popup::is_id_open(ui.ctx(), menu_id) || egui::Popup::is_id_open(ui.ctx(), menu_id.with("ctx"));
    let show_buttons = (hover || menu_open) && st.renaming != Some(row.id);
    let buttons = tree_list::end_buttons(ui, &f, show_buttons, tr("Rename, icon, delete"), tr("Add a document inside"));
    if buttons.plus_clicked {
        create(app, st, Some(row.id));
    }
    let (right, more) = (buttons.right, buttons.dots);
    // The title, or its rename field.
    let text_rect = tree_list::title_rect(&f, right);
    if st.renaming == Some(row.id) {
        let start = std::mem::take(&mut st.rename_start);
        let (new, open) = widgets::rename_field(ui, resp.id.with("rename"), &row.title, text_rect, regular(12.0), start);
        if let Some(new) = new {
            app.run("doc.rename", json!({"id": row.id.0, "title": new}));
        }
        if !open {
            st.renaming = None;
        }
    } else {
        let shown = widgets::elide_font(ui, &name, text_rect.width(), regular(12.0));
        let color = if row.title.trim().is_empty() { t.text_tertiary } else { t.text };
        ui.painter().text(pos2(text_rect.min.x, mid), Align2::LEFT_CENTER, shown, regular(12.0), color);
    }
    // Clicks.
    let on_buttons = tree_list::on_buttons(ui, right);
    if st.renaming != Some(row.id) && resp.clicked() && !on_buttons {
        st.open = Some(row.id);
    }
    if st.renaming != Some(row.id) && resp.double_clicked() && !on_buttons {
        st.renaming = Some(row.id);
        st.rename_start = true;
    }
    if resp.drag_started() && st.renaming != Some(row.id) {
        st.drag = Some(row.id);
    }
    // The menu: from the dots button or a secondary click on the row.
    if let Some(dots) = more {
        crate::right::menu_popup(&dots).id(menu_id).show(|ui| menu_content(app, ui, st, row));
    }
    crate::right::context_popup(&resp).id(menu_id.with("ctx")).show(|ui| menu_content(app, ui, st, row));
    rect
}

fn menu_content(app: &mut FigApp, ui: &mut Ui, st: &mut State, row: &Row) {
    use crate::right::menu_item;
    ui.set_min_width(220.0);
    if menu_item(ui, tr("Rename"), "") {
        st.renaming = Some(row.id);
        st.rename_start = true;
        ui.close();
    }
    if menu_item(ui, tr("Add a document inside"), "") {
        create(app, st, Some(row.id));
        ui.close();
    }
    if row.parent_known() && menu_item(ui, tr("Move to the top level"), "") {
        app.run("doc.move", json!({"id": row.id.0}));
        ui.close();
    }
    ui.add_space(4.0);
    widgets::label(ui, tr("Icon"), Tokens::get(ui.ctx()).text_secondary);
    if let Some(icon) = icon_grid(ui, &row.icon) {
        app.run("doc.set_icon", json!({"id": row.id.0, "icon": icon}));
        ui.close();
    }
    ui.add_space(4.0);
    if menu_item(ui, tr("Delete…"), "") {
        st.confirm_delete = Some(row.id);
        ui.close();
    }
}

impl Row {
    /// Whether the row sits inside another (depth above the top).
    fn parent_known(&self) -> bool {
        self.depth > 0
    }
}

/// The icons to pick from; returns the picked one (empty text: none).
fn icon_grid(ui: &mut Ui, current: &str) -> Option<String> {
    let t = Tokens::get(ui.ctx());
    let mut picked = None;
    const COLS: usize = 8;
    for line in ICONS.chunks(COLS) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
            for icon in line {
                let (r, resp) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::click());
                a11y::button(&resp, icon);
                if resp.hovered() {
                    ui.painter().rect_filled(r, CornerRadius::same(4), t.menu_hover);
                } else if *icon == current {
                    ui.painter().rect_filled(r, CornerRadius::same(4), t.selected_row);
                }
                ui.painter().text(r.center(), Align2::CENTER_CENTER, *icon, regular(14.0), if resp.hovered() { t.menu_text } else { t.text });
                if resp.clicked() {
                    picked = Some((*icon).to_string());
                }
            }
        });
    }
    if !current.is_empty() && crate::right::menu_item(ui, tr("Remove the icon"), "") {
        picked = Some(String::new());
    }
    picked
}

/// While a row is dragged: show where it would land, and move it when the pointer is let go.
fn drag_and_drop(app: &mut FigApp, ui: &mut Ui, st: &mut State, t: &Tokens, list: &[Row], rects: &[(DocId, Rect)], visible: Rect) {
    let Some(dragged) = st.drag else { return };
    let title = list.iter().find(|r| r.id == dragged).map(|r| shown_title(&r.title).to_string()).unwrap_or_default();
    let depth_of = |id: DocId| list.iter().find(|row| row.id == id).map_or(0, |row| row.depth);
    let end = tree_list::drag(ui, t, &title, rects, visible, depth_of, |p| drop_at(app, p, dragged, rects));
    if let DragEnd::Done(target) = end {
        st.drag = None;
        if let Some(drop) = target {
            apply_drop(app, st, dragged, drop);
        }
    }
}

/// The drop under the pointer (see [`tree_list::drop_at`]). Not on the dragged document or anything
/// inside it.
fn drop_at(app: &FigApp, p: Pos2, dragged: DocId, rects: &[(DocId, Rect)]) -> Option<Drop> {
    let docs = &app.session.doc().ok()?.docs;
    tree_list::drop_at(p, rects, |id| docs.is_inside(id, dragged), |_| true)
}

pub(super) fn apply_drop(app: &mut FigApp, st: &mut State, dragged: DocId, drop: Drop) {
    let Ok(d) = app.session.doc() else { return };
    let docs = &d.docs;
    let from = docs.get(dragged).and_then(|x| x.parent);
    let at = docs.siblings(from).iter().position(|s| *s == dragged);
    let (parent, index) = match drop {
        Drop::Inside(t) => (Some(t), None),
        Drop::Before(t) | Drop::After(t) => {
            let parent = docs.get(t).and_then(|x| x.parent);
            let sib = docs.siblings(parent);
            let mut pos = sib.iter().position(|s| *s == t).unwrap_or(0) + usize::from(matches!(drop, Drop::After(_)));
            if let Some(q) = sib.iter().position(|s| *s == dragged)
                && q < pos
            {
                pos -= 1;
            }
            (parent, Some(pos))
        }
    };
    // Dropped where it already is: nothing to do.
    if parent == from && index.is_some() && index == at {
        return;
    }
    let mut params = json!({"id": dragged.0});
    if let Some(p) = parent {
        params["parent"] = json!(p.0);
        st.expanded.insert(p);
    }
    if let Some(i) = index {
        params["index"] = json!(i);
    }
    app.run("doc.move", params);
}

/// The question before a delete.
fn confirm_delete(app: &mut FigApp, ctx: &egui::Context, st: &mut State) {
    let Some(id) = st.confirm_delete else { return };
    let Some((title, inside)) = app.session.doc().ok().and_then(|d| {
        let doc = d.docs.get(id)?;
        let inside = d.docs.walk().into_iter().filter(|o| *o != id && d.docs.is_inside(*o, id)).count();
        Some((shown_title(&doc.title).to_string(), inside))
    }) else {
        st.confirm_delete = None;
        return;
    };
    let mut open = true;
    let mut decided = None;
    crate::dialogs::window(tr("Delete document"), ctx).open(&mut open).show(ctx, |ui| {
        let t = Tokens::get(ui.ctx());
        ui.set_width(300.0);
        ui.label(egui::RichText::new(trf("Delete “{title}”?", &[("title", &title)])).font(semibold(13.0)).color(t.text));
        let note = if inside > 0 {
            trn("It also deletes the {n} document inside it. You can undo this.", inside, &[("n", &inside)])
        } else {
            tr("You can undo this.").to_string()
        };
        ui.label(egui::RichText::new(note).font(regular(11.0)).color(t.text_secondary));
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            if widgets::button(ui, tr("Cancel"), false, true).clicked() {
                decided = Some(false);
            }
            if widgets::button(ui, tr("Delete"), true, true).clicked() {
                decided = Some(true);
            }
        });
    });
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        decided = Some(false);
    }
    if decided == Some(true) {
        app.run("doc.delete", json!({"id": id.0}));
    }
    if decided.is_some() || !open {
        st.confirm_delete = None;
    }
}
