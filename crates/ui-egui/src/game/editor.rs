//! The file's game controls and event names, in the Prototype tab (nothing selected). Narrow
//! layout: every field takes the panel's width.

use egui::{Id, Ui};
use figcraft_doc::game::{ACTIONS, EVENT_NAMES, GameSettings, MENU_ACTIONS};
use serde_json::json;

use crate::right::{menu_check, menu_popup};
use crate::theme::Tokens;
use crate::{FigApp, widgets};

/// The default keys, built once (files made before an action existed play these).
static DEFAULT_BINDINGS: std::sync::LazyLock<std::collections::BTreeMap<String, Vec<String>>> =
    std::sync::LazyLock::new(|| GameSettings::default().bindings);

/// Controls (key and controller bindings for the test scene).
pub fn controls(app: &mut FigApp, ui: &mut Ui) {
    let Ok(ds) = app.session.ds() else { return };
    let doc = ds.doc.clone();
    let uid = ds.uid;
    let tokens = Tokens::get(ui.ctx());
    let controller = app.services.gamepad_state();
    let layout_id = Id::unique("game_binding_controller_layout");
    let mut playstation = ui.data(|d| d.get_temp::<bool>(layout_id)).unwrap_or(controller.playstation);
    ui.push_id(("game_controls", uid), |ui| {
        widgets::label(ui, "Keys and buttons for moving, aiming and shooting in Play. Separate inputs with commas.", tokens.text_secondary);
        ui.horizontal(|ui| {
            widgets::label(ui, "Controller", tokens.text_secondary);
            let response = widgets::dropdown(ui, if playstation { "PlayStation" } else { "Xbox" }, ui.available_width(), None);
            menu_popup(&response).show(|ui| {
                for (label, value) in [("Xbox", false), ("PlayStation", true)] {
                    if menu_check(ui, playstation == value, label, "") {
                        playstation = value;
                        ui.data_mut(|d| d.insert_temp(layout_id, value));
                        ui.close();
                    }
                }
            });
        });
        // Files made before an action existed play it on its default keys: show those.
        let defaults = &*DEFAULT_BINDINGS;
        let rows = ACTIONS.iter().map(|a| (*a, &doc.game.bindings)).chain(MENU_ACTIONS.iter().map(|a| (*a, &doc.game.menu_bindings)));
        for (action, bindings) in rows {
            if action == MENU_ACTIONS[0] {
                widgets::label(ui, "Menus (navigable menus in Play)", tokens.text_secondary);
            }
            let keys = bindings.get(action).or_else(|| defaults.get(action));
            let current = keys.map(|keys| keys.join(", ")).unwrap_or_default();
            widgets::label(ui, action_label(action), tokens.text);
            if let Some(value) =
                widgets::text_field_named(ui, Id::unique(("game_binding", uid, action)), &current, ui.available_width(), action_label(action))
            {
                app.run("game.bind", json!({"action": action, "keys": split_keys(&value)}));
            }
            if let Some(keys) = keys {
                let labels: Vec<_> = keys.iter().filter_map(|key| controller_label(key, playstation)).collect();
                if !labels.is_empty() {
                    widgets::label(ui, &labels.join(" / "), tokens.text_secondary);
                }
            }
        }
        if widgets::button(ui, "Reset controls", false, true).clicked() {
            app.run("game.bind", json!({"reset": true}));
        }
    });
}

/// The designer's named inputs: a name and its keys and buttons. In Play a press fires the "Game
/// event" of that name on the screen on top (T opens a watch screen, E shows a pick-up).
pub fn inputs(app: &mut FigApp, ui: &mut Ui) {
    let Ok(ds) = app.session.ds() else { return };
    let doc = ds.doc.clone();
    let uid = ds.uid;
    let tokens = Tokens::get(ui.ctx());
    ui.push_id(("game_inputs", uid), |ui| {
        widgets::label(
            ui,
            "Your own keys: pressing one in Play fires a Game event with its name. Use it as the trigger of any interaction.",
            tokens.text_secondary,
        );
        for (name, keys) in &doc.game.inputs {
            ui.horizontal(|ui| {
                let w = (ui.available_width() - 30.0).max(60.0);
                if let Some(new) = widgets::text_field_named(ui, Id::unique(("game_input_name", uid, name.as_str())), name, w, "Input name") {
                    app.run("game.input.set", json!({"name": name, "newName": new.trim()}));
                }
                if crate::icons::button(ui, crate::icons::Icon::Minus, 22.0, false, "Remove input").clicked() {
                    app.run("game.input.set", json!({"name": name, "keys": null}));
                }
            });
            if let Some(value) =
                widgets::text_field_named(ui, Id::unique(("game_input_keys", uid, name.as_str())), &keys.join(", "), ui.available_width(), "Keys")
            {
                app.run("game.input.set", json!({"name": name, "keys": split_keys(&value)}));
            }
        }
        if widgets::button(ui, "Add input", false, true).clicked() {
            let fresh = (1..).map(|n| format!("input-{n}")).find(|n| !doc.game.inputs.contains_key(n)).unwrap_or_default();
            app.run("game.input.set", json!({"name": fresh, "keys": []}));
        }
    });
}

