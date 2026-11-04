//! Prototype tab sections for one selected layer: what Play shows behind a screen, and whether a
//! layer is a game bar.

use egui::Ui;
use figcraft_doc::game::GameBackground;
use figcraft_doc::{Document, Node};
use serde_json::json;

use crate::FigApp;
use crate::right::{menu_check, menu_popup};
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
            let response = widgets::dropdown(ui, label, ui.available_width(), None);
            menu_popup(&response).show(|ui| {
                for (value, wire, label) in BACKGROUNDS {
                    if menu_check(ui, current == value, label, "") {
                        app.run("game.background.set", json!({"node": id, "background": wire}));
                        ui.close();
                    }
                }
            });
            widgets::label(ui, "While this screen is on top in Play. Paused and No game free the mouse.", t.text_secondary);
        }
        widgets::separator(ui);
        widgets::section_header(ui, "Game bar", &[], false);
        let current = doc.game.bars.get(&id).map(String::as_str);
        let label = BARS.iter().find(|(v, _)| *v == current).map_or("None", |(_, l)| *l);
        let response = widgets::dropdown(ui, label, ui.available_width(), None);
        menu_popup(&response).show(|ui| {
            for (value, label) in BARS {
                if menu_check(ui, current == value, label, "") {
                    app.run("game.bar.set", json!({"node": id, "variable": value}));
                    ui.close();
                }
            }
        });
        widgets::label(
            ui,
            "In Play this layer's width follows the value: full at the maximum, empty at zero. Draw it at full size.",
            t.text_secondary,
        );
    });
}
