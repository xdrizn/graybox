//! Per-file gameplay input and game event names. Game screens are plain prototype interactions.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

pub const ACTIONS: [&str; 11] = ["forward", "backward", "left", "right", "run", "jump", "crouch", "shoot", "reload", "heal", "respawn"];
/// Menu bindings of early files. Opening a menu is a prototype interaction now, so they are dropped on load.
const LEGACY_ACTIONS: [&str; 3] = ["inventory", "pause", "map"];
pub const EVENT_NAMES: [&str; 13] = [
    "vida-mudou",
    "tomou-dano",
    "vida-zero",
    "renasceu",
    "atirou",
    "recarregou",
    "municao-zero",
    "curou",
    "status-iniciou",
    "status-terminou",
    "inimigo-morreu",
    "tela-abriu",
    "tela-fechou",
];
pub const MAX_KEYS: usize = 8;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "StoredGameSettings")]
pub struct GameSettings {
    pub bindings: BTreeMap<String, Vec<String>>,
    pub event_names: BTreeMap<String, String>,
    pub web_url: String,
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct StoredGameSettings {
    bindings: BTreeMap<String, Vec<String>>,
    /// Screen roles from early files: ignored, screens are plain prototype interactions now.
    #[allow(dead_code)]
    screens: Option<serde::de::IgnoredAny>,
    event_names: BTreeMap<String, String>,
    web_url: String,
}

impl Default for StoredGameSettings {
    fn default() -> Self {
        let settings = GameSettings::default();
        Self { bindings: settings.bindings, screens: None, event_names: settings.event_names, web_url: settings.web_url }
    }
}

impl TryFrom<StoredGameSettings> for GameSettings {
    type Error = String;

    fn try_from(mut stored: StoredGameSettings) -> Result<Self, Self::Error> {
        stored.bindings.retain(|action, _| !LEGACY_ACTIONS.contains(&action.as_str()));
        let settings = Self { bindings: stored.bindings, event_names: stored.event_names, web_url: stored.web_url };
        settings.validate()?;
        Ok(settings)
    }
}

impl Default for GameSettings {
    fn default() -> Self {
        let bindings = [
            ("forward", &["W", "ArrowUp"][..]),
            ("backward", &["S", "ArrowDown"][..]),
            ("left", &["A", "ArrowLeft"][..]),
            ("right", &["D", "ArrowRight"][..]),
            ("run", &["Shift", "Gamepad LS"][..]),
            ("jump", &["Space", "Gamepad A"][..]),
            ("crouch", &["C", "Gamepad B"][..]),
            ("shoot", &["MouseLeft", "Gamepad RT"][..]),
            ("reload", &["R", "Gamepad X"][..]),
            ("heal", &["H", "Gamepad Down"][..]),
            ("respawn", &["Enter", "Gamepad Y"][..]),
        ]
        .into_iter()
        .map(|(action, keys)| (action.to_string(), keys.iter().map(|key| (*key).to_string()).collect()))
        .collect();
        Self { bindings, event_names: EVENT_NAMES.into_iter().map(|name| (name.into(), name.into())).collect(), web_url: String::new() }
    }
}

impl GameSettings {
    pub fn event_name<'a>(&'a self, canonical: &'a str) -> &'a str {
        self.event_names.get(canonical).map(String::as_str).filter(|name| !name.is_empty()).unwrap_or(canonical)
    }

    /// Validate a complete configuration without changing it or silently stealing another action's key.
    pub fn validate(&self) -> Result<(), String> {
        let mut used = BTreeMap::new();
        for (action, keys) in &self.bindings {
            if !ACTIONS.contains(&action.as_str()) {
                return Err(format!("Unknown game action `{action}`"));
            }
            validate_keys(keys)?;
            for key in keys {
                if let Some(previous) = used.insert(key, action) {
                    return Err(format!("`{key}` is already bound to {previous}"));
                }
            }
        }
        if self.event_names.len() > EVENT_NAMES.len() {
            return Err("Too many game event aliases".into());
        }
        if self.web_url.len() > 4096 || self.web_url.chars().any(char::is_control) {
            return Err("Game web URL must be at most 4096 bytes without control characters".into());
        }
        let mut aliases = BTreeSet::new();
        for (canonical, alias) in &self.event_names {
            if !EVENT_NAMES.contains(&canonical.as_str()) || !valid_alias(alias) || !aliases.insert(alias.as_str()) {
                return Err("Event names must be known, non-empty, unique and at most 80 characters".into());
            }
        }
        // An alias cannot shadow a canonical event whose alias was omitted.
        for canonical in EVENT_NAMES {
            if !self.event_names.contains_key(canonical) && aliases.contains(canonical) {
                return Err(format!("Event alias conflicts with `{canonical}`"));
            }
        }
        Ok(())
    }
}

pub fn valid_alias(name: &str) -> bool {
    !name.trim().is_empty() && name == name.trim() && name.chars().count() <= 80 && !name.chars().any(char::is_control)
}

/// Names understood by the egui runtime, plus mouse and standard controller buttons.
pub fn valid_key(key: &str) -> bool {
    if key.len() == 1 && key.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit()) {
        return true;
    }
    if crate::proto::GAMEPAD_BUTTONS.contains(&key) {
        return true;
    }
    if let Some(function) = key.strip_prefix('F') {
        return !function.starts_with('0')
            && function.bytes().all(|b| b.is_ascii_digit())
            && function.parse::<u8>().is_ok_and(|n| (1..=35).contains(&n));
    }
    matches!(
        key,
        "ArrowDown"
            | "ArrowLeft"
            | "ArrowRight"
            | "ArrowUp"
            | "Escape"
            | "Tab"
            | "Backspace"
            | "Enter"
            | "Space"
            | "Insert"
            | "Delete"
            | "Home"
            | "End"
            | "PageUp"
            | "PageDown"
            | "Copy"
            | "Cut"
            | "Paste"
            | "Colon"
            | "Comma"
            | "Backslash"
            | "Slash"
            | "Pipe"
            | "Questionmark"
            | "OpenBracket"
            | "CloseBracket"
            | "Backtick"
            | "Minus"
            | "Period"
            | "Plus"
            | "Equals"
            | "Semicolon"
            | "Quote"
            | "Shift"
            | "Control"
            | "Alt"
            | "MouseLeft"
            | "MouseRight"
            | "MouseMiddle"
            | "MouseExtra1"
            | "MouseExtra2"
    )
}

pub fn validate_keys(keys: &[String]) -> Result<(), String> {
    if keys.len() > MAX_KEYS {
        return Err(format!("Use at most {MAX_KEYS} keys per action"));
    }
    let mut used = BTreeSet::new();
    for key in keys {
        if key.len() > 32 || !valid_key(key) {
            return Err(format!("Unknown input `{}`", key.chars().take(32).collect::<String>()));
        }
        if !used.insert(key) {
            return Err(format!("Duplicate input `{key}`"));
        }
    }
    Ok(())
}
