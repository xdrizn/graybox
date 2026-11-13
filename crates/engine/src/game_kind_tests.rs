//! Which game Play runs: the setting in the file, the command that changes it, old files.

use figcraft_doc::game::{GameKind, GameSettings};
use serde_json::{Value, json};

use crate::{
    Session,
    cmd::file::{file_bytes, load_bytes},
};

fn ex(s: &mut Session, command: &str, params: Value) -> Value {
    s.execute(command, &params).unwrap()
}

fn kind(s: &Session) -> GameKind {
    s.doc().unwrap().game.kind
}

#[test]
fn a_new_file_plays_the_first_person_game() {
    let s = Session::new();
    assert_eq!(kind(&s), GameKind::Fps);
}

#[test]
fn the_kind_is_set_by_command_and_undone_and_redone() {
    let mut s = Session::new();
    let result = ex(&mut s, "game.kind.set", json!({"kind": "thirdPerson"}));
    assert_eq!(result["kind"], "thirdPerson");
    assert_eq!(kind(&s), GameKind::ThirdPerson);
    ex(&mut s, "edit.undo", json!({}));
    assert_eq!(kind(&s), GameKind::Fps);
    ex(&mut s, "edit.redo", json!({}));
    assert_eq!(kind(&s), GameKind::ThirdPerson);
    // Spellings people use are understood too.
    ex(&mut s, "game.kind.set", json!({"kind": "FPS"}));
    assert_eq!(kind(&s), GameKind::Fps);
    ex(&mut s, "game.kind.set", json!({"kind": "third person"}));
    assert_eq!(kind(&s), GameKind::ThirdPerson);
}

#[test]
fn junk_kinds_are_refused_and_change_nothing() {
    let mut s = Session::new();
    ex(&mut s, "game.kind.set", json!({"kind": "thirdPerson"}));
    for params in [json!({}), json!({"kind": null}), json!({"kind": 3}), json!({"kind": "racing"}), json!({"kind": ["fps"]}), json!("fps")] {
        assert!(s.execute("game.kind.set", &params).is_err(), "{params}");
        assert_eq!(kind(&s), GameKind::ThirdPerson);
    }
}

#[test]
fn the_kind_survives_saving_and_opening_the_file() {
    let mut s = Session::new();
    ex(&mut s, "game.kind.set", json!({"kind": "thirdPerson"}));
    let bytes = file_bytes(s.ds().unwrap()).unwrap();
    let (doc, _) = load_bytes(&bytes, "third.figcraft").unwrap();
    assert_eq!(doc.game.kind, GameKind::ThirdPerson);
    // The first-person file keeps reading as first person.
    ex(&mut s, "game.kind.set", json!({"kind": "fps"}));
    let bytes = file_bytes(s.ds().unwrap()).unwrap();
    let (doc, _) = load_bytes(&bytes, "first.figcraft").unwrap();
    assert_eq!(doc.game.kind, GameKind::Fps);
}

#[test]
fn files_written_before_games_were_chosen_play_first_person_and_are_not_changed() {
    // No `kind` in the settings, as in every file saved before.
    let old = serde_json::to_value(GameSettings::default()).unwrap();
    assert!(old.get("kind").is_none(), "the default is not written, so old files keep their bytes");
    let settings: GameSettings = serde_json::from_value(old).unwrap();
    assert_eq!(settings.kind, GameKind::Fps);
    assert_eq!(settings, GameSettings::default());
    // The third-person choice is written and read back.
    let third = GameSettings { kind: GameKind::ThirdPerson, ..GameSettings::default() };
    let wire = serde_json::to_value(&third).unwrap();
    assert_eq!(wire["kind"], "thirdPerson");
    assert_eq!(serde_json::from_value::<GameSettings>(wire).unwrap(), third);
    // An unknown game name is a bad file, not a silent first-person one.
    let mut bad = serde_json::to_value(GameSettings::default()).unwrap();
    bad["kind"] = json!("racing");
    assert!(serde_json::from_value::<GameSettings>(bad).is_err());
}

