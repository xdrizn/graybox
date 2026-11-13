//! The Pages list in the left panel, drawn as the same tree the Docs tab uses (`tree_list`): pages can
//! sit inside other pages, with a chevron to expand or collapse them, dragged in (middle of a row) or
//! before/after a row (its top/bottom quarter), or moved with the row's menu. Every page keeps its own
//! canvas; nesting only organises the list. The data and the commands (`page.nest`, `page.unnest`,
//! `page.collapse`, `page.add` with `parent`, `page.delete` with `subpages`) live in
//! `figcraft_doc::page_tree` and `figcraft_engine::cmd::page_tree`, so each move is one undo step.
//!
//! Hooked from `panels/layers.rs` (see `docs/game/STATUS.md`, Upstream hooks).

use std::cell::Cell;
use std::collections::{BTreeSet, HashMap, HashSet};

use egui::{CornerRadius, Id, Pos2, Rect, Sense, Stroke, Ui, pos2, vec2};
use figcraft_doc::{Document, NodeId};
use serde_json::json;

use crate::a11y;
use crate::game::emoji_picker;
use crate::game::tree_list::{self, DragEnd};
use crate::i18n::{tr, trf, trn};
use crate::icons::Icon;
use crate::panels::layers::{dragged_layers, layer_drag_id, page_rename_start_id, pages_list_height};
use crate::theme::{ACCENT, Tokens, regular, semibold};
use crate::{FigApp, widgets};

/// A row of the list, copied out of the file so drawing it does not hold the file.
#[derive(Clone)]
struct Row {
    id: NodeId,
    depth: usize,
    name: String,
    /// The emoji the page wears in place of the page icon (empty: none).
    icon: String,
    divider: bool,
    has_children: bool,
    expanded: bool,
}

/// The rows to show, top to bottom. With a search `filter` it is the flat list of the pages whose name
/// has it; otherwise the tree, with the pages inside a collapsed page left out.
fn rows(doc: &Document, filter: &str) -> Vec<Row> {
    let listed: HashSet<NodeId> = doc.pages.iter().copied().collect();
    let parent_of = |id: NodeId| doc.page_tree.parents.get(&id).copied().filter(|p| *p != id && listed.contains(p));
    let with_children: HashSet<NodeId> = doc.pages.iter().filter_map(|p| parent_of(*p)).collect();
    let mut depth: HashMap<NodeId, usize> = HashMap::new();
    let mut hidden: HashSet<NodeId> = HashSet::new();
    let mut out = Vec::new();
    for &id in &doc.pages {
        let Some(node) = doc.node(id) else { continue };
        let divider = doc.is_divider(id);
        let icon = doc.page_icon(id).unwrap_or_default().to_string();
        // The list is the tree in order, so a page's parent comes before it; if a file says otherwise
        // the page is shown at the top level.
        let up = parent_of(id).filter(|p| depth.contains_key(p));
        let d = up.map_or(0, |p| depth.get(&p).copied().unwrap_or(0) + 1);
        depth.insert(id, d);
        if let Some(p) = up
            && (hidden.contains(&p) || doc.page_tree.collapsed.contains(&p))
        {
            hidden.insert(id);
        }
        if !filter.is_empty() {
            if !divider && node.name.to_lowercase().contains(filter) {
                out.push(Row { id, depth: 0, name: node.name.clone(), icon, divider, has_children: false, expanded: false });
            }
        } else if !hidden.contains(&id) {
            let has_children = with_children.contains(&id);
            let expanded = has_children && !doc.page_tree.collapsed.contains(&id);
            out.push(Row { id, depth: d, name: node.name.clone(), icon, divider, has_children, expanded });
        }
    }
    out
}

/// The page being dragged in the list (temp data).
fn drag_id() -> Id {
    Id::unique("page_tree_drag")
}

/// The page the list last opened the parents of (temp data), so a refused reveal is not retried
/// every frame.
fn reveal_id() -> Id {
    Id::unique("page_tree_reveal")
}

thread_local! {
    /// The page a delete waits for the person to decide about (it has pages inside it).
    static CONFIRM: Cell<Option<(u64, NodeId)>> = const { Cell::new(None) };
}

/// Called when `page` is about to be deleted: a page with pages inside it asks first. True when it did
/// (and the delete waits for the answer).
pub(crate) fn ask_before_delete(app: &mut FigApp, page: NodeId) -> bool {
    let Ok(ds) = app.session.ds() else { return false };
    if !ds.doc.has_subpages(page) {
        return false;
    }
    CONFIRM.with(|c| c.set(Some((ds.uid, page))));
    true
}

