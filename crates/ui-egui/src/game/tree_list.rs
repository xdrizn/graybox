//! The tree list both the Docs tab and the Pages list use, so the two look and behave the same: a row
//! 28 high with a chevron, an icon and a title, indented 16 per level, a row that lights up under the
//! pointer, `+` and `…` buttons at its end while the pointer is on it, and drag-and-drop where the
//! top quarter of a row drops before it, the bottom quarter after it and the middle inside it.
//!
//! This module only draws and measures; what a row means (a document, a page) and what a drop does
//! stay with the caller, which feeds it rows and acts on what it reports. Colours come from
//! `theme::Tokens`.

use egui::{Align2, Color32, CornerRadius, Id, Pos2, Rect, Response, Sense, Shape, Stroke, StrokeKind, Ui, pos2, vec2};

use crate::a11y::Tip;
use crate::icons::{self, Icon};
use crate::theme::{ACCENT, Tokens, regular};
use crate::widgets;

/// A row's height.
pub(crate) const ROW_H: f32 = 28.0;
/// How far each level is pushed in.
pub(crate) const INDENT: f32 = 16.0;
const PAD: f32 = 8.0;
/// The `+` and `…` buttons' size.
const BUTTON: f32 = 22.0;

/// Where a dragged row would land, next to the row `T` or inside it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Drop<T> {
    Before(T),
    After(T),
    Inside(T),
}

impl<T: Copy> Drop<T> {
    /// The row the drop is next to or inside.
    pub(crate) fn row(self) -> T {
        match self {
            Self::Before(id) | Self::After(id) | Self::Inside(id) => id,
        }
    }
}

/// What a row occupies: its rectangle, the part that lights up, and the lines its pieces sit on.
pub(crate) struct Frame {
    pub rect: Rect,
    pub inner: Rect,
    pub resp: Response,
    /// Where the chevron's column starts (the indent included).
    pub x0: f32,
    pub mid: f32,
}

/// Room for a row `w` wide at the cursor, `depth` levels in.
pub(crate) fn row(ui: &mut Ui, w: f32, sense: Sense, depth: usize) -> Frame {
    let (rect, resp) = ui.allocate_exact_size(vec2(w, ROW_H), sense);
    Frame { rect, inner: rect.shrink2(vec2(6.0, 1.0)), resp, x0: indent_x(rect, depth), mid: rect.center().y }
}

/// Where the content of a row `depth` levels in starts.
pub(crate) fn indent_x(rect: Rect, depth: usize) -> f32 {
    rect.min.x + PAD + 6.0 + depth as f32 * INDENT
}

/// Whether the pointer is on the row. The row's own response stops counting as hovered once one of its
/// `+` or `…` buttons (drawn on top of it) is under the pointer, and those only exist while the row is
/// hovered: asking the rectangle keeps them there until they are clicked.
pub(crate) fn hovered(ui: &Ui, f: &Frame) -> bool {
    f.resp.hovered() || ui.rect_contains_pointer(f.rect)
}

/// The row's background: selected, or lit under the pointer.
pub(crate) fn paint_background(ui: &Ui, t: &Tokens, f: &Frame, selected: bool, hover: bool) {
    if selected {
        ui.painter().rect_filled(f.inner, CornerRadius::same(5), t.selected_row);
    } else if hover {
        ui.painter().rect_filled(f.inner, CornerRadius::same(5), t.hover);
    }
}

/// The chevron, a hit area of its own on top of the row; true when it was clicked.
pub(crate) fn chevron(ui: &mut Ui, t: &Tokens, f: &Frame, expanded: bool) -> bool {
    let hit = Rect::from_center_size(pos2(f.x0 + 6.0, f.mid), vec2(16.0, ROW_H - 4.0));
    let a = ui.interact(hit, f.resp.id.with("arrow"), Sense::click());
    arrow(ui.painter(), hit.center(), expanded, if a.hovered() { t.text } else { t.icon_secondary });
    a.clicked()
}

fn arrow(painter: &egui::Painter, at: Pos2, open: bool, color: Color32) {
    let s = 3.5;
    let pts = if open {
        vec![at + vec2(-s, -s * 0.6), at + vec2(s, -s * 0.6), at + vec2(0.0, s * 0.8)]
    } else {
        vec![at + vec2(-s * 0.6, -s), at + vec2(-s * 0.6, s), at + vec2(s * 0.8, 0.0)]
    };
    painter.add(Shape::convex_polygon(pts, color, Stroke::NONE));
}

/// Where a row's icon is centred.
pub(crate) fn icon_centre(f: &Frame) -> Pos2 {
    pos2(f.x0 + 26.0, f.mid)
}

/// A built-in icon in the icon column.
pub(crate) fn paint_icon(ui: &Ui, f: &Frame, icon: Icon, ink: Color32) {
    icons::paint(ui.painter(), Rect::from_center_size(icon_centre(f), vec2(16.0, 16.0)), icon, ink);
}

/// The row's emoji or symbol in the icon column, or the built-in `fallback` when there is none or the
/// fonts cannot draw it (never a box).
pub(crate) fn paint_glyph_or(ui: &Ui, f: &Frame, glyph: &str, fallback: Icon, ink: Color32) {
    if crate::game::emoji_picker::can_draw(ui.ctx(), glyph) {
        ui.painter().text(icon_centre(f), Align2::CENTER_CENTER, glyph, regular(crate::game::emoji_picker::SIZE), ink);
    } else {
        paint_icon(ui, f, fallback, ink);
    }
}

