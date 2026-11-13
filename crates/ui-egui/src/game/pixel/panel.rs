//! The Pixel art section of the Design panel, for one selected pixel layer: Edit pixels (Pixel
//! mode), the art size and the scale. While the sprite is edited it also shows the sprite's layers
//! (pick the one drawn on, show or hide, opacity, add, delete, reorder, merge down) and the palette
//! (import a palette file as colour styles, save the sprite's colours as one).

use egui::{Align2, CornerRadius, Id, Rect, Sense, Ui, vec2};
use figcraft_doc::{Document, NodeId};
use figcraft_pixel::{Sprite, store};
use serde_json::json;

use super::{Mode, current_layer};
use crate::FigApp;
use crate::icons::Icon;
use crate::theme::Tokens;
use crate::widgets::{self, Lead};

/// Draws the section when exactly one pixel layer is selected. Returns whether it drew.
pub(crate) fn section(app: &mut FigApp, ui: &mut Ui, doc: &Document, sel: &[NodeId]) -> bool {
    if text_section(app, ui, doc, sel) {
        return true;
    }
    let [id] = sel else { return false };
    let Some(n) = doc.node(*id) else { return false };
    // An instance shows its main component's pixels: they are drawn and set there.
    if matches!(n.kind, figcraft_doc::NodeKind::Instance { .. }) {
        if figcraft_render::pixel::source(doc, n).is_some_and(|m| m.id != *id) {
            widgets::section_header(ui, "Pixel art", &[], false);
            widgets::note(ui, "From its main component: edit the main to change its pixels", Tokens::get(ui.ctx()).text_secondary);
            return true;
        }
        return false;
    }
    let Some(sprite) = store::load(&n.plugin_data) else { return false };
    let editing = app.pixel.mode == Mode::Edit(*id);
    // The section folds away from its header (it is big): the chevron or a click on the title.
    let closed = app.pixel.panel_closed;
    let fold = if closed { (Icon::ChevronRight, "Show the Pixel art settings") } else { (Icon::Chevron, "Hide the Pixel art settings") };
    let mut buttons = vec![fold];
    if !super::active(app) {
        buttons.push((Icon::PixelArt, "Edit pixels (double-click)"));
    }
    match widgets::section_header_resp(ui, "Pixel art", &buttons, false) {
        (Some(0), _) => app.pixel.panel_closed = !closed,
        (Some(_), _) => super::enter(app, *id),
        (None, resp) if resp.clicked() => app.pixel.panel_closed = !closed,
        _ => {}
    }
    if closed {
        return true;
    }
    let col = widgets::col_width(ui.available_width());
    crate::right::row(ui, 32.0, |ui| {
        let edit =
            widgets::number_field(ui, Id::unique(("graybox_pixel_scale", id.0)), Lead::Text("Scale"), Some(f64::from(sprite.scale)), "×", col, 1.0);
        if let Some(widgets::FieldEdit::Set(v) | widgets::FieldEdit::Scrub(v, true)) = edit {
            app.run("pixel.set", json!({"id": id.0, "scale": v.round().clamp(1.0, 64.0)}));
        }
    });
    let t = Tokens::get(ui.ctx());
    let frames = sprite.frames.len();
    let more = if frames > 1 { format!(", {frames} frames") } else { String::new() };
    widgets::note(ui, &format!("{} × {} art pixels{more}", sprite.width, sprite.height), t.text_secondary);
    if frames > 1 {
        plays_row(app, ui, *id, &sprite);
    }
    resize_rows(app, ui, doc, *id, &sprite, &t);
    super::font::section(app, ui, doc, *id, &sprite, &t);
    super::export::section(app, ui, doc, *id, &sprite, &t);
    if editing {
        layers(app, ui, *id, &sprite, &t);
        palette(app, ui, *id, &t);
    }
    true
}

/// What the layer plays in Prototype and Play (G16.4): every frame or one tag. A hover or pressed
/// variant whose sprite plays another tag switches what plays with the prototype's own triggers.
fn plays_row(app: &mut FigApp, ui: &mut Ui, id: NodeId, sprite: &figcraft_pixel::Sprite) {
    const ALL: &str = "All frames";
    let current = sprite.play.clone().unwrap_or_else(|| ALL.to_string());
    let width = widgets::col_width(ui.available_width()) * 2.0;
    crate::right::row(ui, 32.0, |ui| {
        ui.label("Plays");
        let resp = widgets::dropdown(ui, &current, width - 48.0, None).on_hover_text("What the layer plays in Prototype and Play");
        crate::right::menu_popup(&resp).show(|ui| {
            for name in std::iter::once(ALL).chain(sprite.tags.iter().map(|t| t.name.as_str())) {
                if crate::right::menu_item(ui, name, "") {
                    let play = if name == ALL { "" } else { name };
                    app.run("pixel.set", json!({"id": id.0, "play": play}));
                    ui.close();
                }
            }
        });
    });
}

