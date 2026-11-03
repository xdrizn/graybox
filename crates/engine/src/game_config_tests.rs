//! Behavioral coverage for persistent game input, screen roles and bound starter-kit nodes.

use figcraft_doc::game::{GameSettings, ScreenConfig, ScreenRole};
use figcraft_doc::{Document, NodeId, NodeKind, VariableValue};
use serde_json::{Value, json};

use crate::{
    Session,
    cmd::game::{ensure_variables, variable_ids},
    params::id_of,
};

fn ex(s: &mut Session, command: &str, params: Value) -> Value {
    s.execute(command, &params).unwrap()
}
fn frame(s: &mut Session) -> NodeId {
    id_of(&ex(s, "frame.create", json!({"width": 1280, "height": 720}))["id"]).unwrap()
}

#[test]
fn bindings_are_valid_by_default_and_reject_conflicts_atomically() {
    let mut s = Session::new();
    let before = s.doc().unwrap().game.clone();
    before.validate().unwrap();
    for params in [
        json!({"action": "jump", "keys": ["W"]}),
        json!({"action": "jump", "keys": ["Space", "Space"]}),
        json!({"action": "jump", "keys": ["NotAKey"]}),
        json!({"action": "fly", "keys": ["J"]}),
        json!({"action": "jump", "keys": vec!["J"; 9]}),
        json!({"action": "jump", "keys": "J"}),
    ] {
        assert!(s.execute("game.bind", &params).is_err());
        assert_eq!(s.doc().unwrap().game, before);
    }
    ex(&mut s, "game.bind", json!({"action": "jump", "keys": ["J", "Gamepad A"]}));
    assert_eq!(s.doc().unwrap().game.bindings["jump"], ["J", "Gamepad A"]);
    ex(&mut s, "edit.undo", json!({}));
    assert_eq!(s.doc().unwrap().game, before);
    ex(&mut s, "edit.redo", json!({}));
    ex(&mut s, "game.bind", json!({"reset": true}));
    assert_eq!(s.doc().unwrap().game.bindings, before.bindings);
}

#[test]
fn role_defaults_overrides_and_hud_layer_selection_are_undoable() {
    let mut s = Session::new();
    let hud = frame(&mut s);
    ex(&mut s, "game.screen", json!({"id": hud, "role": "hud"}));
    let layer = id_of(&ex(&mut s, "shape.rectangle", json!({"parent": hud, "width": 80, "height": 20}))["id"]).unwrap();
    let menu = frame(&mut s);
    ex(&mut s, "game.screen", json!({"id": menu, "role": "inventory"}));
    assert_eq!(s.doc().unwrap().game.screens[&menu], ScreenRole::Inventory.defaults());
    ex(&mut s, "game.screen", json!({"id": menu, "pause": false, "release_cursor": false, "hudLayers": [layer]}));
    let config = &s.doc().unwrap().game.screens[&menu];
    assert!(!config.pause && !config.release_cursor && config.overlay && config.hud_visible);
    assert_eq!(config.hud_layers, vec![layer]);
    let before = s.doc().unwrap().game.clone();
    assert!(s.execute("game.screen", &json!({"id": menu, "hudLayers": [menu]})).is_err());
    assert!(s.execute("game.screen", &json!({"id": menu, "pause": "no"})).is_err());
    assert!(s.execute("game.screen", &json!({"id": layer, "role": "pause"})).is_err());
    assert_eq!(s.doc().unwrap().game, before);
    ex(&mut s, "edit.undo", json!({}));
    assert_eq!(s.doc().unwrap().game.screens[&menu], ScreenRole::Inventory.defaults());
    ex(&mut s, "game.screen", json!({"id": menu, "role": "pause"}));
    assert_eq!(s.doc().unwrap().game.screens[&menu], ScreenRole::Pause.defaults());
    let partial: ScreenConfig = serde_json::from_value(json!({"role": "death"})).unwrap();
    assert_eq!(partial, ScreenRole::Death.defaults());
}

#[test]
fn native_archive_preserves_config_and_old_files_receive_defaults() {
    let mut s = Session::new();
    let menu = frame(&mut s);
    ex(&mut s, "game.screen", json!({"id": menu, "role": "map", "hud_visible": false}));
    ex(&mut s, "game.bind", json!({"action": "heal", "keys": ["Z", "Gamepad Down"]}));
    ex(&mut s, "game.event.rename", json!({"event": "tomou-dano", "name": "damage-received"}));
    let bytes = figcraft_format::save_bytes(s.doc().unwrap(), None).unwrap();
    let (loaded, _) = figcraft_format::load_bytes(&bytes).unwrap();
    assert_eq!(loaded.game, s.doc().unwrap().game);
    assert_eq!(loaded.game.event_name("tomou-dano"), "damage-received");
    assert_eq!(loaded.game.event_name("custom-event"), "custom-event");
    let mut legacy = figcraft_format::to_json(&loaded).unwrap();
    legacy.as_object_mut().unwrap().remove("game");
    let (legacy, _) = figcraft_format::from_json(&legacy.to_string()).unwrap();
    assert_eq!(legacy.game, GameSettings::default());
    ex(&mut s, "edit.undo", json!({}));
    assert_eq!(s.doc().unwrap().game.event_name("tomou-dano"), "tomou-dano");
}