/// Draw the tree of pages below the Pages header. `dev` is Dev Mode (rows open pages, nothing edits).
/// Returns the height the list shows, for the line under it, or `None` when there is no file.
pub(crate) fn show(app: &mut FigApp, ui: &mut Ui, dev: bool) -> Option<f32> {
    let t = Tokens::get(ui.ctx());
    let filter = app.ui.page_filter.trim().to_lowercase();
    let filtering = !filter.is_empty();
    let (cur, uid, sel) = app.session.ds().ok().map(|d| (d.page, d.uid, d.selection.clone()))?;
    let page_key = (uid, cur);
    let scroll_to_cur = app.ui.pages_seen_page != Some(page_key);
    // Arriving at a page inside a collapsed page (undo, opening a file) opens the pages above it.
    if scroll_to_cur && !filtering {
        let hidden = app.session.doc().is_ok_and(|d| d.page_hidden(cur));
        let tried: Option<(u64, NodeId)> = ui.data(|d| d.get_temp(reveal_id()));
        if hidden && tried != Some(page_key) {
            ui.data_mut(|d| d.insert_temp(reveal_id(), page_key));
            app.run("page.reveal", json!({"id": cur}));
        }
    }
    let list = rows(app.session.doc().ok()?, &filter);
    let mut drag: Option<NodeId> = ui.data(|d| d.get_temp(drag_id()));
    // A page dragged out of the list, and gone (undo, another file in front): the drag is over.
    if drag.is_some_and(|d| !list.iter().any(|r| r.id == d)) {
        drag = None;
        ui.data_mut(|d| d.remove::<NodeId>(drag_id()));
    }
    // Layers dragged from the list onto a page move there.
    let layer_dragging: Option<NodeId> = ui.data(|d| d.get_temp(layer_drag_id()));
    let pointer = ui.input(|i| i.pointer.hover_pos());
    let mut out = Out::default();
    let mut current_row_rendered = false;
    // A long list scrolls in its own area, so the layers keep their room below it.
    let max_h = pages_list_height(app.ui.pages_height, ui.available_height());
    let mut rects: Vec<(NodeId, Rect)> = Vec::new();
    let scroll = egui::ScrollArea::vertical().id_salt("pages_list").max_height(max_h).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        let w = ui.available_width();
        for row in &list {
            let ctx = RowCtx { t: &t, uid, cur, w, dev, filtering, drag, layer_dragging, pointer };
            let rect = row_ui(app, ui, &ctx, row, &mut out);
            if row.id == cur {
                current_row_rendered = true;
                if scroll_to_cur {
                    ui.scroll_to_rect(rect, None);
                }
            }
            rects.push((row.id, rect));
        }
    });
    if current_row_rendered {
        app.ui.pages_seen_page = Some(page_key);
    }
    // Rows scrolled out of view still have a rect; only what can be seen is a place to drop.
    let visible = scroll.inner_rect;
    rects.retain(|(_, r)| r.intersects(visible));
    // A drop outside the list's visible area isn't on a page row, even one scrolled out of view.
    if !ui.input(|i| i.pointer.latest_pos()).is_some_and(|p| visible.expand2(vec2(0.0, 3.0)).contains(p)) {
        out.move_to = None;
    }
    if let Some(dragged) = drag {
        drag_and_drop(app, ui, &t, &list, &rects, visible, dragged);
    }
    if let Some((id, new)) = out.rename_done {
        app.ui.rename_page = None;
        app.run("page.rename", json!({"id": id, "name": new}));
    }
    if let Some((page, name)) = out.move_to
        && !crate::menus::dev_mode_refuses(app, "object.moveToPage")
    {
        let ids = dragged_layers(layer_dragging, &sel);
        if app.run("object.moveToPage", json!({"ids": ids, "page": page})).is_some() {
            app.toast(trf("Moved to {name}", &[("name", &name)]));
        }
    }
    if let Some(p) = out.select
        && p != cur
    {
        app.run("page.select", json!({"id": p}));
    }
    confirm_delete(app, ui.ctx());
    // `inner_rect` is the room the list was given; a short list shrinks to its rows.
    Some(scroll.inner_rect.height().min(scroll.content_size.y))
}

/// What the rows of one frame ask for, done after the list is drawn.
#[derive(Default)]
struct Out {
    select: Option<NodeId>,
    rename_done: Option<(NodeId, String)>,
    /// Layers dropped on this page.
    move_to: Option<(NodeId, String)>,
}

