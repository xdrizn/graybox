//! Compact, command-driven per-file game settings, using the editor's existing tokens and fields.

use egui::{Id, Ui};
use figcraft_doc::game::{ACTIONS, EVENT_NAMES, ScreenConfig, ScreenRole};
use figcraft_doc::{Document, NodeId, NodeKind};
use serde_json::json;

use crate::right::{menu_check, menu_popup};
use crate::theme::Tokens;
use crate::{FigApp, widgets};

/// Panel content; the owning app supplies the window, so no second modal or UI state is needed.
pub fn show(app: &mut FigApp, ui: &mut Ui) {
    let Ok(ds) = app.session.ds() else {
        ui.label("Open a design file to configure game input and screens.");
        return;
    };
    let doc = ds.doc.clone();
    let uid = ds.uid;
    let selected = app.session.selection();
    let frames: Vec<NodeId> = selected.into_iter().filter(|id| doc.node(*id).is_some_and(|n| matches!(n.kind, NodeKind::Frame))).collect();
    let tokens = Tokens::get(ui.ctx());
    let controller = app.services.gamepad_state();
    let layout_id = Id::unique("game_binding_controller_layout");
    let mut playstation = ui.data(|d| d.get_temp::<bool>(layout_id)).unwrap_or(controller.playstation);
    ui.push_id(("game_settings", uid), |ui| {
        ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
        widgets::label(ui, "Saved in this design file. Changes can be undone.", tokens.text_secondary);
        ui.scope(|ui| {
            ui.add_space(8.0);
            ui.heading("Screen role");
            screen(app, ui, &doc, &frames);
            ui.add_space(16.0);
            ui.separator();
            ui.heading("Input bindings");
            widgets::label(ui, "Comma-separated input names. Clear a field to unbind it.", tokens.text_secondary);
            widgets::label(ui, "Examples: Space, MouseLeft, Gamepad A. Sticks move and look.", tokens.text_secondary);
            ui.horizontal(|ui| {
                widgets::label(ui, "Button labels", tokens.text_secondary);
                let response = widgets::dropdown(ui, if playstation { "PlayStation" } else { "Xbox" }, 140.0, None);
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
            egui::Grid::new("game_bindings").num_columns(2).spacing([16.0, 6.0]).show(ui, |ui| {
                for action in ACTIONS {
                    let current = doc.game.bindings.get(action).map(|keys| keys.join(", ")).unwrap_or_default();
                    widgets::label(ui, action_label(action), tokens.text);
                    ui.vertical(|ui| {
                        if let Some(value) = widgets::text_field_named(
                            ui,
                            Id::unique(("game_binding", uid, action)),
                            &current,
                            ui.available_width().clamp(180.0, 380.0),
                            action_label(action),
                        ) {
                            app.run("game.bind", json!({"action": action, "keys": split_keys(&value)}));
                        }
                        if let Some(keys) = doc.game.bindings.get(action) {
                            let labels: Vec<_> = keys.iter().filter_map(|key| controller_label(key, playstation)).collect();
                            if !labels.is_empty() {
                                widgets::label(ui, &labels.join(" / "), tokens.text_secondary);
                            }
                        }
                    });
                    ui.end_row();
                }
            });
            ui.horizontal(|ui| {
                if widgets::button(ui, "Reset bindings", false, true).clicked() {
                    app.run("game.bind", json!({"reset": true}));
                }
                if widgets::button(ui, "Set up Game variables", false, true).clicked() {
                    app.run("game.setup", json!({}));
                }
            });
            egui::CollapsingHeader::new("Controller button names").show(ui, |ui| {
                egui::Grid::new("game_controller_names").num_columns(2).show(ui, |ui| {
                    for name in figcraft_doc::proto::GAMEPAD_BUTTONS {
                        widgets::label(ui, name, tokens.text);
                        widgets::label(ui, &controller_label(name, playstation).unwrap_or_default(), tokens.text_secondary);
                        ui.end_row();
                    }
                });
            });
            ui.add_space(16.0);
            ui.separator();
            egui::CollapsingHeader::new("Game event names").default_open(false).show(ui, |ui| {
                widgets::label(ui, "Rename an event without changing the game state it represents.", tokens.text_secondary);
                egui::Grid::new("game_event_names").num_columns(2).spacing([16.0, 6.0]).show(ui, |ui| {
                    for canonical in EVENT_NAMES {
                        widgets::label(ui, canonical, tokens.text_secondary);
                        if let Some(name) = widgets::text_field_named(
                            ui,
                            Id::unique(("game_event", uid, canonical)),
                            doc.game.event_name(canonical),
                            ui.available_width().clamp(180.0, 380.0),
                            canonical,
                        ) {
                            app.run("game.event.rename", json!({"event": canonical, "name": name}));
                        }
                        ui.end_row();
                    }
                });
            });
            ui.add_space(12.0);
            if widgets::button(ui, "Add wireframe starter kit", true, true).clicked() {
                app.run("game.starterKit", json!({}));
            }
            widgets::label(ui, "Adds a bound HUD, Inventory, Pause, Map and Death screens, plus editable components.", tokens.text_secondary);
        });
    });
}

fn screen(app: &mut FigApp, ui: &mut Ui, doc: &Document, ids: &[NodeId]) {
    let tokens = Tokens::get(ui.ctx());
    let Some(id) = ids.first().copied() else {
        widgets::label(ui, "Select a frame, then choose Inventory or Pause to make it playable.", tokens.text_secondary);
        return;
    };
    let current = doc.game.screens.get(&id);
    let common = ids.iter().all(|id| doc.game.screens.get(id) == current);
    let caption = if !common { "Mixed roles" } else { current.map_or("Choose screen role", |c| c.role.label()) };
    ui.horizontal(|ui| {
        if let Some(node) = doc.node(id) {
            let label = if ids.len() == 1 { node.name.clone() } else { format!("{} selected frames", ids.len()) };
            widgets::label(ui, &label, tokens.text_secondary);
        }
        let response = widgets::dropdown(ui, caption, 180.0, None);
        menu_popup(&response).show(|ui| {
            for role in ScreenRole::ALL {
                if menu_check(ui, common && current.is_some_and(|c| c.role == role), role.label(), "") {
                    app.run("game.screen", json!({"ids": ids, "role": role}));
                    ui.close();
                }
            }
        });
    });
    if !common {
        widgets::label(ui, "Choose a role to apply its defaults to every selected frame.", tokens.text_secondary);
        return;
    }
    let Some(config) = current else { return };
    if config.role == ScreenRole::Death {
        widgets::label(ui, "Opens automatically when health reaches zero.", tokens.text_secondary);
    }
    if config.role == ScreenRole::Hud {
        widgets::label(ui, "Always visible while playing, unless another screen hides the HUD.", tokens.text_secondary);
    }
    ui.horizontal(|ui| {
        widgets::label(ui, "Open / close inputs", tokens.text_secondary);
        if let Some(value) =
            widgets::text_field_named(ui, Id::unique(("game_screen_keys", id.0)), &config.keys.join(", "), 280.0, "Open or close inputs")
        {
            app.run("game.screen", json!({"ids": ids, "keys": split_keys(&value)}));
        }
    });
    checkbox(app, ui, ids, config, "pause", "Pause simulation");
    checkbox(app, ui, ids, config, "release_cursor", "Release cursor");
    checkbox(app, ui, ids, config, "overlay", "Open as overlay");
    checkbox(app, ui, ids, config, "hud_visible", "Keep HUD visible");
    if config.role != ScreenRole::Hud {
        egui::CollapsingHeader::new("Visible HUD layers").show(ui, |ui| {
            let layers: Vec<NodeId> = doc
                .game
                .screens
                .iter()
                .filter(|(_, screen)| screen.role == ScreenRole::Hud)
                .filter_map(|(frame, _)| doc.node(*frame))
                .flat_map(|frame| frame.children.iter().copied())
                .collect();
            if layers.is_empty() {
                widgets::label(ui, "Assign a HUD frame to choose which of its layers remain visible.", tokens.text_secondary);
                return;
            }
            if widgets::button(ui, "Show all HUD layers", false, true).clicked() {
                app.run("game.screen", json!({"ids": ids, "hud_visible": true, "hudLayers": []}));
            }
            for layer in &layers {
                let Some(node) = doc.node(*layer) else { continue };
                let visible = config.hud_visible && (config.hud_layers.is_empty() || config.hud_layers.contains(layer));
                if widgets::checkbox(ui, visible, &node.name) {
                    let mut chosen = if !config.hud_visible {
                        Vec::new()
                    } else if config.hud_layers.is_empty() {
                        layers.clone()
                    } else {
                        config.hud_layers.clone()
                    };
                    if visible {
                        chosen.retain(|id| id != layer);
                    } else {
                        chosen.push(*layer);
                    }
                    app.run("game.screen", json!({"ids": ids, "hud_visible": !chosen.is_empty(), "hudLayers": chosen}));
                }
            }
        });
    }
    if widgets::button(ui, "Restore role defaults", false, true).clicked() {
        app.run("game.screen", json!({"ids": ids, "role": config.role}));
    }
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

fn checkbox(app: &mut FigApp, ui: &mut Ui, ids: &[NodeId], config: &ScreenConfig, key: &str, label: &str) {
    let value = match key {
        "pause" => config.pause,
        "release_cursor" => config.release_cursor,
        "overlay" => config.overlay,
        _ => config.hud_visible,
    };
    if widgets::checkbox(ui, value, label) {
        let mut params = json!({"ids": ids});
        params[key] = json!(!value);
        app.run("game.screen", params);
    }
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
        "inventory" => "Inventory",
        "pause" => "Pause",
        "map" => "Map",
        _ => action,
    }
}