#[test]
fn the_kind_names_parse_both_ways() {
    for kind in GameKind::ALL {
        assert_eq!(GameKind::parse(kind.id()), Some(kind));
        assert!(!kind.label().is_empty());
    }
    assert_eq!(GameKind::parse(""), None);
    assert_eq!(GameKind::parse("fps2"), None);
}

fn names_of(s: &Session) -> Vec<String> {
    crate::cmd::game::variable_ids(s.doc().unwrap()).into_keys().collect()
}

#[test]
fn the_third_person_game_adds_its_own_variables() {
    let mut s = Session::new();
    ex(&mut s, "game.setup", json!({}));
    let first = names_of(&s);
    assert!(first.contains(&"municao".to_string()) && !first.contains(&"stamina".to_string()));
    ex(&mut s, "game.kind.set", json!({"kind": "thirdPerson"}));
    ex(&mut s, "game.setup", json!({}));
    let third = names_of(&s);
    for name in ["vida", "vidaMax", "stamina", "staminaMax", "arma", "agachado", "escalando", "mirando", "atacante"] {
        assert!(third.contains(&name.to_string()), "{name} in {third:?}");
    }
    // The third-person hero starts with its sword on its back.
    let mut s = Session::new();
    ex(&mut s, "game.kind.set", json!({"kind": "thirdPerson"}));
    ex(&mut s, "game.setup", json!({}));
    let doc = s.doc().unwrap();
    let arma = doc.variables.variables.iter().find(|v| v.name == "arma").unwrap();
    assert_eq!(arma.values.values().next(), Some(&figcraft_doc::VariableValue::String("Sword".into())));
}

#[test]
fn a_game_bar_can_follow_stamina() {
    let mut s = Session::new();
    let bar = ex(&mut s, "shape.rectangle", json!({"width": 100, "height": 10}))["id"].clone();
    ex(&mut s, "game.bar.set", json!({"node": bar, "variable": "stamina"}));
    assert_eq!(s.doc().unwrap().game.bars.values().next().map(String::as_str), Some("stamina"));
    assert!(s.execute("game.bar.set", &json!({"node": bar, "variable": "mana"})).is_err());
}

#[test]
fn the_crosshair_can_be_hidden_saved_and_undone() {
    let mut s = Session::new();
    assert!(!s.doc().unwrap().game.hide_crosshair, "shown by default");
    assert_eq!(ex(&mut s, "game.crosshair.set", json!({"visible": false}))["visible"], false);
    assert!(s.doc().unwrap().game.hide_crosshair);
    let bytes = file_bytes(s.ds().unwrap()).unwrap();
    let (doc, _) = load_bytes(&bytes, "hidden.figcraft").unwrap();
    assert!(doc.game.hide_crosshair, "kept in the file");
    ex(&mut s, "edit.undo", json!({}));
    assert!(!s.doc().unwrap().game.hide_crosshair);
    for params in [json!({}), json!({"visible": "no"}), json!({"visible": 1})] {
        assert!(s.execute("game.crosshair.set", &params).is_err(), "{params}");
    }
}

#[test]
fn the_bow_aim_and_shoulder_have_default_keys_and_older_files_still_load() {
    let defaults = GameSettings::default();
    assert_eq!(defaults.bindings.get("aim").map(Vec::len), Some(2));
    assert!(defaults.bindings["shoulder"].contains(&"MouseExtra1".to_string()));
    assert!(defaults.validate().is_ok());
    let mut s = Session::new();
    ex(&mut s, "game.bind", json!({"action": "shoulder", "keys": ["Gamepad Left"]}));
    assert_eq!(s.doc().unwrap().game.bindings["shoulder"], ["Gamepad Left"]);
    // A key already on another action is refused.
    assert!(s.execute("game.bind", &json!({"action": "aim", "keys": ["W"]})).is_err());
}