/// The Pixel text section when only text layers are selected (G16.3): on, the text draws with no
/// anti-aliasing, its glyphs on the art-pixel grid at the scale shown.
fn text_section(app: &mut FigApp, ui: &mut Ui, doc: &Document, sel: &[NodeId]) -> bool {
    use figcraft_render::pixel::{TEXT_KEY, text_scale};
    let texts: Vec<&figcraft_doc::Node> = sel.iter().filter_map(|i| doc.node(*i)).collect();
    if texts.is_empty() || texts.len() != sel.len() || !texts.iter().all(|n| matches!(n.kind, figcraft_doc::NodeKind::Text(_))) {
        return false;
    }
    let ids: Vec<u64> = sel.iter().map(|i| i.0).collect();
    let scale = texts.iter().find_map(|n| text_scale(n));
    let on = texts.iter().all(|n| n.plugin_data.contains_key(TEXT_KEY));
    let buttons: &[(Icon, &str)] =
        if on { &[(Icon::Minus, "Draw as smooth text")] } else { &[(Icon::Plus, "Pixel text: sharp glyphs on the art-pixel grid")] };
    if widgets::section_header(ui, "Pixel text", buttons, !on).is_some() {
        app.run("pixel.text", json!({"ids": ids, "on": !on}));
    }
    if let (true, Some(k)) = (on, scale) {
        let col = widgets::col_width(ui.available_width());
        crate::right::row(ui, 32.0, |ui| {
            let edit = widgets::number_field(
                ui,
                Id::unique(("graybox_pixel_text_scale", sel.first().map_or(0, |i| i.0))),
                Lead::Text("Pixel"),
                Some(k),
                "×",
                col,
                1.0,
            );
            if let Some(widgets::FieldEdit::Set(v) | widgets::FieldEdit::Scrub(v, true)) = edit {
                app.run("pixel.text", json!({"ids": ids, "on": true, "scale": v.round().clamp(1.0, 64.0)}));
            }
        });
    }
    let family = texts.first().and_then(|n| match &n.kind {
        figcraft_doc::NodeKind::Text(td) => Some(td.style.font_family.as_str()),
        _ => None,
    });
    super::font::text_row(app, ui, doc, &ids, family, scale);
    true
}

/// How the art regrows when the layer is resized (G16.3): without 9-slice the middle repeats, the
/// edges repeat outward or the sprite tiles; with 9-slice its edges and middle tile or stretch, and
/// on a frame with auto layout its cuts can be the padding.
fn resize_rows(app: &mut FigApp, ui: &mut Ui, doc: &Document, id: NodeId, sprite: &Sprite, t: &Tokens) {
    use figcraft_pixel::resize::{Resize, SliceFill};
    let Some(n) = doc.node(id) else { return };
    if figcraft_doc::nine_slice::NineSlice::of(n).is_some() {
        widgets::note(ui, "9-slice edges and middle", t.text_secondary);
        let at = usize::from(sprite.slice_fill == SliceFill::Stretch);
        padded(ui, |ui| {
            if let Some(i) = widgets::segmented_text(ui, &["Repeat", "Stretch"], at) {
                app.run("pixel.set", json!({"id": id.0, "sliceFill": (["tile", "stretch"].get(i).copied().unwrap_or("tile"))}));
            }
            if n.layout.is_some() && widgets::checkbox(ui, sprite.slice_padding, "Cuts as padding") {
                app.run("pixel.set", json!({"id": id.0, "slicePadding": !sprite.slice_padding}));
            }
        });
        return;
    }
    widgets::note(ui, "When resized", t.text_secondary);
    let at = match sprite.resize {
        Resize::Middle => 0,
        Resize::Edges => 1,
        Resize::Tile => 2,
    };
    padded(ui, |ui| {
        if let Some(i) = widgets::segmented_text(ui, &["Middle", "Edges", "Tile"], at) {
            app.run("pixel.set", json!({"id": id.0, "resize": (["middle", "edges", "tile"].get(i).copied().unwrap_or("middle"))}));
        }
    });
}

