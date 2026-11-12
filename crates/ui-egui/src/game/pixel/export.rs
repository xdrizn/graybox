//! Export and Import in the Pixel art section (G16.5). A preset picks the format with settings a
//! game artist would start from (a Unity sheet, a sprite sheet with JSON, a PNG, a PNG per frame,
//! a GIF); the scale, which frames, trim, padding and layout can then be changed. Export asks for
//! a folder (in the browser each file downloads). Import reads a PNG, cut into frames when a frame
//! size is given, as a new pixel layer beside the selected one. The settings last for the session.

use egui::{Id, Ui};
use figcraft_doc::{Document, NodeId};
use figcraft_pixel::Sprite;
use figcraft_pixel::export::Layout;
use serde_json::{Value, json};

use super::panel::padded;
use crate::FigApp;
use crate::theme::Tokens;
use crate::widgets::{self, Lead};

/// The export presets: label, format, scale, padding.
const PRESETS: [(&str, &str, u32, u32); 5] = [
    ("Unity sheet", "unity", 1, 1),
    ("Sprite sheet + JSON", "sheet", 1, 1),
    ("PNG", "png", 1, 0),
    ("PNG per frame", "frames", 1, 0),
    ("GIF", "gif", 4, 0),
];

/// What Export and Import use, kept while Graybox runs.
pub(crate) struct Prefs {
    preset: usize,
    scale: u32,
    /// Only this tag's frames.
    tag: Option<String>,
    trim: bool,
    padding: u32,
    layout: Layout,
    /// Import: the frame size a sheet is cut by (0 = the whole picture).
    frame: [u32; 2],
}

impl Default for Prefs {
    fn default() -> Self {
        Self { preset: 0, scale: 1, tag: None, trim: false, padding: 1, layout: Layout::Packed, frame: [0, 0] }
    }
}

impl Prefs {
    fn format(&self) -> &'static str {
        PRESETS.get(self.preset).map_or("unity", |p| p.1)
    }

    fn sheet(&self) -> bool {
        matches!(self.format(), "unity" | "sheet")
    }

    /// `pixel.export`'s params for layer `id`.
    fn params(&self, id: NodeId) -> Value {
        let mut p = json!({"id": id.0, "format": self.format(), "scale": self.scale, "trim": self.trim});
        if let Some(o) = p.as_object_mut() {
            if let Some(tag) = &self.tag {
                o.insert("tag".into(), json!(tag));
            }
            if self.sheet() {
                o.insert("padding".into(), json!(self.padding));
                o.insert("layout".into(), json!(self.layout.name()));
            }
        }
        p
    }
}

/// The Export and Import rows for pixel layer `id`.
pub(super) fn section(app: &mut FigApp, ui: &mut Ui, doc: &Document, id: NodeId, sprite: &Sprite, t: &Tokens) {
    if app.pixel.export.tag.as_ref().is_some_and(|tag| sprite.tag(tag).is_none()) {
        app.pixel.export.tag = None;
    }
    widgets::section_header(ui, "Export pixel art", &[], false);
    let col = widgets::col_width(ui.available_width());
    let label = PRESETS.get(app.pixel.export.preset).map_or("", |p| p.0);
    crate::right::row(ui, 32.0, |ui| {
        let resp = widgets::dropdown(ui, label, col * 2.0, None).on_hover_text("What to export");
        crate::right::menu_popup(&resp).show(|ui| {
            for (i, (name, _, scale, padding)) in PRESETS.iter().enumerate() {
                if crate::right::menu_item(ui, name, "") {
                    let e = &mut app.pixel.export;
                    (e.preset, e.scale, e.padding) = (i, *scale, *padding);
                    ui.close();
                }
            }
        });
    });
    crate::right::row(ui, 32.0, |ui| {
        let e = &mut app.pixel.export;
        let edit =
            widgets::number_field(ui, Id::unique(("graybox_pixel_export_scale", id.0)), Lead::Text("Scale"), Some(f64::from(e.scale)), "×", col, 1.0);
        if let Some(widgets::FieldEdit::Set(v) | widgets::FieldEdit::Scrub(v, _)) = edit {
            e.scale = v.round().clamp(1.0, f64::from(figcraft_pixel::export::MAX_SCALE)) as u32;
        }
        if e.sheet() {
            let edit = widgets::number_field(
                ui,
                Id::unique(("graybox_pixel_export_pad", id.0)),
                Lead::Text("Pad"),
                Some(f64::from(e.padding)),
                "px",
                col,
                1.0,
            );
            if let Some(widgets::FieldEdit::Set(v) | widgets::FieldEdit::Scrub(v, _)) = edit {
                e.padding = v.round().clamp(0.0, f64::from(figcraft_pixel::export::MAX_PADDING)) as u32;
            }
        }
    });
    if !sprite.tags.is_empty() {
        frames_row(app, ui, sprite, col);
    }
    padded(ui, |ui| {
        let e = &mut app.pixel.export;
        if e.sheet() {
            let at = match e.layout {
                Layout::Packed => 0,
                Layout::Grid => 1,
                Layout::Row => 2,
            };
            if let Some(i) = widgets::segmented_text(ui, &["Packed", "Grid", "Row"], at) {
                e.layout = [Layout::Packed, Layout::Grid, Layout::Row].get(i).copied().unwrap_or_default();
            }
        }
        if widgets::checkbox(ui, e.trim, "Trim empty edges") {
            e.trim = !e.trim;
        }
    });
    padded(ui, |ui| {
        if super::panel::text_button(ui, "Export…", "Write the files into a folder", t, true) {
            export(app, id);
        }
    });
    import_rows(app, ui, doc, id, t, col);
}