/// Game event names as the Game event trigger lists them.
pub fn events(app: &mut FigApp, ui: &mut Ui) {
    let Ok(ds) = app.session.ds() else { return };
    let doc = ds.doc.clone();
    let uid = ds.uid;
    let tokens = Tokens::get(ui.ctx());
    ui.push_id(("game_events", uid), |ui| {
        widgets::label(ui, "Any layer can react to these with the Game event trigger. Rename them to your own words.", tokens.text_secondary);
        for canonical in EVENT_NAMES {
            widgets::label(ui, canonical, tokens.text_secondary);
            if let Some(name) = widgets::text_field_named(
                ui,
                Id::unique(("game_event", uid, canonical)),
                doc.game.event_name(canonical),
                ui.available_width(),
                canonical,
            ) {
                app.run("game.event.rename", json!({"event": canonical, "name": name}));
            }
        }
        if widgets::button(ui, "Set up Game variables", false, true).clicked() {
            app.run("game.setup", json!({}));
        }
        widgets::label(ui, "Health, ammo and more as variables in a Game collection, updated while you play.", tokens.text_secondary);
    });
}

/// The example screens.
pub fn examples(app: &mut FigApp, ui: &mut Ui) {
    let tokens = Tokens::get(ui.ctx());
    if widgets::button(ui, "Add example screens", false, true).clicked() {
        app.run("game.starterKit", json!({}));
    }
    widgets::label(
        ui,
        "A HUD, inventory, pause, map and death screen with wireframe components. Only examples: their keys and events are ordinary interactions you can change or copy.",
        tokens.text_secondary,
    );
}

fn controller_label(key: &str, playstation: bool) -> Option<String> {
    let index = figcraft_doc::proto::GAMEPAD_BUTTONS.iter().position(|name| *name == key)?;
    let name = if playstation {
        match index {
            0 => "Cross",
            1 => "Circle",
            2 => "Square",
            3 => "Triangle",
            4 => "L1",
            5 => "R1",
            6 => "L2",
            7 => "R2",
            8 => "Share",
            9 => "Options",
            10 => "L3",
            11 => "R3",
            12 => "D-pad Up",
            13 => "D-pad Down",
            14 => "D-pad Left",
            15 => "D-pad Right",
            _ => "PS",
        }
    } else {
        match index {
            10 => "Left stick press",
            11 => "Right stick press",
            12 => "D-pad Up",
            13 => "D-pad Down",
            14 => "D-pad Left",
            15 => "D-pad Right",
            _ => key.strip_prefix("Gamepad ").unwrap_or(key),
        }
    };
    Some(format!("{} {name}", if playstation { "PlayStation" } else { "Xbox" }))
}

fn split_keys(value: &str) -> Vec<String> {
    value.split(',').map(str::trim).filter(|key| !key.is_empty()).map(str::to_string).collect()
}

fn action_label(action: &str) -> &str {
    match action {
        "forward" => "Move forward",
        "backward" => "Move backward",
        "left" => "Strafe left",
        "right" => "Strafe right",
        "run" => "Run",
        "jump" => "Jump",
        "crouch" => "Crouch",
        "shoot" => "Shoot",
        "reload" => "Reload",
        "heal" => "Heal",
        "respawn" => "Respawn",
        "aim" => "Aim (third person, bow)",
        "shoulder" => "Swap shoulder (aiming)",
        "menu-up" => "Menu up",
        "menu-down" => "Menu down",
        "menu-left" => "Menu left",
        "menu-right" => "Menu right",
        "menu-confirm" => "Confirm",
        "menu-back" => "Back",
        _ => action,
    }
}