/// A small text button; true when clicked.
pub(super) fn text_button(ui: &mut Ui, label: &str, tip: &str, t: &Tokens, enabled: bool) -> bool {
    let font = crate::theme::regular(11.0);
    let w = ui.painter().layout_no_wrap(label.to_string(), font.clone(), t.text).size().x + 14.0;
    let (r, resp) = ui.allocate_exact_size(vec2(w, 22.0), if enabled { Sense::click() } else { Sense::hover() });
    let fill = if enabled && resp.hovered() { t.hover } else { t.field };
    ui.painter().rect_filled(r, CornerRadius::same(4), fill);
    ui.painter().text(r.center(), Align2::CENTER_CENTER, label, font, if enabled { t.text } else { t.text_tertiary });
    enabled && resp.on_hover_text(tip).clicked()
}

/// `add` inside the panel's content margins (16 in from the left, 8 from the right).
pub(super) fn padded<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let margin = egui::Margin { left: widgets::PAD as i8, right: widgets::PAD_RIGHT as i8, top: 2, bottom: 4 };
    egui::Frame::NONE.inner_margin(margin).show(ui, add).inner
}

/// The sprite's layers, top first.
fn layers(app: &mut FigApp, ui: &mut Ui, id: NodeId, sprite: &Sprite, t: &Tokens) {
    ui.add_space(6.0);
    if widgets::section_header(ui, "Sprite layers", &[(Icon::Plus, "New layer above")], false).is_some() {
        let above = current_layer(&app.pixel, sprite).and_then(|l| sprite.layer_index(l)).map_or(sprite.layers.len(), |i| i + 1);
        if let Some(r) = app.run("pixel.layer.add", json!({"id": id.0, "index": above}))
            && let Some(l) = r["layer"].as_u64().and_then(|l| u32::try_from(l).ok())
        {
            app.pixel.layer = Some(l);
        }
    }
    let current = current_layer(&app.pixel, sprite);
    padded(ui, |ui| layer_rows(app, ui, id, sprite, current, t));
    let Some(cur) = current else { return };
    let index = sprite.layer_index(cur).unwrap_or(0);
    let opacity = sprite.layers.get(index).map_or(255, |l| l.opacity);
    let col = widgets::col_width(ui.available_width());
    crate::right::row(ui, 32.0, |ui| {
        let edit = widgets::number_field(
            ui,
            Id::unique(("graybox_pixel_layer_opacity", id.0, cur)),
            Lead::Icon(Icon::Opacity, "Layer opacity"),
            Some((f64::from(opacity) / 2.55).round()),
            "%",
            col,
            1.0,
        );
        if let Some(widgets::FieldEdit::Set(v) | widgets::FieldEdit::Scrub(v, true)) = edit {
            app.run("pixel.layer.set", json!({"id": id.0, "layer": cur, "opacity": v.clamp(0.0, 100.0)}));
        }
    });
    padded(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
            let top = index + 1 >= sprite.layers.len();
            if text_button(ui, "Up", "Move the layer up", t, !top) {
                app.run("pixel.layer.move", json!({"id": id.0, "layer": cur, "index": index + 1}));
            }
            if text_button(ui, "Down", "Move the layer down", t, index > 0) {
                app.run("pixel.layer.move", json!({"id": id.0, "layer": cur, "index": index.saturating_sub(1)}));
            }
            if text_button(ui, "Merge down", "Merge the layer into the one below it", t, index > 0) {
                // Drawing goes on in the layer it merged into.
                let merged = app.run("pixel.layer.merge", json!({"id": id.0, "layer": cur}));
                app.pixel.layer = merged.and_then(|r| r["layer"].as_u64()).and_then(|l| u32::try_from(l).ok());
            }
            if text_button(ui, "Delete", "Delete the layer", t, sprite.layers.len() > 1) {
                app.run("pixel.layer.delete", json!({"id": id.0, "layer": cur}));
                app.pixel.layer = None;
            }
        });
    });
}

