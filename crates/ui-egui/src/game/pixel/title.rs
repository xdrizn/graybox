//! The open sprite's frame title (G16.8): while a sprite is edited in Pixel mode its name sits
//! just above the board's top-left corner, like a frame's name label on the canvas. Pressing it
//! selects the layer and dragging it moves the whole sprite, because the title is registered as a
//! name label (`ToolState::labels`) and the canvas's own tools take the press.
//!
//! Outside Pixel mode nothing is drawn: the layer goes on hugging the pixels it uses, and the
//! title would sit over the board's empty margin instead of over the art.

use egui::{Painter, Pos2, Rect, pos2, vec2};
use figcraft_doc::NodeId;
use figcraft_geom::Point;
use figcraft_pixel::Sprite;

use super::{Mode, art_to_page, art_to_screen};
use crate::FigApp;

/// How far above the board's top edge the title's text starts (screen points). A little higher
/// than a frame's own label, so the board's top side stays easy to grab (`edges::REACH`).
const UP: f32 = 20.0;
/// The widest a title is drawn before it ends in an ellipsis.
const MAX_W: f32 = 320.0;

/// The board's top-left corner on screen and the layer's name, while sprite `id` is open.
fn anchor(app: &FigApp, id: NodeId, sprite: &Sprite) -> Option<(Pos2, String)> {
    if app.pixel.mode != Mode::Edit(id) {
        return None;
    }
    let name = app.session.doc().ok()?.node(id)?.name.clone();
    let xf = art_to_page(app, id, sprite)?;
    Some((art_to_screen(app, xf, 0.0, 0.0), name))
}

/// The title's screen rect while sprite `id` is open, title and all.
pub(super) fn rect(app: &FigApp, painter: &Painter, id: NodeId, sprite: &Sprite) -> Option<Rect> {
    let (tl, name) = anchor(app, id, sprite)?;
    if !(tl.x.is_finite() && tl.y.is_finite()) {
        return None;
    }
    let galley = crate::bidi::name_galley(painter, &name, crate::theme::regular(11.0), egui::Color32::WHITE, MAX_W);
    let size = galley.size();
    // A press anywhere over the name takes it, a little beyond it so a narrow name is still easy
    // to grab.
    Some(Rect::from_min_size(pos2(tl.x, tl.y - UP - 2.0), vec2(size.x.max(16.0) + 4.0, size.y.max(12.0) + 4.0)))
}

/// Draws the title and registers it as a name label, so the canvas selects and drags the layer.
pub(super) fn draw(app: &mut FigApp, painter: &Painter, id: NodeId, sprite: &Sprite) {
    let Some((tl, name)) = anchor(app, id, sprite) else { return };
    let Some(area) = rect(app, painter, id, sprite) else { return };
    if !area.intersects(app.canvas.rect) {
        return;
    }
    let col = crate::theme::canvas_accent(painter.ctx());
    let galley = crate::bidi::name_galley(painter, &name, crate::theme::regular(11.0), col, MAX_W);
    painter.galley(pos2(tl.x, tl.y - UP), galley, col);
    let quad = [area.left_top(), area.right_top(), area.right_bottom(), area.left_bottom()].map(|p| {
        let page = app.to_page(p);
        Point::new(page.x, page.y)
    });
    app.session.tool.labels.push((id, quad));
}

/// Whether screen `pos` is on the open sprite's title. Pixel mode then lets the press through to
/// the canvas, which finds the name label and selects or drags the layer.
pub(super) fn hits(app: &FigApp, ui: &egui::Ui, id: NodeId, sprite: &Sprite, pos: Pos2) -> bool {
    rect(app, ui.painter(), id, sprite).is_some_and(|r| r.contains(pos))
}
