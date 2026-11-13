//! A pixel board's size in the Design panel in Pixel mode (G16.7): where a frame's W and H sit, the
//! board shows its drawing area in art pixels, and typing a size grows or cuts it on the right and
//! bottom (`pixel.canvas`). Its scale stays as it is. Outside Pixel mode the layer hugs its used
//! pixels and its W and H are a layer's.

use egui::{Id, Ui};
use figcraft_doc::{Document, NodeId};
use serde_json::json;

use crate::FigApp;
use crate::widgets::{self, Lead};

/// The W and H row of one selected board, in art pixels. False when the selection is not one board
/// (the usual W and H show).
pub(crate) fn dims_row(app: &mut FigApp, ui: &mut Ui, doc: &Document, sel: &[NodeId]) -> bool {
    let [id] = sel else { return false };
    // Only in Pixel mode: in Design the layer hugs its used pixels and its W and H are a layer's.
    if app.pixel.mode != super::Mode::Edit(*id) {
        return false;
    }
    if !figcraft_engine::cmd::pixel::board::is_board(doc, *id) {
        return false;
    }
    let Some(sprite) = doc.node(*id).and_then(|n| figcraft_pixel::store::load(&n.plugin_data)) else { return false };
    let col = widgets::col_width(ui.available_width());
    crate::right::row(ui, 32.0, |ui| {
        let w = widgets::number_field(ui, Id::unique(("graybox_board_w", id.0)), Lead::Text("W"), Some(f64::from(sprite.width)), "", col, 1.0)
            .map(|e| ("width", e));
        let h = widgets::number_field(ui, Id::unique(("graybox_board_h", id.0)), Lead::Text("H"), Some(f64::from(sprite.height)), "", col, 1.0)
            .map(|e| ("height", e));
        for (key, edit) in [w, h].into_iter().flatten() {
            if let widgets::FieldEdit::Set(v) | widgets::FieldEdit::Scrub(v, true) = edit
                && v.is_finite()
            {
                let max = f64::from(figcraft_pixel::Canvas::MAX_SIDE);
                app.run("pixel.canvas", json!({"id": id.0, (key): v.round().clamp(1.0, max)}));
            }
        }
        ui.label(egui::RichText::new("art px").size(11.0).color(crate::theme::Tokens::get(ui.ctx()).text_secondary))
            .on_hover_text("The board's size in art pixels (its scale is in Pixel art)");
    });
    true
}
