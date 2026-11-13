//! The emoji picker shared by the Pages list and the Documents tree: a double-click on a row's icon
//! opens it, the picked emoji replaces the icon. Emoji are drawn with the app's own fonts (egui's
//! monochrome emoji font), so only the ones those fonts have are offered: never a box.

use egui::{Align2, CornerRadius, Id, Rect, Response, Sense, Ui, vec2};

use crate::a11y;
use crate::i18n::{tr, tr_noop};
use crate::theme::{Tokens, regular, semibold};

/// The emoji on offer, in groups (the group's name, a catalog key, then its emoji).
pub(crate) const GROUPS: [(&str, &[&str]); 6] = [
    (tr_noop("Pages"), &["📄", "📝", "📋", "📌", "📎", "📁", "📂", "📦", "📖", "📚", "🔖", "📐", "✏", "🖼", "📃", "🏷"]),
    (tr_noop("Game"), &["🎮", "🕹", "🎯", "🏆", "🎲", "⚔", "🛡", "🗺", "👾", "💎", "🔦", "🏰", "🐉", "🔮", "🎵", "🎬"]),
    (tr_noop("Work"), &["✅", "❌", "⚠", "💡", "🔒", "🔑", "🔧", "⚙", "🚀", "🔥", "📈", "📅", "❓", "❗", "💬", "🔔"]),
    (tr_noop("People"), &["😀", "😎", "😴", "😍", "😢", "😡", "👍", "👎", "👀", "👥", "🙌", "👋", "💪", "❤", "💀", "👻"]),
    (tr_noop("Nature"), &["🌱", "🌲", "🌊", "☀", "🌙", "🌋", "🍀", "🌸", "❄", "⚡", "🌍", "🍎", "🐱", "🐶", "🐼", "🐸"]),
    (tr_noop("Shapes"), &["⭐", "★", "☆", "●", "○", "■", "□", "▲", "△", "◆", "◇", "♠", "♥", "♣", "♦", "♪"]),
];

/// Emoji per line of the grid.
const COLS: usize = 8;
/// One cell of the grid.
const CELL: f32 = 28.0;
/// The size emoji are drawn at, in the grid and in the lists.
pub(crate) const SIZE: f32 = 14.0;

/// Whether the app's fonts can draw `glyph` (else it would show as a box).
pub(crate) fn can_draw(ctx: &egui::Context, glyph: &str) -> bool {
    !glyph.is_empty() && ctx.fonts_mut(|f| f.has_glyphs(&regular(SIZE), glyph))
}

/// The grid of emoji with a "Remove the icon" row when there is one; returns the pick (empty text:
/// remove).
pub(crate) fn grid(ui: &mut Ui, current: &str) -> Option<String> {
    let t = Tokens::get(ui.ctx());
    let mut picked = None;
    for (group, emoji) in GROUPS {
        let shown: Vec<&str> = emoji.iter().copied().filter(|e| can_draw(ui.ctx(), e)).collect();
        if shown.is_empty() {
            continue;
        }
        ui.add_space(4.0);
        let (r, _) = ui.allocate_exact_size(vec2(CELL * COLS as f32, 18.0), Sense::hover());
        ui.painter().text(r.left_center() + vec2(4.0, 0.0), Align2::LEFT_CENTER, tr(group), semibold(11.0), t.text_secondary);
        for line in shown.chunks(COLS) {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                for e in line {
                    let (r, resp) = ui.allocate_exact_size(vec2(CELL, CELL), Sense::click());
                    a11y::button(&resp, e);
                    let on = *e == current;
                    if resp.hovered() {
                        ui.painter().rect_filled(r.shrink(1.0), CornerRadius::same(5), t.menu_hover);
                    } else if on {
                        ui.painter().rect_filled(r.shrink(1.0), CornerRadius::same(5), t.selected_row);
                    }
                    let ink = if resp.hovered() { t.menu_text } else { t.text };
                    ui.painter().text(r.center(), Align2::CENTER_CENTER, *e, regular(SIZE + 2.0), ink);
                    if resp.clicked() {
                        picked = Some((*e).to_string());
                    }
                }
            });
        }
    }
    if !current.is_empty() {
        ui.add_space(4.0);
        if crate::right::menu_item(ui, tr("Remove the icon"), "") {
            picked = Some(String::new());
        }
    }
    picked
}

/// The room of a row's icon that reacts to the pointer.
pub(crate) fn hot_rect(centre: egui::Pos2) -> Rect {
    Rect::from_center_size(centre, vec2(22.0, 22.0))
}

/// The icon of a row as a place to open the picker: lit while the pointer is on it (unless a drag is
/// going on), with a tip. Returns its hover response (the row keeps the clicks and drags).
pub(crate) fn hot_icon(ui: &Ui, id: Id, centre: egui::Pos2, quiet: bool) -> Response {
    let r = hot_rect(centre);
    let resp = ui.interact(r, id, Sense::hover());
    if !quiet && resp.hovered() {
        let t = Tokens::get(ui.ctx());
        // A step above the row's own hover, so the icon reads as a button of its own.
        ui.painter().rect_filled(r, CornerRadius::same(5), t.pressed);
    }
    if quiet { resp } else { resp.on_hover_text(tr("Double-click to change the icon")) }
}

/// Whether this frame's double-click on a row landed on its icon.
pub(crate) fn double_clicked_icon(ui: &Ui, row: &Response, centre: egui::Pos2) -> bool {
    row.double_clicked() && ui.input(|i| i.pointer.interact_pos()).is_some_and(|p| hot_rect(centre).contains(p))
}

/// Open the picker `id` on the next frame (from a row's menu, "Change the icon…").
pub(crate) fn request(ctx: &egui::Context, id: Id) {
    ctx.data_mut(|d| d.insert_temp(id.with("request"), true));
}

/// The picker as a popup under the icon `anchor`; `open` (or a [`request`]) opens it this frame.
/// Returns the pick.
pub(crate) fn popup(ui: &Ui, anchor: &Response, id: Id, open: bool, current: &str) -> Option<String> {
    let asked = ui.ctx().data_mut(|d| d.remove_temp::<bool>(id.with("request"))).unwrap_or(false);
    let open = open || asked;
    let mut picked = None;
    egui::Popup::menu(anchor)
        .id(id)
        .width(CELL * COLS as f32)
        .style(crate::right::menu_style)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .open_memory(open.then_some(egui::SetOpenCommand::Bool(true)))
        .show(|ui| {
            egui::ScrollArea::vertical().max_height(340.0).show(ui, |ui| {
                picked = grid(ui, current);
            });
            if picked.is_some() {
                ui.close();
            }
        });
    picked
}
