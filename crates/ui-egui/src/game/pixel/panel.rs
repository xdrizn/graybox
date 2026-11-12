//! The Pixel art section of the Design panel, for one selected pixel layer: Edit pixels (Pixel
//! mode), the art size and the scale (screen-size of one art pixel, in page units).

use egui::{Id, Ui};
use figcraft_doc::{Document, NodeId};
use figcraft_pixel::store;
use serde_json::json;

use crate::FigApp;
use crate::icons::Icon;
use crate::widgets::{self, Lead};

/// Draws the section when exactly one pixel layer is selected. Returns whether it drew.
pub(crate) fn section(app: &mut FigApp, ui: &mut Ui, doc: &Document, sel: &[NodeId]) -> bool {
    let [id] = sel else { return false };
    let Some(sprite) = doc.node(*id).and_then(|n| store::load(&n.plugin_data)) else { return false };
    let editing = super::active(app);
    let buttons: &[(Icon, &str)] = if editing { &[] } else { &[(Icon::PixelArt, "Edit pixels (double-click)")] };
    if widgets::section_header(ui, "Pixel art", buttons, false).is_some() {
        super::enter(app, *id);
    }
    let col = widgets::col_width(ui.available_width());
    crate::right::row(ui, 32.0, |ui| {
        let edit =
            widgets::number_field(ui, Id::unique(("graybox_pixel_scale", id.0)), Lead::Text("Scale"), Some(f64::from(sprite.scale)), "×", col, 1.0);
        if let Some(widgets::FieldEdit::Set(v) | widgets::FieldEdit::Scrub(v, true)) = edit {
            app.run("pixel.set", json!({"id": id.0, "scale": v.round().clamp(1.0, 64.0)}));
        }
    });
    let t = crate::theme::Tokens::get(ui.ctx());
    let frames = sprite.frames.len();
    let more = if frames > 1 { format!(", {frames} frames") } else { String::new() };
    widgets::note(ui, &format!("{} × {} art pixels{more}", sprite.width, sprite.height), t.text_secondary);
    true
}
