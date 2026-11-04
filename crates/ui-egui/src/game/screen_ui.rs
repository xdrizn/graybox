//! Prototype tab sections for one selected layer: what Play shows behind a screen, and whether a
//! layer is a game bar.

use egui::Ui;
use figcraft_doc::game::GameBackground;
use figcraft_doc::{Document, Node};
use serde_json::json;

use crate::FigApp;
use crate::right::{menu_check, menu_popup, row};
use crate::theme::Tokens;
use crate::widgets;

const BACKGROUNDS: [(GameBackground, &str, &str); 3] = [
    (GameBackground::Running, "running", "Game running"),
    (GameBackground::Paused, "paused", "Game paused"),
    (GameBackground::Hidden, "hidden", "No game"),
];
const BARS: [(Option<&str>, &str); 4] =
    [(None, "None"), (Some("vida"), "Health (vida)"), (Some("municao"), "Ammo (municao)"), (Some("reserva"), "Reserve (reserva)")];

pub fn section(app: &mut FigApp, ui: &mut Ui, doc: &Document, node: &Node) {
    let t = Tokens::get(ui.ctx());
    let id = node.id;
    ui.push_id(("greybox_screen", id), |ui| {
        ui.spacing_mut().item_spacing = egui::vec2(8.0, 6.0);
        if doc.top_level_frame(id) == Some(id) {
            widgets::separator(ui);
            widgets::section_header(ui, "Game behind this screen", &[], false);
            let current = doc.game.backgrounds.get(&id).copied().unwrap_or_default();
            let label = BACKGROUNDS.iter().find(|(b, _, _)| *b == current).map_or("Game running", |(_, _, l)| *l);
            row(ui, 32.0, |ui| {
                let response = widgets::dropdown(ui, label, ui.available_width() - widgets::PAD_RIGHT, None);
                menu_popup(&response).show(|ui| {
                    for (value, wire, label) in BACKGROUNDS {
                        if menu_check(ui, current == value, label, "") {
                            app.run("game.background.set", json!({"node": id, "background": wire}));
                            ui.close();
                        }
                    }
                });
            });
            widgets::note(ui, "While this screen is on top in Play. Paused and No game free the mouse.", t.text_secondary);
        }
        if node.kind.is_frame_like() {
            map_section(app, ui, doc, id);
        }
        widgets::separator(ui);
        widgets::section_header(ui, "Game bar", &[], false);
        let current = doc.game.bars.get(&id).map(String::as_str);
        let label = BARS.iter().find(|(v, _)| *v == current).map_or("None", |(_, l)| *l);
        row(ui, 32.0, |ui| {
            let response = widgets::dropdown(ui, label, ui.available_width() - widgets::PAD_RIGHT, None);
            menu_popup(&response).show(|ui| {
                for (value, label) in BARS {
                    if menu_check(ui, current == value, label, "") {
                        app.run("game.bar.set", json!({"node": id, "variable": value}));
                        ui.close();
                    }
                }
            });
        });
        widgets::note(
            ui,
            "In Play this layer's width follows the value: full at the maximum, empty at zero. Draw it at full size.",
            t.text_secondary,
        );
    });
}

const ZOOM_INPUTS: [(figcraft_doc::game::MapZoomInput, &str, &str); 2] = [
    (figcraft_doc::game::MapZoomInput::RightStick, "rightStick", "Right stick up / down"),
    (figcraft_doc::game::MapZoomInput::Shoulders, "shoulders", "R2 / RT in, R1 / RB out"),
];
const ZOOM_MIN: [f64; 3] = [0.25, 0.5, 1.0];
const ZOOM_MAX: [f64; 4] = [2.0, 4.0, 8.0, 16.0];

/// A frame whose content the player moves and zooms in Play.
fn map_section(app: &mut FigApp, ui: &mut Ui, doc: &Document, id: figcraft_doc::NodeId) {
    let t = Tokens::get(ui.ctx());
    widgets::separator(ui);
    widgets::section_header(ui, "Map", &[], false);
    let map = doc.game.maps.get(&id).copied();
    row(ui, 28.0, |ui| {
        if widgets::checkbox(ui, map.is_some(), "Pan and zoom in Play") {
            app.run("game.map.set", json!({"node": id, "enabled": map.is_none()}));
        }
    });
    let Some(map) = map else {
        widgets::note(ui, "Put a large image or drawing inside this frame: in Play it can be dragged and zoomed.", t.text_secondary);
        return;
    };
    let dropdown = |app: &mut FigApp, ui: &mut Ui, title: &str, shown: String, options: Vec<(String, bool, serde_json::Value)>| {
        row(ui, 32.0, |ui| {
            let (lr, _) = ui.allocate_exact_size(egui::vec2(72.0, widgets::FIELD_H), egui::Sense::hover());
            ui.painter().text(lr.left_center(), egui::Align2::LEFT_CENTER, title, crate::theme::regular(11.0), t.text_secondary);
            let response = widgets::dropdown(ui, &shown, ui.available_width() - widgets::PAD_RIGHT, None);
            menu_popup(&response).show(|ui| {
                for (label, on, fields) in options {
                    if menu_check(ui, on, &label, "") {
                        let mut params = json!({"node": id});
                        if let (Some(p), Some(f)) = (params.as_object_mut(), fields.as_object()) {
                            p.extend(f.clone());
                        }
                        app.run("game.map.set", params);
                        ui.close();
                    }
                }
            });
        });
    };
    let zoom_label = ZOOM_INPUTS.iter().find(|(z, _, _)| *z == map.zoom_input).map_or("", |(_, _, l)| *l).to_string();
    dropdown(
        app,
        ui,
        "Zoom",
        zoom_label,
        ZOOM_INPUTS.iter().map(|(z, wire, l)| ((*l).to_string(), *z == map.zoom_input, json!({"zoomInput": wire}))).collect(),
    );
    dropdown(
        app,
        ui,
        "Zoom out to",
        format!("{}×", map.zoom_min),
        ZOOM_MIN.iter().map(|v| (format!("{v}×"), *v == map.zoom_min, json!({"zoomMin": v}))).collect(),
    );
    dropdown(
        app,
        ui,
        "Zoom in to",
        format!("{}×", map.zoom_max),
        ZOOM_MAX.iter().map(|v| (format!("{v}×"), *v == map.zoom_max, json!({"zoomMax": v}))).collect(),
    );
    widgets::note(
        ui,
        "Mouse: drag to move, wheel to zoom. Controller: left stick moves. Works while the mouse is free (a map screen with the game paused).",
        t.text_secondary,
    );
}
