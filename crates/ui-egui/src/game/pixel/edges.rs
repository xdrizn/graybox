//! A board's sides in Pixel mode (G16.7): just outside the sprite's edge the pointer turns into a
//! resize cursor, and dragging a side or corner adds or cuts art pixels there (Alt: on both sides,
//! from the centre). The art stays where it is; the scale stays. Released, it is one
//! `pixel.canvas` step. Not for a 9-sliced layer or an instance (they regrow their art).

use egui::{Color32, CursorIcon, Pos2, Shape, Stroke};
use figcraft_doc::NodeId;
use figcraft_pixel::Sprite;
use serde_json::json;

use super::{Gesture, art_to_page, art_to_screen};
use crate::FigApp;

/// How far outside the sprite's edge (screen points) its side can be grabbed.
const REACH: f32 = 10.0;

/// The side under `pos`: x (-1 left, 1 right, 0 neither) and y (-1 top, 1 bottom, 0 neither).
pub(super) fn side_at(app: &FigApp, id: NodeId, sprite: &Sprite, pos: Pos2) -> Option<(i8, i8)> {
    if !figcraft_engine::cmd::pixel::board::is_board(app.session.doc().ok()?, id) {
        return None;
    }
    let xf = art_to_page(app, id, sprite)?;
    let (w, h) = (f64::from(sprite.width), f64::from(sprite.height));
    let (a, b) = (art_to_screen(app, xf, 0.0, 0.0), art_to_screen(app, xf, w, h));
    let (x0, x1, y0, y1) = (a.x.min(b.x), a.x.max(b.x), a.y.min(b.y), a.y.max(b.y));
    let inside_x = pos.x > x0 - REACH && pos.x < x1 + REACH;
    let inside_y = pos.y > y0 - REACH && pos.y < y1 + REACH;
    if !(inside_x && inside_y) || (pos.x > x0 && pos.x < x1 && pos.y > y0 && pos.y < y1) {
        return None;
    }
    let sx = if pos.x <= x0 {
        -1
    } else if pos.x >= x1 {
        1
    } else {
        0
    };
    let sy = if pos.y <= y0 {
        -1
    } else if pos.y >= y1 {
        1
    } else {
        0
    };
    Some((sx, sy))
}

fn cursor((sx, sy): (i8, i8)) -> CursorIcon {
    match (sx, sy) {
        (0, _) => CursorIcon::ResizeVertical,
        (_, 0) => CursorIcon::ResizeHorizontal,
        (x, y) if x == y => CursorIcon::ResizeNwSe,
        _ => CursorIcon::ResizeNeSw,
    }
}

/// The new box (left, top, right, bottom, in art pixels of the board as it is) a drag from `from` to
/// `to` (screen) on side `sides` gives, at least one art pixel each way.
fn new_box(app: &FigApp, id: NodeId, sprite: &Sprite, sides: (i8, i8), from: Pos2, to: Pos2, alt: bool) -> Option<[i64; 4]> {
    let xf = art_to_page(app, id, sprite)?;
    if xf.determinant().abs() < 1e-12 {
        return None;
    }
    let inv = xf.inverse();
    let (a, b) = (inv * app.to_page(from), inv * app.to_page(to));
    let (dx, dy) = ((b.x - a.x).round(), (b.y - a.y).round());
    if !(dx.is_finite() && dy.is_finite()) {
        return None;
    }
    let max = f64::from(figcraft_pixel::Canvas::MAX_SIDE);
    let (dx, dy) = (dx.clamp(-max, max) as i64, dy.clamp(-max, max) as i64);
    let (w, h) = (i64::from(sprite.width), i64::from(sprite.height));
    let (mut l, mut t, mut r, mut bm) = (0, 0, w, h);
    match sides.0 {
        -1 => {
            l += dx;
            if alt {
                r -= dx;
            }
        }
        1 => {
            r += dx;
            if alt {
                l -= dx;
            }
        }
        _ => {}
    }
    match sides.1 {
        -1 => {
            t += dy;
            if alt {
                bm -= dy;
            }
        }
        1 => {
            bm += dy;
            if alt {
                t -= dy;
            }
        }
        _ => {}
    }
    let max = i64::from(figcraft_pixel::Canvas::MAX_SIDE);
    // At least one art pixel, at most the largest sprite (the side dragged gives way).
    if r - l < 1 {
        if sides.0 < 0 { l = r - 1 } else { r = l + 1 }
    }
    if bm - t < 1 {
        if sides.1 < 0 { t = bm - 1 } else { bm = t + 1 }
    }
    if r - l > max {
        if sides.0 < 0 { l = r - max } else { r = l + max }
    }
    if bm - t > max {
        if sides.1 < 0 { t = bm - max } else { bm = t + max }
    }
    Some([l, t, r, bm])
}

