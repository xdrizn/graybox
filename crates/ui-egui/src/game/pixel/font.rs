//! Pixel fonts in the Design panel (G16.6): a pixel layer's "Pixel font" section (turn the layer
//! into a glyph sheet, its name, cell size, baseline, spacing and characters) and the Font menu of
//! the Pixel text section (set text layers in one of the file's pixel fonts).

use egui::{Id, Ui};
use figcraft_doc::{Document, NodeId};
use figcraft_pixel::font::{self, Font};
use figcraft_pixel::{Sprite, store};
use serde_json::{Value, json};

use crate::FigApp;
use crate::icons::Icon;
use crate::theme::Tokens;
use crate::widgets::{self, Lead};

/// The Pixel font section of pixel layer `id`.
pub(super) fn section(app: &mut FigApp, ui: &mut Ui, doc: &Document, id: NodeId, sprite: &Sprite, t: &Tokens) {
    let Some(n) = doc.node(id) else { return };
    let current = n.plugin_data.get(font::KEY).and_then(|s| Font::load(s));
    let buttons: &[(Icon, &str)] = if current.is_some() {
        &[(Icon::Minus, "Stop using this layer as a font")]
    } else {
        &[(Icon::Plus, "Use this layer as a pixel font: a grid of cells, one character each")]
    };
    if widgets::section_header(ui, "Pixel font", buttons, current.is_none()).is_some() {
        let p = if current.is_some() { json!({"id": id.0, "off": true}) } else { json!({"id": id.0}) };
        app.run("pixel.font.set", p);
    }
    let Some(f) = current else { return };
    let set = |app: &mut FigApp, key: &str, v: Value| {
        app.run("pixel.font.set", json!({"id": id.0, key: v}));
    };
    let col = widgets::col_width(ui.available_width());
    crate::right::row(ui, 32.0, |ui| {
        if let Some(name) = widgets::text_field_named(ui, Id::unique(("graybox_pixel_font_name", id.0)), &f.name, col * 2.0, "Font name") {
            set(app, "name", json!(name));
        }
    });
    let number = |ui: &mut Ui, key: &str, lead: &'static str, v: u32, min: f64| {
        widgets::number_field(ui, Id::unique(("graybox_pixel_font", key, id.0)), Lead::Text(lead), Some(f64::from(v)), "px", col, 1.0).and_then(|e| {
            match e {
                widgets::FieldEdit::Set(v) | widgets::FieldEdit::Scrub(v, true) => Some(v.round().clamp(min, f64::from(font::MAX_CELL))),
                _ => None,
            }
        })
    };
    let mut change: Option<(&str, f64)> = None;
    crate::right::row(ui, 32.0, |ui| {
        if let Some(v) = number(ui, "cellWidth", "W", f.cell_width, 1.0) {
            change = Some(("cellWidth", v));
        }
        if let Some(v) = number(ui, "cellHeight", "H", f.cell_height, 1.0) {
            change = Some(("cellHeight", v));
        }
    });
    crate::right::row(ui, 32.0, |ui| {
        if let Some(v) = number(ui, "baseline", "Base", f.baseline, 1.0) {
            change = Some(("baseline", v));
        }
        if let Some(v) = number(ui, "spacing", "Gap", f.spacing, 0.0) {
            change = Some(("spacing", v));
        }
    });
    if let Some((key, v)) = change {
        set(app, key, json!(v));
    }
    crate::right::row(ui, 32.0, |ui| {
        let id_chars = Id::unique(("graybox_pixel_font_chars", id.0));
        if let Some(chars) = widgets::text_field_named(ui, id_chars, &f.chars, col * 2.0, "Characters, cell by cell") {
            set(app, "chars", json!(chars));
        }
    });
    let drawn = store::frame_image(&n.plugin_data, sprite, 0).map_or(0, |c| font::glyphs(&c, &f).iter().filter(|g| !g.rects.is_empty()).count());
    let cells = f.chars.chars().count();
    if figcraft_engine::cmd::pixel::font::sheet_of(doc, &f.name) == Some(id) {
        widgets::note(ui, &format!("{drawn} of {cells} characters drawn. Text uses it as \"{}\"", f.name), t.text_secondary);
    } else {
        widgets::note(ui, &format!("A copy of the font \"{}\": rename it to make a font of its own", f.name), t.text_secondary);
    }
}

/// The Font row of the Pixel text section: the file's pixel fonts, picked for text layers `ids`.
pub(super) fn text_row(app: &mut FigApp, ui: &mut Ui, doc: &Document, ids: &[u64], family: Option<&str>, scale: Option<f64>) {
    let fonts = figcraft_engine::cmd::pixel::font::fonts(doc);
    if fonts.is_empty() {
        return;
    }
    let current = family.filter(|f| fonts.iter().any(|(n, _)| n.eq_ignore_ascii_case(f))).unwrap_or("Not a pixel font");
    let width = (ui.available_width() - widgets::PAD - widgets::PAD_RIGHT - 40.0).max(80.0);
    crate::right::row(ui, 32.0, |ui| {
        ui.label("Font");
        let resp = widgets::dropdown(ui, current, width, None).on_hover_text("Set the text in a pixel font drawn in this file");
        crate::right::menu_popup(&resp).show(|ui| {
            for (name, _) in &fonts {
                if crate::right::menu_item(ui, name, "") {
                    let mut p = json!({"ids": ids, "on": true, "font": name});
                    if let (Some(k), Some(o)) = (scale, p.as_object_mut()) {
                        o.insert("scale".into(), json!(k));
                    }
                    app.run("pixel.text", p);
                    ui.close();
                }
            }
        });
    });
}