#[test]
fn invalid_persisted_config_and_aliases_return_errors() {
    let mut value = serde_json::to_value(GameSettings::default()).unwrap();
    value["bindings"]["jump"] = json!(["W"]);
    assert!(serde_json::from_value::<GameSettings>(value).is_err());
    let mut s = Session::new();
    let before = s.doc().unwrap().game.clone();
    for params in [
        json!({"event": "unknown", "name": "foo"}),
        json!({"event": "atirou", "name": "tomou-dano"}),
        json!({"event": "atirou", "name": ""}),
        json!({"event": "atirou", "name": "x".repeat(81)}),
    ] {
        assert!(s.execute("game.event.rename", &params).is_err());
        assert_eq!(s.doc().unwrap().game, before);
    }
}

#[test]
fn setup_is_idempotent_preserves_values_and_scopes_names_to_game() {
    let mut s = Session::new();
    ex(&mut s, "variable.createCollection", json!({"name": "Other"}));
    ex(&mut s, "variable.create", json!({"collection": "Other", "name": "vida", "type": "string", "value": "user data"}));
    ex(&mut s, "game.setup", json!({}));
    assert_eq!(variable_ids(s.doc().unwrap()).len(), 9);
    let ids = variable_ids(s.doc().unwrap());
    ex(&mut s, "variable.set", json!({"variable": ids["vida"], "value": 42}));
    let variables = s.doc().unwrap().variables.clone();
    ex(&mut s, "game.setup", json!({}));
    assert_eq!(s.doc().unwrap().variables, variables);
    ensure_variables(s.doc_mut().unwrap());
    assert_eq!(s.doc().unwrap().variables, variables);
    let game_health = s.doc().unwrap().variables.variables.iter().find(|v| v.id == ids["vida"]).unwrap();
    assert_eq!(game_health.values.values().next(), Some(&VariableValue::Number(42.0)));
}

#[test]
fn misbound_game_variables_are_not_retyped_or_published() {
    let mut s = Session::new();
    ex(&mut s, "variable.createCollection", json!({"name": "Game"}));
    ex(&mut s, "variable.create", json!({"collection": "Game", "name": "vida", "type": "string", "value": "do not overwrite"}));
    let before = s.doc().unwrap().clone();
    assert!(s.execute("game.setup", &json!({})).is_err());
    assert!(s.execute("game.starterKit", &json!({})).is_err());
    assert_eq!(*s.doc().unwrap(), before);
    ensure_variables(s.doc_mut().unwrap());
    assert!(!variable_ids(s.doc().unwrap()).contains_key("vida"));
    let health = s.doc().unwrap().variables.variables.iter().find(|v| v.name == "vida").unwrap();
    assert_eq!(health.values.values().next(), Some(&VariableValue::String("do not overwrite".into())));
}

#[test]
fn starter_kit_has_real_roles_components_and_live_variable_bindings() {
    let mut s = Session::new();
    let before = s.doc().unwrap().clone();
    let kit = ex(&mut s, "game.starterKit", json!({}));
    let ids = variable_ids(s.doc().unwrap());
    for (name, role) in [
        ("hud", ScreenRole::Hud),
        ("inventory", ScreenRole::Inventory),
        ("pause", ScreenRole::Pause),
        ("map", ScreenRole::Map),
        ("death", ScreenRole::Death),
    ] {
        let id = id_of(&kit[name]).unwrap();
        assert!(matches!(s.doc().unwrap().node(id).unwrap().kind, NodeKind::Frame));
        assert_eq!(s.doc().unwrap().game.screens[&id], role.defaults());
    }
    for id in kit["components"].as_array().unwrap() {
        let node = s.doc().unwrap().node(id_of(id).unwrap()).unwrap();
        assert!(matches!(node.kind, NodeKind::Component { .. }));
        assert!(!node.children.is_empty());
    }
    let doc = s.doc().unwrap();
    let health = doc.nodes().find(|n| n.bound.get("width") == ids.get("vida")).unwrap().id;
    let ammo = doc.nodes().find(|n| n.bound.get("width") == ids.get("municao")).unwrap().id;
    assert!(doc.nodes().any(|n| n.bound.get("characters") == ids.get("vida")));
    ex(&mut s, "variable.set", json!({"variable": ids["vida"], "value": 37}));
    ex(&mut s, "variable.set", json!({"variable": ids["municao"], "value": 5}));
    assert_eq!(s.doc().unwrap().node(health).unwrap().width, 37.0);
    assert_eq!(s.doc().unwrap().node(ammo).unwrap().width, 5.0);
    let bytes = figcraft_format::save_bytes(s.doc().unwrap(), None).unwrap();
    let (loaded, _) = figcraft_format::load_bytes(&bytes).unwrap();
    assert_eq!(loaded.game, s.doc().unwrap().game);
    ex(&mut s, "edit.undo", json!({}));
    ex(&mut s, "edit.undo", json!({}));
    ex(&mut s, "edit.undo", json!({}));
    assert_eq!(*s.doc().unwrap(), before);
}

#[test]
fn helper_only_adds_missing_defaults_to_plain_documents() {
    let mut doc = Document::new("Game config");
    ensure_variables(&mut doc);
    let before = doc.clone();
    ensure_variables(&mut doc);
    assert_eq!(doc, before);
}