/// "Frames": every frame or one tag's.
fn frames_row(app: &mut FigApp, ui: &mut Ui, sprite: &Sprite, col: f32) {
    const ALL: &str = "All frames";
    let current = app.pixel.export.tag.clone().unwrap_or_else(|| ALL.to_string());
    crate::right::row(ui, 32.0, |ui| {
        ui.label("Frames");
        let resp = widgets::dropdown(ui, &current, col * 2.0 - 48.0, None).on_hover_text("Which frames to export");
        crate::right::menu_popup(&resp).show(|ui| {
            for name in std::iter::once(ALL).chain(sprite.tags.iter().map(|t| t.name.as_str())) {
                if crate::right::menu_item(ui, name, "") {
                    app.pixel.export.tag = (name != ALL).then(|| name.to_string());
                    ui.close();
                }
            }
        });
    });
}

fn export(app: &mut FigApp, id: NodeId) {
    let params = app.pixel.export.params(id);
    if cfg!(target_arch = "wasm32") {
        let Some(r) = app.run("pixel.export", params) else { return };
        for f in r["files"].as_array().into_iter().flatten() {
            let name = f["name"].as_str().unwrap_or("sprite").to_string();
            app.download(&name, f);
        }
        return;
    }
    app.pick_folder("Export pixel art to", move |app, dir| {
        let mut p = params;
        if let Some(o) = p.as_object_mut() {
            o.insert("dir".into(), json!(dir.to_string_lossy()));
        }
        if let Some(r) = app.run("pixel.export", p) {
            let n = r["files"].as_array().map_or(0, Vec::len);
            app.toast(format!("Exported {n} file{} to {}", if n == 1 { "" } else { "s" }, dir.display()));
        }
    });
}

/// "Import": a frame size to cut a sheet by, and the button.
fn import_rows(app: &mut FigApp, ui: &mut Ui, doc: &Document, id: NodeId, t: &Tokens, col: f32) {
    widgets::section_header(ui, "Import pixel art", &[], false);
    widgets::note(ui, "Frame size to cut a sheet by (0: one frame)", t.text_secondary);
    crate::right::row(ui, 32.0, |ui| {
        let e = &mut app.pixel.export;
        for (i, lead) in ["W", "H"].into_iter().enumerate() {
            let at = e.frame.get(i).copied().unwrap_or(0);
            let edit =
                widgets::number_field(ui, Id::unique(("graybox_pixel_import", i, id.0)), Lead::Text(lead), Some(f64::from(at)), "px", col, 1.0);
            if let (Some(widgets::FieldEdit::Set(v) | widgets::FieldEdit::Scrub(v, _)), Some(slot)) = (edit, e.frame.get_mut(i)) {
                *slot = v.round().clamp(0.0, f64::from(figcraft_pixel::Canvas::MAX_SIDE)) as u32;
            }
        }
    });
    // Beside the selected layer, in the same parent.
    let (x, y, parent) =
        doc.node(id).map_or((0.0, 0.0, None), |n| (n.transform.translation().x + n.width + 16.0, n.transform.translation().y, n.parent.map(|p| p.0)));
    padded(ui, |ui| {
        if super::panel::text_button(ui, "Import…", "A PNG or a sprite sheet as a new pixel layer", t, true) {
            let [w, h] = app.pixel.export.frame;
            app.pick_file("Import pixel art", &["png"], true, move |app, file| {
                let mut p = json!({"x": x, "y": y});
                if let Some(o) = p.as_object_mut() {
                    for key in ["path", "data"] {
                        if let Some(v) = file.get(key) {
                            o.insert(key.into(), v.clone());
                        }
                    }
                    if let Some(stem) = file["name"].as_str().and_then(|n| std::path::Path::new(n).file_stem()) {
                        o.insert("name".into(), json!(stem.to_string_lossy()));
                    }
                    if w > 0 || h > 0 {
                        o.insert("frameWidth".into(), json!(if w > 0 { w } else { h }));
                        o.insert("frameHeight".into(), json!(if h > 0 { h } else { w }));
                    }
                    if let Some(parent) = parent {
                        o.insert("parent".into(), json!(parent));
                    }
                }
                app.run("pixel.import", p);
            });
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_preset_and_the_rows_become_the_export_params() {
        let mut e = Prefs::default();
        assert_eq!(e.params(NodeId(7)), json!({"id": 7, "format": "unity", "scale": 1, "trim": false, "padding": 1, "layout": "packed"}));
        (e.preset, e.scale, e.tag, e.trim) = (4, 4, Some("idle".into()), true);
        assert_eq!(e.params(NodeId(7)), json!({"id": 7, "format": "gif", "scale": 4, "trim": true, "tag": "idle"}), "a GIF has no sheet settings");
    }
}