/// What every row of the frame needs to know.
struct RowCtx<'a> {
    t: &'a Tokens,
    uid: u64,
    cur: NodeId,
    w: f32,
    dev: bool,
    filtering: bool,
    drag: Option<NodeId>,
    layer_dragging: Option<NodeId>,
    pointer: Option<Pos2>,
}

/// One row of the tree; returns the room it took.
fn row_ui(app: &mut FigApp, ui: &mut Ui, c: &RowCtx, row: &Row, out: &mut Out) -> Rect {
    let t = c.t;
    let sense = if c.filtering { Sense::click() } else { Sense::click_and_drag() };
    let f = tree_list::row(ui, c.w, sense, row.depth);
    let resp = f.resp.clone();
    let (id, divider) = (row.id, row.divider);
    let is_cur = id == c.cur;
    if c.dev && resp.drag_started() {
        crate::menus::dev_mode_refuses(app, "page.move");
    }
    let renaming = app.ui.rename_page.as_ref().is_some_and(|(i, _)| *i == id);
    if !c.dev && !c.filtering && !renaming && resp.drag_started() {
        ui.data_mut(|d| d.insert_temp(drag_id(), id));
    }
    a11y::list_item(&resp, if divider { tr("Divider") } else { &row.name }, is_cur);
    a11y::ring_around(ui, f.inner, &resp);
    let hover = tree_list::hovered(ui, &f) && c.drag.is_none();
    if divider && !renaming {
        // A divider is a line between pages: it opens nothing and takes no layers.
        let y = f.mid;
        ui.painter().line_segment([pos2(f.x0 + 4.0, y), pos2(f.inner.max.x - 10.0, y)], Stroke::new(1.0, t.border));
        if hover {
            ui.painter().rect_filled(f.inner, CornerRadius::same(5), t.hover);
        }
    } else {
        tree_list::paint_background(ui, t, &f, is_cur, hover);
    }
    if c.layer_dragging.is_some() && !is_cur && !divider && c.pointer.is_some_and(|p| f.rect.contains(p)) {
        ui.painter().rect_stroke(f.inner, CornerRadius::same(5), Stroke::new(2.0, ACCENT), egui::StrokeKind::Inside);
        if ui.input(|i| i.pointer.any_released()) {
            out.move_to = Some((id, row.name.clone()));
        }
    }
    if row.has_children && tree_list::chevron(ui, t, &f, row.expanded) {
        app.run("page.collapse", json!({"id": id, "collapsed": row.expanded}));
    }
    // The icon: lit under the pointer, a double-click on it opens the emoji picker.
    let mut on_icon = false;
    // Keyed by the page, not the row's place: a list that changes while the picker is open never sends the pick to another page.
    let picker = Id::unique(("page_emoji", id));
    if !divider {
        let centre = tree_list::icon_centre(&f);
        let busy = c.dev || c.drag.is_some() || renaming;
        let icon_resp = emoji_picker::hot_icon(ui, resp.id.with("icon"), centre, busy);
        let ink = if c.drag == Some(id) { t.text_tertiary } else { t.icon };
        tree_list::paint_glyph_or(ui, &f, &row.icon, Icon::Page, ink);
        on_icon = !busy && emoji_picker::double_clicked_icon(ui, &resp, centre);
        if let Some(icon) = emoji_picker::popup(ui, &icon_resp, picker, on_icon, &row.icon) {
            app.run("page.set_icon", json!({"id": id, "icon": icon}));
        }
    }
    // The buttons at the end, while the pointer is on the row (or its menu is open).
    let menu_id = resp.id.with("menu");
    let menu_open = egui::Popup::is_id_open(ui.ctx(), menu_id) || egui::Popup::is_id_open(ui.ctx(), menu_id.with("ctx"));
    let show_buttons = (hover || menu_open) && !renaming && !divider && c.drag.is_none();
    let buttons = tree_list::end_buttons(ui, &f, show_buttons, tr("Rename, move, delete"), tr("Add a subpage inside"));
    if buttons.plus_clicked {
        add_subpage(app, c.uid, id);
    }
    let right = buttons.right;
    // The name, or its rename field.
    let name_rect = tree_list::title_rect(&f, right);
    if renaming {
        let start = ui.data_mut(|d| d.remove_temp::<bool>(page_rename_start_id())).unwrap_or(false);
        let (new, open) = widgets::rename_field(ui, Id::unique(("pagename", id)), &row.name, name_rect, regular(12.0), start);
        if let Some(new) = new {
            out.rename_done = Some((id, new));
        } else if !open {
            // Cancelled, or committed unchanged: the row goes back to its name, or the idle field
            // would catch the next click and a click wouldn't open this page.
            app.ui.rename_page = None;
        }
    } else if !divider {
        let font = if is_cur { semibold(12.0) } else { regular(12.0) };
        crate::bidi::paint_name(ui.painter(), pos2(name_rect.min.x, f.mid), &row.name, font, t.text, name_rect.width());
    }
    // Clicks.
    let on_buttons = tree_list::on_buttons(ui, right);
    if resp.clicked() && !divider && !on_buttons {
        out.select = Some(id);
        app.ui.pages_focus = Some(c.uid);
        app.ui.layers_focus = false;
    }
    if resp.double_clicked() && !on_buttons && !on_icon && !crate::menus::dev_mode_refuses(app, "page.rename") {
        app.ui.rename_page = Some((id, row.name.clone()));
        // The field takes the focus next frame, once it's drawn: focusing it now would put a node
        // that isn't there yet in the screen reader tree, which AccessKit aborts on.
        ui.data_mut(|d| d.insert_temp(page_rename_start_id(), true));
    }
    // The menu: from the dots button or a secondary click on the row.
    if let Some(dots) = buttons.dots {
        crate::right::menu_popup(&dots).id(menu_id).show(|ui| menu_content(app, ui, c, row, picker));
    }
    crate::right::context_popup(&resp).id(menu_id.with("ctx")).show(|ui| menu_content(app, ui, c, row, picker));
    f.rect
}

