//! Per-file gameplay input and game event names. Game screens are plain prototype interactions.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::NodeId;

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
/// Inputs that move focus inside a navigable menu; their keys may also drive gameplay.
pub const MENU_ACTIONS: [&str; 6] = ["menu-up", "menu-down", "menu-left", "menu-right", "menu-confirm", "menu-back"];
pub const MAX_MENUS: usize = 4096;

/// What a disabled item (`State=Disabled`) does in a navigable menu.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DisabledItems {
    /// It takes focus but does not confirm.
    #[default]
    Focus,
    /// Focus passes over it.
    Skip,
}

/// How a menu item or hotbar slot shows a state (focused, selected, pressed, disabled).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum StateLook {
    /// Found by the variant names (hover, selected, focused, pressed, disabled…), else an outline.
    #[default]
    Auto,
    /// The variant whose `property` is `value` (the other properties kept).
    Variant { property: String, value: String },
    /// An orange outline, no variant.
    Outline,
}

impl StateLook {
    fn valid(&self) -> bool {
        match self {
            Self::Variant { property, value } => valid_alias(property) && valid_alias(value),
            _ => true,
        }
    }
}

/// One auto layout frame that is a navigable menu in Play mode.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MenuSettings {
    /// Past the last item, come back to the first.
    pub wrap: bool,
    /// The child focused first (default: the first visible child).
    pub start: Option<NodeId>,
    /// Coming back to the menu focuses the item focused last.
    pub remember: bool,
    pub disabled: DisabledItems,
    /// How the focused item looks.
    pub focus_look: StateLook,
    /// How it looks while Confirm is held.
    pub pressed_look: StateLook,
    /// Which items are disabled: those whose own variant has this value.
    pub disabled_look: StateLook,
}

impl Default for MenuSettings {
    fn default() -> Self {
        Self {
            wrap: false,
            start: None,
            remember: true,
            disabled: DisabledItems::Focus,
            focus_look: StateLook::Auto,
            pressed_look: StateLook::Auto,
            disabled_look: StateLook::Auto,
        }
    }
}

/// What Play shows and runs behind a screen while it is the one on top.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GameBackground {
    /// The game, playing (the default: a HUD).
    #[default]
    Running,
    /// The game, frozen; the mouse is free (an inventory or pause menu shown on its own).
    Paused,
    /// No game: the screen alone, the game frozen (a title or map screen).
    Hidden,
}

/// Which controller input zooms a map.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MapZoomInput {
    /// Right stick up zooms in, down zooms out.
    #[default]
    RightStick,
    /// RT / R2 zooms in, RB / R1 zooms out.
    Shoulders,
}

/// A frame whose content the player moves and zooms in Play (a map): drag and wheel with the
/// mouse, left stick and the chosen zoom input on a controller.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MapSettings {
    pub zoom_min: f64,
    pub zoom_max: f64,
    pub zoom_input: MapZoomInput,
}

impl Default for MapSettings {
    fn default() -> Self {
        Self { zoom_min: 0.5, zoom_max: 4.0, zoom_input: MapZoomInput::RightStick }
    }
}

/// A row of slots where one is selected (a hotbar): keys 1 to 9 pick a slot, the wheel and LB / RB
/// step through them, while the player keeps moving.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct HotbarSettings {
    /// Past the last slot, the wheel and LB / RB come back to the first.
    pub wrap: bool,
    /// How the selected slot looks.
    pub selected_look: StateLook,
}

impl Default for HotbarSettings {
    fn default() -> Self {
        Self { wrap: true, selected_look: StateLook::Auto }
    }
}

