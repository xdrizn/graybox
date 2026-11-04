use super::{DisabledItems, GameSettings, MAX_MENUS, MENU_ACTIONS, MenuSettings};
use crate::NodeId;

#[test]
fn defaults_validate_and_menu_keys_may_overlap_gameplay_keys() {
    let s = GameSettings::default();
    s.validate().unwrap();
    assert_eq!(s.menu_bindings.len(), MENU_ACTIONS.len());
    assert_eq!(s.menu_bindings["menu-up"], ["ArrowUp", "W", "Gamepad Up"]);
    assert_eq!(s.menu_bindings["menu-down"], ["ArrowDown", "S", "Gamepad Down"]);
    assert_eq!(s.menu_bindings["menu-left"], ["ArrowLeft", "A", "Gamepad Left"]);
    assert_eq!(s.menu_bindings["menu-right"], ["ArrowRight", "D", "Gamepad Right"]);
    assert_eq!(s.menu_bindings["menu-confirm"], ["Enter", "Space", "Gamepad A"]);
    assert_eq!(s.menu_bindings["menu-back"], ["Escape", "Gamepad B"]);
    // W moves the player forward and the menu up.
    assert!(s.bindings["forward"].contains(&"W".to_string()));
    assert!(s.menus.is_empty());
    let m = MenuSettings::default();
    assert!(!m.wrap && m.remember && m.start.is_none() && m.disabled == DisabledItems::Focus);
}

#[test]
fn menu_binding_conflict_within_menu_actions_is_rejected() {
    let mut s = GameSettings::default();
    s.menu_bindings.insert("menu-back".into(), vec!["Enter".into()]);
    assert!(s.validate().is_err());
    let mut s = GameSettings::default();
    s.menu_bindings.insert("menu-fly".into(), vec!["J".into()]);
    assert!(s.validate().is_err());
    let mut s = GameSettings::default();
    s.menu_bindings.insert("menu-back".into(), vec!["NotAKey".into()]);
    assert!(s.validate().is_err());
}

#[test]
fn settings_round_trip_with_menus() {
    let mut s = GameSettings::default();
    s.menus.insert(NodeId(42), MenuSettings { wrap: true, start: Some(NodeId(7)), remember: false, disabled: DisabledItems::Skip, ..Default::default() });
    s.menus.insert(NodeId(3), MenuSettings::default());
    let json = serde_json::to_string(&s).unwrap();
    let back: GameSettings = serde_json::from_str(&json).unwrap();
    assert_eq!(back, s);
}

#[test]
fn file_without_menus_loads_with_defaults() {
    let json = r#"{"bindings":{"jump":["Space"]},"event_names":{},"web_url":""}"#;
    let s: GameSettings = serde_json::from_str(json).unwrap();
    assert!(s.menus.is_empty());
    assert_eq!(s.menu_bindings, GameSettings::default().menu_bindings);
    let partial: MenuSettings = serde_json::from_str(r#"{"wrap":true}"#).unwrap();
    assert!(partial.wrap && partial.remember);
}

#[test]
fn too_many_menus_is_rejected() {
    let mut s = GameSettings::default();
    for i in 0..=MAX_MENUS as u64 {
        s.menus.insert(NodeId(i), MenuSettings::default());
    }
    assert!(s.validate().is_err());
    let json = serde_json::to_string(&s).unwrap();
    assert!(serde_json::from_str::<GameSettings>(&json).is_err());
}

#[test]
fn state_looks_round_trip_and_reject_junk_names() {
    use crate::game::{HotbarSettings, StateLook};
    let mut s = GameSettings::default();
    let look = StateLook::Variant { property: "Property 1".into(), value: "hover".into() };
    s.hotbars.insert(NodeId(3), HotbarSettings { wrap: true, selected_look: look.clone() });
    s.menus.insert(NodeId(4), MenuSettings { focus_look: StateLook::Outline, ..Default::default() });
    let json = serde_json::to_value(&s).unwrap();
    assert_eq!(json["hotbars"]["3"]["selected_look"], serde_json::json!({"kind": "variant", "property": "Property 1", "value": "hover"}));
    assert_eq!(serde_json::from_value::<GameSettings>(json).unwrap(), s);
    let mut bad = serde_json::to_value(&s).unwrap();
    bad["hotbars"]["3"]["selected_look"]["value"] = serde_json::json!("");
    assert!(serde_json::from_value::<GameSettings>(bad).is_err());
}