fn menu_content(app: &mut FigApp, ui: &mut Ui, c: &RowCtx, row: &Row, picker: Id) {
    use crate::right::menu_item;
    let id = row.id;
    ui.set_min_width(220.0);
    if menu_item(ui, tr("Rename page"), "") {
        if !crate::menus::dev_mode_refuses(app, "page.rename") {
            app.ui.rename_page = Some((id, row.name.clone()));
            ui.data_mut(|d| d.insert_temp(page_rename_start_id(), true));
        }
        ui.close();
    }
    if !row.divider && menu_item(ui, tr("Add subpage"), "") {
        add_subpage(app, c.uid, id);
        ui.close();
    }
    if !row.divider && menu_item(ui, tr("Change the icon…"), "") {
        if !crate::menus::dev_mode_refuses(app, "page.set_icon") {
            emoji_picker::request(ui.ctx(), picker);
        }
        ui.close();
    }
    if row.depth > 0 && !c.filtering && menu_item(ui, tr("Move out"), "") {
        if !crate::menus::dev_mode_refuses(app, "page.unnest") {
            app.run("page.unnest", json!({"id": id}));
        }
        ui.close();
    }
    if menu_item(ui, tr("Duplicate page"), "") {
        if !crate::menus::dev_mode_refuses(app, "page.duplicate") {
            app.run("page.duplicate", json!({"id": id}));
        }
        ui.close();
    }
    if menu_item(ui, tr("Delete page"), "") {
        if !crate::menus::dev_mode_refuses(app, "page.delete") {
            crate::layers::delete_page(app, id);
        }
        ui.close();
    }
}

/// Make a page inside `parent` (it opens, and `parent` opens to show it).
fn add_subpage(app: &mut FigApp, uid: u64, parent: NodeId) {
    app.ui.pages_focus = Some(uid);
    if !crate::menus::dev_mode_refuses(app, "page.add") {
        app.run("page.add", json!({"parent": parent}));
    }
}

/// While a row is dragged: show where it would land, and move it when the pointer is let go.
fn drag_and_drop(app: &mut FigApp, ui: &mut Ui, t: &Tokens, list: &[Row], rects: &[(NodeId, Rect)], visible: Rect, dragged: NodeId) {
    // Escape cancels the drag.
    if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
        ui.data_mut(|d| d.remove::<NodeId>(drag_id()));
        return;
    }
    let title = list.iter().find(|r| r.id == dragged).map(|r| if r.divider { tr("Divider").to_string() } else { r.name.clone() }).unwrap_or_default();
    let depth_of = |id: NodeId| list.iter().find(|r| r.id == id).map_or(0, |r| r.depth);
    // Half a row above the first row and below the last still counts: before the first, after the last.
    let slack = tree_list::ROW_H / 2.0;
    let target_at = |mut p: Pos2| {
        if let (Some((_, first)), Some((_, last))) = (rects.first(), rects.last()) {
            p.y = p.y.clamp(first.min.y + 1.0, last.max.y - 1.0);
        }
        drop_at(app, p, dragged, list, rects)
    };
    if let DragEnd::Done(target) = tree_list::drag(ui, t, &title, rects, visible.expand2(vec2(0.0, slack)), depth_of, target_at) {
        ui.data_mut(|d| d.remove::<NodeId>(drag_id()));
        if let Some(drop) = target
            && !crate::menus::dev_mode_refuses(app, "page.nest")
        {
            apply_drop(app, dragged, drop);
        }
    }
}