/// Game values a "game bar" can follow (health, ammo in the magazine, reserve ammo).
pub const BAR_VARIABLES: [&str; 3] = ["vida", "municao", "reserva"];
pub const MAX_SCREEN_SETTINGS: usize = 4096;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "StoredGameSettings")]
pub struct GameSettings {
    pub bindings: BTreeMap<String, Vec<String>>,
    pub event_names: BTreeMap<String, String>,
    pub web_url: String,
    /// Navigable menus: auto layout frame → its options.
    pub menus: BTreeMap<NodeId, MenuSettings>,
    /// Keys and buttons for [`MENU_ACTIONS`].
    pub menu_bindings: BTreeMap<String, Vec<String>>,
    /// Top-level frame → what Play shows behind it (missing: [`GameBackground::Running`]).
    pub backgrounds: BTreeMap<NodeId, GameBackground>,
    /// Layer → the game variable whose share of its maximum sets the layer's width in Play
    /// (a health bar drawn at any size empties from 100 % to 0 %).
    pub bars: BTreeMap<NodeId, String>,
    /// Frames whose content pans and zooms in Play.
    pub maps: BTreeMap<NodeId, MapSettings>,
    /// The designer's own named inputs (name → keys and buttons). Pressing one fires a game event
    /// of that name, for "Game event" triggers on the screen on top (T opens a watch, E picks up).
    pub inputs: BTreeMap<String, Vec<String>>,
    /// Auto layout frames that are hotbars.
    pub hotbars: BTreeMap<NodeId, HotbarSettings>,
    /// Font files (base64) of the font store families the file uses, added when it is shared so
    /// whoever opens it (in the browser too) sees its text as designed. Family → files.
    pub fonts: BTreeMap<String, Vec<String>>,
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
    menus: BTreeMap<NodeId, MenuSettings>,
    menu_bindings: BTreeMap<String, Vec<String>>,
    backgrounds: BTreeMap<NodeId, GameBackground>,
    bars: BTreeMap<NodeId, String>,
    maps: BTreeMap<NodeId, MapSettings>,
    inputs: BTreeMap<String, Vec<String>>,
    hotbars: BTreeMap<NodeId, HotbarSettings>,
    fonts: BTreeMap<String, Vec<String>>,
}

impl Default for StoredGameSettings {
    fn default() -> Self {
        let settings = GameSettings::default();
        Self {
            bindings: settings.bindings,
            screens: None,
            event_names: settings.event_names,
            web_url: settings.web_url,
            menus: settings.menus,
            menu_bindings: settings.menu_bindings,
            backgrounds: settings.backgrounds,
            bars: settings.bars,
            maps: settings.maps,
            inputs: settings.inputs,
            hotbars: settings.hotbars,
            fonts: settings.fonts,
        }
    }
}

impl TryFrom<StoredGameSettings> for GameSettings {
    type Error = String;

    fn try_from(mut stored: StoredGameSettings) -> Result<Self, Self::Error> {
        stored.bindings.retain(|action, _| !LEGACY_ACTIONS.contains(&action.as_str()));
        let settings = Self {
            bindings: stored.bindings,
            event_names: stored.event_names,
            web_url: stored.web_url,
            menus: stored.menus,
            menu_bindings: stored.menu_bindings,
            backgrounds: stored.backgrounds,
            bars: stored.bars,
            maps: stored.maps,
            inputs: stored.inputs,
            hotbars: stored.hotbars,
            fonts: stored.fonts,
        };
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
        Self {
            bindings,
            event_names: EVENT_NAMES.into_iter().map(|name| (name.into(), name.into())).collect(),
            web_url: String::new(),
            menus: BTreeMap::new(),
            menu_bindings: default_menu_bindings(),
            backgrounds: BTreeMap::new(),
            bars: BTreeMap::new(),
            maps: BTreeMap::new(),
            inputs: default_inputs(),
            hotbars: BTreeMap::new(),
            fonts: BTreeMap::new(),
        }
    }
}

