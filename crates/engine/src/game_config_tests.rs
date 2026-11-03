//! Behavioral coverage for persistent game input and the starter kit (plain frames, interactions and bound nodes).

use figcraft_doc::game::GameSettings;
use figcraft_doc::proto::{Action, Trigger};
use figcraft_doc::{Document, NodeKind, VariableValue};
use serde_json::{Value, json};

use crate::{
    Session,
    cmd::game::{ensure_variables, variable_ids},
    params::id_of,
};

fn ex(s: &mut Session, command: &str, params: Value) -> Value {
    s.execute(command, &params).unwrap()
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
fn files_with_early_screen_roles_and_menu_bindings_still_open() {
    let mut value = serde_json::to_value(GameSettings::default()).unwrap();
    value["screens"] = json!({"5": {"role": "inventory", "keys": ["Tab"]}});
    value["bindings"]["inventory"] = json!(["Tab", "Gamepad View"]);
    let loaded: GameSettings = serde_json::from_value(value).unwrap();
    assert_eq!(loaded, GameSettings::default());
}

#[test]
fn native_archive_preserves_config_and_old_files_receive_defaults() {
    let mut s = Session::new();
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
fn starter_kit_screens_are_plain_frames_driven_by_interactions() {
    let mut s = Session::new();
    let before = s.doc().unwrap().clone();
    let kit = ex(&mut s, "game.starterKit", json!({}));
    let ids = variable_ids(s.doc().unwrap());
    let screen = |name: &str| id_of(&kit[name]).unwrap();
    for name in ["hud", "inventory", "pause", "map", "death"] {
        assert!(matches!(s.doc().unwrap().node(screen(name)).unwrap().kind, NodeKind::Frame));
    }
    let doc = s.doc().unwrap();
    let opens = |from: &str, trigger: Trigger, to: &str| {
        doc.node(screen(from)).unwrap().interactions.iter().any(|it| {
            it.trigger == trigger
                && matches!(it.actions.first(), Some(Action::OpenOverlay { dest, .. }) if *dest == screen(to))
                && it.actions.contains(&Action::GamePause { paused: true })
        })
    };
    let key = |k: &str| Trigger::KeyPress { key: k.into() };
    assert!(opens("hud", key("Tab"), "inventory") && opens("hud", key("Gamepad View"), "inventory"));
    assert!(opens("hud", key("Escape"), "pause") && opens("hud", key("M"), "map"));
    assert!(opens("hud", Trigger::GameEvent { event: "vida-zero".into() }, "death"));
    let closes = |on: &str, trigger: Trigger| {
        doc.node(screen(on)).unwrap().interactions.iter().any(|it| it.trigger == trigger && it.actions == [Action::CloseOverlay])
    };
    assert!(closes("inventory", key("Tab")) && closes("pause", key("Gamepad Menu")) && closes("map", key("M")));
    assert!(closes("death", Trigger::GameEvent { event: "renasceu".into() }));
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

/// G6: a component carries its sounds, layer animations and game events into every instance, each
/// animation aimed at the instance's own copy of the layer.
#[test]
fn component_behaviour_follows_every_instance() {
    let mut s = Session::new();
    let page = s.page().unwrap();
    let button = id_of(&ex(&mut s, "frame.create", json!({"width": 200, "height": 48}))["id"]).unwrap();
    let glow = id_of(&ex(&mut s, "shape.rectangle", json!({"parent": button, "width": 200, "height": 4}))["id"]).unwrap();
    let main = id_of(&ex(&mut s, "component.create", json!({"ids": [button]}))["id"]).unwrap();
    let animate = |target| json!({"type": "playAnimation", "target": target, "preset": "pulse", "durationMs": 300, "easing": "easeOut"});
    ex(&mut s, "proto.interaction.add", json!({"id": main, "trigger": {"type": "mouseEnter"}, "actions": [animate(glow)]}));
    ex(&mut s, "proto.interaction.add", json!({"id": main, "trigger": {"type": "gameEvent", "event": "atirou"}, "actions": [animate(glow)]}));
    let hud = id_of(&ex(&mut s, "frame.create", json!({"x": 400, "width": 1280, "height": 720}))["id"]).unwrap();
    let copy = id_of(&ex(&mut s, "instance.create", json!({"component": main, "parent": hud}))["id"]).unwrap();
    let mut play = s.doc().unwrap().clone();
    crate::proto_run::play_interactions(&mut play);
    let copy_glow = *play.node(copy).unwrap().children.first().unwrap();
    assert_ne!(copy_glow, glow);
    let its = &play.node(copy).unwrap().interactions;
    assert_eq!(its.len(), 2, "the instance plays the component's interactions");
    assert_eq!(its[1].trigger, Trigger::GameEvent { event: "atirou".into() });
    for it in its {
        assert!(
            matches!(it.actions.first(), Some(Action::PlayAnimation { target, .. }) if *target == copy_glow),
            "animations aim at the instance's own layer: {:?}",
            it.actions
        );
    }
    assert!(
        matches!(play.node(glow).unwrap().interactions.as_slice(), []) && play.node(main).unwrap().interactions.len() == 2,
        "the main itself keeps aiming at its own layer"
    );
    // A layer inside the component with its own animation: each instance's copy aims at itself.
    ex(&mut s, "proto.interaction.add", json!({"id": glow, "trigger": {"type": "onClick"}, "actions": [animate(glow)]}));
    let mut play = s.doc().unwrap().clone();
    crate::proto_run::play_interactions(&mut play);
    let copy_glow = *play.node(copy).unwrap().children.first().unwrap();
    assert!(matches!(
        play.node(copy_glow).unwrap().interactions.first().and_then(|it| it.actions.first()),
        Some(Action::PlayAnimation { target, .. }) if *target == copy_glow
    ));
    let _ = page;
}

fn menu_scene(s: &mut Session) -> (figcraft_doc::NodeId, figcraft_doc::NodeId, figcraft_doc::NodeId) {
    let list = id_of(&ex(s, "frame.create", json!({"width": 200, "height": 200, "props": {"layout": "vertical", "gap": 8}}))["id"]).unwrap();
    let row = id_of(&ex(s, "shape.rectangle", json!({"parent": list, "width": 200, "height": 40}))["id"]).unwrap();
    let plain = id_of(&ex(s, "frame.create", json!({"x": 400, "width": 200, "height": 200}))["id"]).unwrap();
    (list, row, plain)
}

#[test]
fn menu_set_enables_updates_disables_and_undoes() {
    use figcraft_doc::game::{DisabledItems, MenuSettings};
    let mut s = Session::new();
    let (list, row, _) = menu_scene(&mut s);
    ex(&mut s, "game.menu.set", json!({"node": list, "enabled": true}));
    assert_eq!(s.doc().unwrap().game.menus[&list], MenuSettings::default());
    ex(&mut s, "game.menu.set", json!({"node": list, "wrap": true, "start": row, "remember": false, "disabled": "skip"}));
    let m = &s.doc().unwrap().game.menus[&list];
    assert!(m.wrap && !m.remember && m.start == Some(row) && m.disabled == DisabledItems::Skip);
    ex(&mut s, "game.menu.set", json!({"node": list, "start": null}));
    assert_eq!(s.doc().unwrap().game.menus[&list].start, None);
    ex(&mut s, "game.menu.set", json!({"node": list, "enabled": false}));
    assert!(s.doc().unwrap().game.menus.is_empty());
    ex(&mut s, "edit.undo", json!({}));
    assert!(s.doc().unwrap().game.menus.contains_key(&list));
}

#[test]
fn menu_set_refuses_frames_without_auto_layout_and_foreign_start() {
    let mut s = Session::new();
    let (list, _, plain) = menu_scene(&mut s);
    let before = s.doc().unwrap().game.clone();
    for params in [
        json!({"node": plain, "enabled": true}),
        json!({"node": list, "start": plain}),
        json!({"node": list, "disabled": "maybe"}),
        json!({"node": list, "wrap": "yes"}),
        json!({"node": 999_999, "enabled": true}),
        json!({}),
    ] {
        assert!(s.execute("game.menu.set", &params).is_err(), "{params}");
        assert_eq!(s.doc().unwrap().game, before);
    }
}

#[test]
fn menu_set_drops_menus_whose_frame_is_gone() {
    let mut s = Session::new();
    let (list, _, _) = menu_scene(&mut s);
    let other = id_of(&ex(&mut s, "frame.create", json!({"x": 800, "width": 100, "height": 100, "props": {"layout": "grid"}}))["id"]).unwrap();
    ex(&mut s, "game.menu.set", json!({"node": list, "enabled": true}));
    ex(&mut s, "edit.delete", json!({"ids": [list]}));
    ex(&mut s, "game.menu.set", json!({"node": other, "enabled": true}));
    let menus = &s.doc().unwrap().game.menus;
    assert!(menus.contains_key(&other) && !menus.contains_key(&list));
}

#[test]
fn bind_accepts_menu_actions_and_reset_restores_them() {
    let mut s = Session::new();
    let before = s.doc().unwrap().game.clone();
    ex(&mut s, "game.bind", json!({"action": "menu-confirm", "keys": ["E", "Gamepad A"]}));
    assert_eq!(s.doc().unwrap().game.menu_bindings["menu-confirm"], ["E", "Gamepad A"]);
    // Menu keys may also be gameplay keys, but not another menu action's.
    ex(&mut s, "game.bind", json!({"action": "menu-back", "keys": ["R"]}));
    assert!(s.execute("game.bind", &json!({"action": "menu-back", "keys": ["E"]})).is_err());
    ex(&mut s, "game.bind", json!({"reset": true}));
    assert_eq!(s.doc().unwrap().game.menu_bindings, before.menu_bindings);
}

#[test]
fn starter_kit_menus_are_auto_layout_with_state_variants() {
    use crate::components::{variant_options, variant_props};
    let mut s = Session::new();
    let kit = ex(&mut s, "game.starterKit", json!({}));
    let doc = s.doc().unwrap();
    let states = ["Default", "Focused", "Pressed", "Disabled"];
    // Button, Tab and Grid cell are sets with every State.
    let sets: Vec<_> = kit["sets"].as_array().unwrap().iter().map(|v| id_of(v).unwrap()).collect();
    assert_eq!(sets.len(), 3);
    for set in &sets {
        assert!(matches!(doc.node(*set).unwrap().kind, NodeKind::ComponentSet { .. }));
        let options = variant_options(doc, *set);
        assert_eq!(options, [("State".to_string(), states.iter().map(|s| s.to_string()).collect::<Vec<_>>())]);
    }
    let menus: Vec<_> = kit["menus"].as_array().unwrap().iter().map(|v| id_of(v).unwrap()).collect();
    assert_eq!(menus.len(), 4, "inventory slots, pause buttons, library list and grid");
    assert_eq!(doc.game.menus.keys().copied().collect::<std::collections::BTreeSet<_>>(), menus.iter().copied().collect());
    for menu in &menus {
        let node = doc.node(*menu).unwrap();
        assert!(node.layout.is_some(), "{} has auto layout", node.name);
        assert!(!node.children.is_empty());
    }
    // The inventory grid: 18 Grid cell instances, 6 per row, laid out by auto layout.
    let inventory = id_of(&kit["inventory"]).unwrap();
    let slots = *menus.iter().find(|m| doc.node(**m).unwrap().parent == Some(inventory)).unwrap();
    let slots = doc.node(slots).unwrap();
    let layout = slots.layout.as_ref().unwrap();
    assert_eq!((layout.mode, layout.grid_columns), (figcraft_doc::LayoutMode::Grid, 6));
    assert_eq!(slots.children.len(), 18);
    for c in &slots.children {
        let NodeKind::Instance { component, .. } = &doc.node(*c).unwrap().kind else { panic!("slot is not an instance") };
        let main = doc.node(*component).unwrap();
        assert_eq!(variant_props(&main.name)["State"], "Default");
        assert!(sets.contains(&main.parent.unwrap()));
    }
    let x = |i: usize| doc.node(slots.children[i]).unwrap().transform.as_coeffs()[4];
    assert!(x(1) > x(0) && x(6) == x(0), "the grid lays its cells out in rows of 6");
    // Every menu's items fit inside it (nothing is clipped).
    for menu in &menus {
        let m = doc.node(*menu).unwrap();
        for c in &m.children {
            let n = doc.node(*c).unwrap();
            let [.., cx, cy] = n.transform.as_coeffs();
            assert!(
                cx >= 0.0 && cy >= 0.0 && cx + n.width <= m.width + 0.01 && cy + n.height <= m.height + 0.01,
                "{} spills out of {}",
                n.name,
                m.name
            );
        }
    }
}