/// A press, drag or release on a board's side. True when it took the pointer.
pub(super) fn pointer(
    app: &mut FigApp,
    ui: &egui::Ui,
    resp: &egui::Response,
    pos: Pos2,
    id: NodeId,
    sprite: &Sprite,
    pressed: bool,
    down: bool,
) -> bool {
    let alt = ui.input(|i| i.modifiers.alt);
    if let Some(Gesture::Board { sides, from, .. }) = app.pixel.gesture.clone() {
        super::set_cursor(app, ui.ctx(), cursor(sides));
        if down {
            app.pixel.gesture = Some(Gesture::Board { sides, from, to: pos, alt });
            return true;
        }
        app.pixel.gesture = None;
        if let Some([l, t, r, b]) = new_box(app, id, sprite, sides, from, pos, alt)
            && [l, t, r, b] != [0, 0, i64::from(sprite.width), i64::from(sprite.height)]
        {
            commit(app, id, [l, t, r, b]);
        }
        return true;
    }
    // A mirror axis's handle sits just past the top and left sides: it comes first.
    if app.pixel.gesture.is_some() || !resp.hovered() || super::input::axis_under(app, id, sprite, pos).is_some() {
        return false;
    }
    let Some(sides) = side_at(app, id, sprite, pos) else { return false };
    super::set_cursor(app, ui.ctx(), cursor(sides));
    if pressed {
        app.pixel.gesture = Some(Gesture::Board { sides, from: pos, to: pos, alt });
    }
    true
}

/// The board takes the box `[left, top, right, bottom]` (art pixels of the board as it is).
fn commit(app: &mut FigApp, id: NodeId, [l, t, r, b]: [i64; 4]) {
    // The art moves into the new area by as much as its top left side moved out.
    app.run("pixel.canvas", json!({"id": id.0, "width": r - l, "height": b - t, "offset": [-l, -t]}));
    app.pixel.decoded = None;
}

/// The new box while a side is dragged: a dashed outline, and its size in art pixels.
pub(super) fn draw(app: &FigApp, painter: &egui::Painter, id: NodeId, sprite: &Sprite) {
    let Some(Gesture::Board { sides, from, to, alt }) = &app.pixel.gesture else { return };
    let Some([l, t, r, b]) = new_box(app, id, sprite, *sides, *from, *to, *alt) else { return };
    let Some(xf) = art_to_page(app, id, sprite) else { return };
    let (l, t, r, b) = (l as f64, t as f64, r as f64, b as f64);
    let q = [
        art_to_screen(app, xf, l, t),
        art_to_screen(app, xf, r, t),
        art_to_screen(app, xf, r, b),
        art_to_screen(app, xf, l, b),
        art_to_screen(app, xf, l, t),
    ];
    let accent = crate::theme::Tokens::get(painter.ctx()).accent;
    painter.add(Shape::dashed_line(&q, Stroke::new(1.5, accent), 6.0, 4.0));
    let label = format!("{} × {}", (r - l) as i64, (b - t) as i64);
    let at = q[2] + egui::vec2(8.0, 8.0);
    let galley = painter.layout_no_wrap(label, crate::theme::regular(11.0), Color32::WHITE);
    let bg = egui::Rect::from_min_size(at, galley.size() + egui::vec2(10.0, 4.0));
    painter.rect_filled(bg, egui::CornerRadius::same(4), accent);
    painter.galley(at + egui::vec2(5.0, 2.0), galley, Color32::WHITE);
}