/// The drop under the pointer (see [`tree_list::drop_at`]). Not on the dragged page or anything inside
/// it; a divider holds nothing, so there is only before and after it.
fn drop_at(app: &FigApp, p: Pos2, dragged: NodeId, list: &[Row], rects: &[(NodeId, Rect)]) -> Option<Drop> {
    let doc = app.session.doc().ok()?;
    let dividers: BTreeSet<NodeId> = list.iter().filter(|r| r.divider).map(|r| r.id).collect();
    tree_list::drop_at(p, rects, |id| id == dragged || doc.page_is_inside(id, dragged), |id| !dividers.contains(&id))
}

/// Where a dragged page would land.
pub(crate) type Drop = tree_list::Drop<NodeId>;

/// Move `dragged` where `drop` says, as one command.
pub(crate) fn apply_drop(app: &mut FigApp, dragged: NodeId, drop: Drop) {
    let Ok(doc) = app.session.doc() else { return };
    let from = doc.page_parent(dragged);
    let at = doc.page_children(from).iter().position(|s| *s == dragged);
    let (parent, index) = match drop {
        Drop::Inside(t) => (Some(t), None),
        Drop::Before(t) | Drop::After(t) => {
            let parent = doc.page_parent(t);
            let sibs = doc.page_children(parent);
            let mut pos = sibs.iter().position(|s| *s == t).unwrap_or(0) + usize::from(matches!(drop, Drop::After(_)));
            if let Some(q) = sibs.iter().position(|s| *s == dragged)
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
    let mut params = json!({"id": dragged});
    if let Some(p) = parent {
        params["parent"] = json!(p);
    }
    if let Some(i) = index {
        params["index"] = json!(i);
    }
    app.run("page.nest", params);
}

/// The question before deleting a page that has pages inside it.
fn confirm_delete(app: &mut FigApp, ctx: &egui::Context) {
    let Some((uid, page)) = CONFIRM.with(Cell::get) else { return };
    let here = app.session.ds().ok().filter(|d| d.uid == uid);
    let Some((name, inside)) = here.and_then(|d| {
        let n = d.doc.node(page)?;
        let inside = d.doc.page_subtree(page).len().saturating_sub(1);
        (inside > 0).then(|| (n.name.clone(), inside))
    }) else {
        CONFIRM.with(|c| c.set(None));
        return;
    };
    let mut open = true;
    let mut decided: Option<Option<bool>> = None;
    crate::dialogs::window(tr("Delete page"), ctx).open(&mut open).show(ctx, |ui| {
        let t = Tokens::get(ui.ctx());
        ui.set_width(320.0);
        ui.label(egui::RichText::new(trf("Delete “{title}”?", &[("title", &name)])).font(semibold(13.0)).color(t.text));
        let note =
            trn("It has {n} subpage inside it. Keep it by moving it up a level, or delete it too. You can undo this.", inside, &[("n", &inside)]);
        ui.label(egui::RichText::new(note).font(regular(11.0)).color(t.text_secondary));
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            if widgets::button(ui, tr("Cancel"), false, true).clicked() {
                decided = Some(None);
            }
            if widgets::button(ui, tr("Keep the subpages"), false, true).clicked() {
                decided = Some(Some(false));
            }
            if widgets::button(ui, tr("Delete with subpages"), true, true).clicked() {
                decided = Some(Some(true));
            }
        });
    });
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        decided = Some(None);
    }
    if let Some(Some(with_subpages)) = decided {
        let params = if with_subpages { json!({"id": page, "subpages": true}) } else { json!({"id": page}) };
        if app.run("page.delete", params).is_some() {
            app.toast(tr("Page deleted"));
        }
    }
    if decided.is_some() || !open {
        CONFIRM.with(|c| c.set(None));
    }
}

#[cfg(test)]
mod tests;
