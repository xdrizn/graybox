//! Per-file gameplay input, event aliases and prototype screen roles.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::NodeId;

pub const ACTIONS: [&str; 14] =
    ["forward", "backward", "left", "right", "run", "jump", "crouch", "shoot", "reload", "heal", "respawn", "inventory", "pause", "map"];
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
    pub screens: BTreeMap<NodeId, ScreenConfig>,
    pub event_names: BTreeMap<String, String>,
    pub web_url: String,
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct StoredGameSettings {
    bindings: BTreeMap<String, Vec<String>>,
    screens: BTreeMap<NodeId, ScreenConfig>,
    event_names: BTreeMap<String, String>,
    web_url: String,
}

impl Default for StoredGameSettings {
    fn default() -> Self {
        let settings = GameSettings::default();
        Self { bindings: settings.bindings, screens: settings.screens, event_names: settings.event_names, web_url: settings.web_url }
    }
}

impl TryFrom<StoredGameSettings> for GameSettings {
    type Error = String;

    fn try_from(stored: StoredGameSettings) -> Result<Self, Self::Error> {
        let settings = Self { bindings: stored.bindings, screens: stored.screens, event_names: stored.event_names, web_url: stored.web_url };
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
            ("inventory", &["Tab", "Gamepad View"][..]),
            ("pause", &["Escape", "Gamepad Menu"][..]),
            ("map", &["M", "Gamepad Up"][..]),
        ]
        .into_iter()
        .map(|(action, keys)| (action.to_string(), keys.iter().map(|key| (*key).to_string()).collect()))
        .collect();
        Self {
            bindings,
            screens: BTreeMap::new(),
            event_names: EVENT_NAMES.into_iter().map(|name| (name.into(), name.into())).collect(),
            web_url: String::new(),
        }
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
        if self.screens.len() > 4096 || self.event_names.len() > EVENT_NAMES.len() {
            return Err("Too many game screens or event aliases".into());
        }
        if self.web_url.len() > 4096 || self.web_url.chars().any(char::is_control) {
            return Err("Game web URL must be at most 4096 bytes without control characters".into());
        }
        for screen in self.screens.values() {
            validate_keys(&screen.keys)?;
            if screen.hud_layers.len() > 4096 || screen.hud_layers.iter().collect::<BTreeSet<_>>().len() != screen.hud_layers.len() {
                return Err("HUD layers must be unique and at most 4096 entries".into());
            }
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ScreenRole {
    #[default]
    Hud,
    Inventory,
    Pause,
    Map,
    Death,
    Custom,
}

impl ScreenRole {
    pub const ALL: [Self; 6] = [Self::Hud, Self::Inventory, Self::Pause, Self::Map, Self::Death, Self::Custom];

    pub fn label(self) -> &'static str {
        match self {
            Self::Hud => "HUD",
            Self::Inventory => "Inventory",
            Self::Pause => "Pause",
            Self::Map => "Map",
            Self::Death => "Death",
            Self::Custom => "Custom",
        }
    }

    pub fn defaults(self) -> ScreenConfig {
        let keys: &[&str] = match self {
            Self::Inventory => &["Tab", "Gamepad View"],
            Self::Pause => &["Escape", "Gamepad Menu"],
            Self::Map => &["M", "Gamepad Up"],
            _ => &[],
        };
        ScreenConfig {
            role: self,
            keys: keys.iter().map(|key| (*key).to_string()).collect(),
            pause: matches!(self, Self::Inventory | Self::Pause | Self::Map | Self::Death),
            release_cursor: matches!(self, Self::Inventory | Self::Pause | Self::Map | Self::Death),
            overlay: self != Self::Hud,
            hud_visible: matches!(self, Self::Hud | Self::Inventory | Self::Map | Self::Custom),
            hud_layers: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(from = "StoredScreenConfig")]
pub struct ScreenConfig {
    pub role: ScreenRole,
    pub keys: Vec<String>,
    pub pause: bool,
    pub release_cursor: bool,
    pub overlay: bool,
    pub hud_visible: bool,
    /// Empty means every HUD layer when `hud_visible`; otherwise only these subtrees are visible.
    pub hud_layers: Vec<NodeId>,
}

impl Default for ScreenConfig {
    fn default() -> Self {
        ScreenRole::Hud.defaults()
    }
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct StoredScreenConfig {
    role: ScreenRole,
    keys: Option<Vec<String>>,
    pause: Option<bool>,
    release_cursor: Option<bool>,
    overlay: Option<bool>,
    hud_visible: Option<bool>,
    hud_layers: Vec<NodeId>,
}

impl From<StoredScreenConfig> for ScreenConfig {
    fn from(stored: StoredScreenConfig) -> Self {
        let mut config = stored.role.defaults();
        if let Some(keys) = stored.keys {
            config.keys = keys;
        }
        if let Some(value) = stored.pause {
            config.pause = value;
        }
        if let Some(value) = stored.release_cursor {
            config.release_cursor = value;
        }
        if let Some(value) = stored.overlay {
            config.overlay = value;
        }
        if let Some(value) = stored.hud_visible {
            config.hud_visible = value;
        }
        config.hud_layers = stored.hud_layers;
        config
    }
}