/// One row per sprite layer, top first: the eye shows or hides it, a click draws on it.
fn layer_rows(app: &mut FigApp, ui: &mut Ui, id: NodeId, sprite: &Sprite, current: Option<u32>, t: &Tokens) {
    for l in sprite.layers.iter().rev() {
        let w = ui.available_width();
        let (r, resp) = ui.allocate_exact_size(vec2(w, 26.0), Sense::click());
        let on = current == Some(l.id);
        if on {
            ui.painter().rect_filled(r, CornerRadius::same(4), t.accent.gamma_multiply(0.18));
        } else if resp.hovered() {
            ui.painter().rect_filled(r, CornerRadius::same(4), t.hover);
        }
        let eye = Rect::from_min_size(r.min + vec2(4.0, 3.0), vec2(20.0, 20.0));
        let eye_resp =
            ui.interact(eye, ui.scope_id().with(("graybox_pixel_eye", l.id)), Sense::click()).on_hover_text(if l.visible { "Hide" } else { "Show" });
        crate::icons::paint(
            ui.painter(),
            eye.shrink(2.0),
            if l.visible { Icon::Eye } else { Icon::EyeOff },
            if l.visible { t.icon } else { t.icon_secondary },
        );
        ui.painter().text(
            r.left_center() + vec2(30.0, 0.0),
            Align2::LEFT_CENTER,
            &l.name,
            crate::theme::regular(11.5),
            if l.visible { t.text } else { t.text_secondary },
        );
        let pct = (f64::from(l.opacity) / 2.55).round();
        if pct < 100.0 {
            ui.painter().text(
                r.right_center() - vec2(6.0, 0.0),
                Align2::RIGHT_CENTER,
                format!("{pct:.0}%"),
                crate::theme::regular(11.0),
                t.text_secondary,
            );
        }
        if eye_resp.clicked() {
            app.run("pixel.layer.set", json!({"id": id.0, "layer": l.id, "visible": !l.visible}));
        } else if resp.clicked() {
            app.pixel.layer = Some(l.id);
        }
    }
}

/// The palette (G16.6): colours come from Graybox's own colour system (the colour picker of the
/// tool colours offers the file's colour styles and variables), so here a palette file becomes
/// colour styles, and the sprite's colours can be saved as one.
fn palette(app: &mut FigApp, ui: &mut Ui, id: NodeId, t: &Tokens) {
    ui.add_space(6.0);
    widgets::section_header(ui, "Palette", &[], false);
    padded(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
            if text_button(ui, "Import…", "Make colour styles from a palette file (.gpl or .hex)", t, true) {
                load_palette(app);
            }
            if text_button(ui, "Save…", "Save the sprite's colours as a palette file (.gpl)", t, true) {
                save_palette(app, id);
            }
        })
    });
}

fn load_palette(app: &mut FigApp) {
    app.pick_file("Import palette", &["gpl", "hex", "txt"], true, move |app, file| {
        use base64::Engine as _;
        let text = if let Some(path) = file["path"].as_str() {
            std::fs::read_to_string(path).ok()
        } else {
            file["data"].as_str().and_then(|d| base64::engine::general_purpose::STANDARD.decode(d).ok()).and_then(|b| String::from_utf8(b).ok())
        };
        // The file's name (without the extension) names the styles' folder unless the file names it.
        let stem = file["name"]
            .as_str()
            .or_else(|| file["path"].as_str())
            .and_then(|p| std::path::Path::new(p).file_stem().and_then(|s| s.to_str()).map(str::to_string));
        match text {
            Some(text) if !figcraft_pixel::palette::parse(&text).is_empty() => {
                let mut p = json!({"text": text});
                if let (Some(stem), Some(o)) = (stem, p.as_object_mut()) {
                    o.insert("name".into(), json!(stem));
                }
                app.run("pixel.palette.import", p);
            }
            _ => app.show_error("That file has no palette colours in it.".to_string()),
        }
    });
}

fn save_palette(app: &mut FigApp, id: NodeId) {
    let Some(r) = app.run("pixel.palette.export", json!({"id": id.0, "format": "gpl"})) else { return };
    let text = r["text"].as_str().unwrap_or_default().to_string();
    if cfg!(target_arch = "wasm32") {
        use base64::Engine as _;
        app.download("palette.gpl", &json!({"data": base64::engine::general_purpose::STANDARD.encode(text.as_bytes())}));
        return;
    }
    app.pick_save("Save palette", "palette.gpl", &["gpl"], move |app, path| {
        if std::fs::write(&path, text).is_err() {
            app.show_error(format!("Couldn't save {}", path.display()));
        }
    });
}