/// A button the size of the row's `+` and `…`.
pub(crate) fn small_button(ui: &mut Ui, id: Id, rect: Rect, icon: Icon, tip: &str) -> Response {
    let t = Tokens::get(ui.ctx());
    let resp = ui.interact(rect, id, Sense::click()).tip(tip);
    if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(4), t.pressed);
    }
    icons::paint(ui.painter(), rect.shrink(4.0), icon, t.icon);
    resp
}

/// What the buttons at a row's end did.
pub(crate) struct Buttons {
    /// The `…` button, to hang the row's menu on (`None` while the buttons are not shown).
    pub dots: Option<Response>,
    pub plus_clicked: bool,
    /// Where the free part of the row ends.
    pub right: f32,
}

/// The `+` and `…` buttons at the end of the row, when `show` (the pointer is on it or its menu is
/// open).
pub(crate) fn end_buttons(ui: &mut Ui, f: &Frame, show: bool, dots_tip: &str, plus_tip: &str) -> Buttons {
    let mut right = f.inner.max.x - 4.0;
    if !show {
        return Buttons { dots: None, plus_clicked: false, right };
    }
    let mid = f.mid;
    let dots = Rect::from_center_size(pos2(right - BUTTON / 2.0, mid), vec2(BUTTON, BUTTON));
    right -= BUTTON + 2.0;
    let plus = Rect::from_center_size(pos2(right - BUTTON / 2.0, mid), vec2(BUTTON, BUTTON));
    right -= BUTTON + 2.0;
    let d = small_button(ui, f.resp.id.with("dots"), dots, Icon::Dots, dots_tip);
    let plus_clicked = small_button(ui, f.resp.id.with("plus"), plus, Icon::Plus, plus_tip).clicked();
    Buttons { dots: Some(d), plus_clicked, right }
}

/// The room for the title, between the icon column and `right`.
pub(crate) fn title_rect(f: &Frame, right: f32) -> Rect {
    let text_x = f.x0 + 40.0;
    Rect::from_min_max(pos2(text_x, f.rect.min.y + 3.0), pos2((right - 2.0).max(text_x + 20.0), f.rect.max.y - 3.0))
}

/// Whether the pointer is on the row's end buttons (a click there is not a click on the row).
pub(crate) fn on_buttons(ui: &Ui, right: f32) -> bool {
    ui.input(|i| i.pointer.interact_pos()).is_some_and(|p| p.x > right)
}

/// The drop under the pointer: the top quarter of a row is before it, the bottom quarter after it,
/// the middle inside it. `refuse` rows (the dragged row and what is inside it) take nothing;
/// a row that is not `can_hold` has no middle: its top half is before it, its bottom half after.
pub(crate) fn drop_at<T: Copy + PartialEq>(
    p: Pos2,
    rects: &[(T, Rect)],
    refuse: impl Fn(T) -> bool,
    can_hold: impl Fn(T) -> bool,
) -> Option<Drop<T>> {
    let &(id, r) = rects.iter().find(|(_, r)| r.y_range().contains(p.y) && r.x_range().contains(p.x))?;
    if refuse(id) {
        return None;
    }
    let f = (p.y - r.min.y) / r.height().max(1.0);
    Some(if !can_hold(id) {
        if f < 0.5 { Drop::Before(id) } else { Drop::After(id) }
    } else if f < 0.25 {
        Drop::Before(id)
    } else if f > 0.75 {
        Drop::After(id)
    } else {
        Drop::Inside(id)
    })
}

/// Where a drag stands.
pub(crate) enum DragEnd<T> {
    /// Still being dragged.
    Moving,
    /// The pointer was let go over this drop (`None`: over nothing that takes it).
    Done(Option<Drop<T>>),
}

/// While a row is dragged: show where it would land (a line before or after a row, an outline around
/// the row it would go inside) and what is carried, and report when the pointer is let go. `rects` are
/// the rows that can be seen; `visible` is the area they scroll in; `depth_of` gives a row's indent.
pub(crate) fn drag<T: Copy + PartialEq>(
    ui: &mut Ui,
    t: &Tokens,
    title: &str,
    rects: &[(T, Rect)],
    visible: Rect,
    depth_of: impl Fn(T) -> usize,
    target_at: impl Fn(Pos2) -> Option<Drop<T>>,
) -> DragEnd<T> {
    let ctx = ui.ctx().clone();
    let pointer = ctx.input(|i| i.pointer.interact_pos());
    let released = ctx.input(|i| i.pointer.any_released()) || !ctx.input(|i| i.pointer.any_down());
    let target = pointer.filter(|p| visible.contains(*p)).and_then(target_at);
    if let Some(drop) = target {
        let Some(&(_, r)) = rects.iter().find(|(id, _)| *id == drop.row()) else { return DragEnd::Done(None) };
        let x0 = indent_x(r, depth_of(drop.row()));
        match drop {
            Drop::Before(_) => widgets::insertion_line(ui.painter(), x0, r.max.x - 6.0, r.min.y),
            Drop::After(_) => widgets::insertion_line(ui.painter(), x0, r.max.x - 6.0, r.max.y),
            Drop::Inside(_) => {
                ui.painter().rect_stroke(r.shrink2(vec2(6.0, 1.0)), CornerRadius::same(5), Stroke::new(1.5, ACCENT), StrokeKind::Inside);
            }
        }
    }
    if let Some(p) = pointer {
        let galley = ui.painter().layout_no_wrap(title.to_string(), regular(11.5), t.text);
        widgets::drag_ghost(&ctx, p, galley);
    }
    if released {
        DragEnd::Done(target)
    } else {
        ctx.request_repaint();
        DragEnd::Moving
    }
}