/// Arrows, WASD and the D-pad move; Enter, Space or A confirms; Escape or B goes back.
pub fn default_menu_bindings() -> BTreeMap<String, Vec<String>> {
    [
        ("menu-up", &["ArrowUp", "W", "Gamepad Up"][..]),
        ("menu-down", &["ArrowDown", "S", "Gamepad Down"][..]),
        ("menu-left", &["ArrowLeft", "A", "Gamepad Left"][..]),
        ("menu-right", &["ArrowRight", "D", "Gamepad Right"][..]),
        ("menu-confirm", &["Enter", "Space", "Gamepad A"][..]),
        ("menu-back", &["Escape", "Gamepad B"][..]),
    ]
    .into_iter()
    .map(|(action, keys)| (action.to_string(), keys.iter().map(|key| (*key).to_string()).collect()))
    .collect()
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
        // Menu keys are unique among menu actions only: a menu owns input while it is active.
        let mut menu_used = BTreeMap::new();
        for (action, keys) in &self.menu_bindings {
            if !MENU_ACTIONS.contains(&action.as_str()) {
                return Err(format!("Unknown menu action `{action}`"));
            }
            validate_keys(keys)?;
            for key in keys {
                if let Some(previous) = menu_used.insert(key, action) {
                    return Err(format!("`{key}` is already bound to {previous}"));
                }
            }
        }
        if self.backgrounds.len() > MAX_SCREEN_SETTINGS || self.bars.len() > MAX_SCREEN_SETTINGS {
            return Err(format!("Use at most {MAX_SCREEN_SETTINGS} game backgrounds and game bars"));
        }
        validate_inputs(&self.inputs, &self.bindings)?;
        let looks_ok = self.hotbars.values().all(|h| h.selected_look.valid())
            && self.menus.values().all(|m| m.focus_look.valid() && m.pressed_look.valid() && m.disabled_look.valid());
        if !looks_ok {
            return Err("A state look names a variant property and value (1 to 80 characters)".into());
        }
        let font_bytes: usize = self.fonts.values().flatten().map(String::len).sum();
        if self.fonts.len() > MAX_SHARED_FONTS || font_bytes > MAX_SHARED_FONT_TEXT || self.fonts.keys().any(|f| !valid_alias(f)) {
            return Err(format!("Shared fonts: at most {MAX_SHARED_FONTS} families and {} MB", MAX_SHARED_FONT_TEXT / (1024 * 1024)));
        }
        if self.hotbars.len() > MAX_SCREEN_SETTINGS {
            return Err(format!("Use at most {MAX_SCREEN_SETTINGS} hotbars"));
        }
        if self.maps.len() > MAX_SCREEN_SETTINGS {
            return Err(format!("Use at most {MAX_SCREEN_SETTINGS} maps"));
        }
        for map in self.maps.values() {
            if !(map.zoom_min.is_finite()
                && map.zoom_max.is_finite()
                && 0.05 <= map.zoom_min
                && map.zoom_min <= 1.0
                && 1.0 <= map.zoom_max
                && map.zoom_max <= 20.0)
            {
                return Err("Map zoom: minimum 0.05 to 1, maximum 1 to 20".into());
            }
        }
        if self.bars.values().any(|v| !BAR_VARIABLES.contains(&v.as_str())) {
            return Err("A game bar follows vida, municao or reserva".into());
        }
        if self.menus.len() > MAX_MENUS {
            return Err(format!("Use at most {MAX_MENUS} navigable menus"));
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

#[cfg(test)]
#[path = "game_menu_tests.rs"]
mod menu_tests;

pub const MAX_INPUTS: usize = 32;
/// Shared fonts: families, and their files' total size as base64 text.
pub const MAX_SHARED_FONTS: usize = 32;
pub const MAX_SHARED_FONT_TEXT: usize = 48 * 1024 * 1024;

/// Named inputs every file starts with.
pub fn default_inputs() -> BTreeMap<String, Vec<String>> {
    BTreeMap::from([("pegar-item".to_string(), vec!["E".to_string(), "Gamepad Right".to_string()])])
}

/// A name for an input: like an event alias, and not one of the game's own events.
pub fn valid_input_name(name: &str) -> bool {
    valid_alias(name) && name.chars().count() <= 40 && !EVENT_NAMES.contains(&name)
}

/// Named inputs: valid names and keys, no key on two inputs or on a gameplay action.
pub fn validate_inputs(inputs: &BTreeMap<String, Vec<String>>, bindings: &BTreeMap<String, Vec<String>>) -> Result<(), String> {
    if inputs.len() > MAX_INPUTS {
        return Err(format!("Use at most {MAX_INPUTS} inputs"));
    }
    let mut used: BTreeMap<&str, &str> = BTreeMap::new();
    for (action, keys) in bindings {
        for key in keys {
            used.insert(key.as_str(), action.as_str());
        }
    }
    for (name, keys) in inputs {
        if !valid_input_name(name) {
            return Err(format!("`{}` can't name an input: 1 to 40 characters, not a game event", name.chars().take(40).collect::<String>()));
        }
        validate_keys(keys)?;
        for key in keys {
            if let Some(other) = used.insert(key.as_str(), name.as_str()) {
                return Err(format!("`{key}` is already used by {other}"));
            }
        }
    }
    Ok(())
}
