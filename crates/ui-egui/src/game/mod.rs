//! Greybox Play mode: the platform input, GPU scene and game editor around the pure simulation.

use figcraft_game::Game;

pub(crate) mod alt_scrub;
pub(crate) mod anchor_ui;
pub mod animation;
pub mod audio;
pub mod brand;
pub(crate) mod brush_taper;
pub mod claude;
pub(crate) mod color_fold;
pub mod components;
pub(crate) mod corner_style_ui;
pub(crate) mod docs;
pub mod editor;
pub(crate) mod font_watch;
pub mod fonts;
pub mod frames;
pub(crate) mod gradient_turn;
pub mod hotbar;
pub(crate) mod input;
pub(crate) mod inspect_button;
pub(crate) mod layout_ease;
pub(crate) mod line_dims;
pub mod look_ui;
pub mod looks;
pub mod map;
pub mod menu;
pub(crate) mod nine_slice_nodes;
pub(crate) mod nine_slice_ui;
pub mod proto_targets;
pub(crate) mod pseudo_view;
pub mod renderer;
pub mod screen_fit;
pub mod screen_ui;
pub mod share;
pub(crate) mod style_fill;
// 3D rotation of layers (G17).
pub(crate) mod tilt_ui;
// Graybox: the third-person game (G11) and the picker of which game a file plays.
pub(crate) mod kind_ui;
pub(crate) mod tps;
// Graybox: mouse 4 and 5 are game buttons in the browser, not back and forward (G11.6).
mod browser_buttons;
pub(crate) mod unity_ui;
// Graybox: the tree list shared by the Docs tab and the Pages list, and the Pages list itself (G9).
pub(crate) mod page_list;
// Graybox: Pixel mode, editing a pixel layer's sprite on the canvas (G16).
pub(crate) mod pixel;
pub(crate) mod tree_list;

/// Platform hosts read these markers inside the browser's trusted input event, before egui's
// Graybox: the emoji picker of page and document icons (G9, G13).
pub(crate) mod emoji_picker;
#[cfg(test)]
mod page_icon_tests;
/// next frame. Fullscreen and pointer lock cannot reliably be requested from that later frame.
pub const PLAY_BUTTON_RECT: &str = "greybox.play.gesture.rect";
pub const PLAY_KEY_ENABLED: &str = "greybox.play.gesture.enabled";
pub const BROWSER_ERROR: &str = "greybox.browser.error";

#[cfg(test)]
mod tests;

/// A game being played: the simulation and the display time it last advanced to.
#[derive(Clone, Debug, Default)]
pub struct Play {
    pub(crate) game: Game,
    /// egui's input time at the last frame (seconds; None before the first).
    last: Option<f64>,
    pub(crate) captured: bool,
    pub(crate) entered: bool,
    pub(crate) was_fullscreen: bool,
    pub(crate) applied_capture: Option<bool>,
    pub(crate) capture_requested_at: f64,
    pub(crate) pads_held: u32,
    pub(crate) variable_ids: std::collections::BTreeMap<String, String>,
    /// Pause game / cursor actions run this frame, applied by [`Play::settle`].
    pub(crate) pending: Vec<GameSet>,
    /// What each open overlay changed, undone when it closes (oldest first).
    holds: Vec<Hold>,
    /// The player's own volume in Play (0..1), on top of the file's volumes. Kept for the app run.
    pub(crate) volume: f32,
    /// Focus in the navigable menu the open screen owns.
    pub(crate) menu: menu::Focus,
    /// The selected slot of each hotbar (Play only).
    pub(crate) hotbars: hotbar::Selection,
    /// Selected hotbar slots that have no selected variant: the player outlines them.
    pub(crate) hotbar_outline: Vec<figcraft_doc::NodeId>,
    /// How far each map is moved and zoomed (Play only).
    pub(crate) maps: std::collections::BTreeMap<figcraft_doc::NodeId, map::View>,
    /// Screens redrawn off the UI thread (see [`frames`]).
    pub(crate) frames: frames::Frames,
    /// The third-person game, when that is the file's game (`game` stays the first-person one).
    pub(crate) tps: Option<figcraft_game::tps::Tps>,
}

/// A game action from a prototype interaction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GameSet {
    Pause(bool),
    Cursor(bool),
}

/// The pause and cursor state from before an overlay changed them.
#[derive(Clone, Copy, Debug)]
struct Hold {
    overlay: figcraft_doc::NodeId,
    paused: Option<bool>,
    captured: Option<bool>,
}

impl Play {
    pub fn new() -> Self {
        Self { captured: true, volume: 1.0, ..Self::default() }
    }

    /// Advance the game to `now`, egui's input time (seconds). The first frame only starts the
    /// clock. Returns the steps simulated.
    pub fn frame(&mut self, now: f64) -> u32 {
        let dt = self.last.map_or(0.0, |last| now - last);
        if now.is_finite() {
            self.last = Some(now);
        }
        browser_buttons::playing();
        // Graybox: the third-person game runs instead of the first-person one.
        if let Some(steps) = tps::advance(self, dt) {
            return steps;
        }
        self.game.advance(dt)
    }

    /// Apply this frame's Pause game / cursor actions. With an overlay open they belong to the top
    /// one: when it closes (however it closes) the game goes back to how it was before it.
    /// `open` lists the open overlays, bottom first.
    pub(crate) fn settle(&mut self, open: &[figcraft_doc::NodeId]) {
        while let Some(i) = self.holds.iter().rposition(|h| !open.contains(&h.overlay)) {
            let hold = self.holds.remove(i);
            if let Some(paused) = hold.paused {
                self.game.paused = paused;
            }
            if let Some(captured) = hold.captured {
                self.captured = captured;
            }
        }
        for set in std::mem::take(&mut self.pending) {
            let hold = match open.last() {
                Some(&top) => {
                    if !self.holds.iter().any(|h| h.overlay == top) {
                        self.holds.push(Hold { overlay: top, paused: None, captured: None });
                    }
                    self.holds.iter_mut().find(|h| h.overlay == top)
                }
                None => None,
            };
            match set {
                GameSet::Pause(paused) => {
                    if let Some(hold) = hold {
                        hold.paused.get_or_insert(self.game.paused);
                    }
                    self.game.paused = paused;
                }
                GameSet::Cursor(captured) => {
                    if let Some(hold) = hold {
                        hold.captured.get_or_insert(self.captured);
                    }
                    self.captured = captured;
                }
            }
        }
    }

    /// Keep the clock at `now` without simulating (the game is frozen behind a screen).
    pub(crate) fn hold_clock(&mut self, now: f64) {
        if now.is_finite() {
            self.last = Some(now);
        }
    }

    pub fn game(&self) -> &Game {
        &self.game
    }

    pub fn inspect(&self) -> serde_json::Value {
        if let Some(report) = tps::inspect(self) {
            return report;
        }
        let p = &self.game.player;
        serde_json::json!({
            "position": p.position.to_array(), "yaw": p.yaw, "pitch": p.pitch,
            "health": p.health, "maxHealth": p.max_health, "ammo": p.ammo,
            "reserve": p.reserve, "crouching": p.crouching, "onFire": p.on_fire,
            "dead": p.dead, "paused": self.game.paused, "captured": self.captured,
            "ticks": self.game.ticks(), "enemies": self.game.enemies.iter().map(|e|
                serde_json::json!({"position":e.position.to_array(),"health":e.health,"respawnRemaining":e.respawn_remaining})
            ).collect::<Vec<_>>(),
            "projectiles": self.game.projectiles.len()
        })
    }
}

/// The file's game settings, at the end of the Prototype tab when nothing is selected (beside
/// upstream's prototype settings). Each part folds away; all are closed at first.
pub(crate) fn prototype_section(app: &mut crate::FigApp, ui: &mut egui::Ui) {
    crate::widgets::separator(ui);
    // The game Play runs under the screens, first of everything.
    padded(ui, |ui| {
        ui.add_space(8.0);
        ui.spacing_mut().item_spacing = egui::vec2(8.0, 6.0);
        kind_ui::picker(app, ui);
    });
    crate::widgets::separator(ui);
    // The pseudo-localization switch, open: it's for any screen, not one part of the game.
    padded(ui, |ui| {
        ui.add_space(8.0);
        ui.spacing_mut().item_spacing = egui::vec2(8.0, 6.0);
        pseudo_view::switch(ui);
    });
    crate::widgets::separator(ui);
    type Part = fn(&mut crate::FigApp, &mut egui::Ui);
    let parts: [(&str, Part); 7] = [
        ("Game controls", editor::controls),
        ("Game inputs", editor::inputs),
        ("Game events", editor::events),
        ("Sounds", audio::show),
        ("Share Play link", share::show),
        ("Example screens", editor::examples),
        ("Unity export", unity_ui::settings),
    ];
    for (i, (title, show)) in parts.into_iter().enumerate() {
        if i > 0 {
            crate::widgets::separator(ui);
        }
        // Headers in the panel's grid, like Prototype settings and Flow starting points.
        if color_fold::section(ui, egui::Id::unique(("greybox_prototype_section", title)), title) {
            padded(ui, |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(8.0, 6.0);
                show(app, ui);
            });
        }
    }
}

/// Prototype tab, one auto layout frame selected: "Navigable menu" and its options.
pub(crate) fn menu_section(app: &mut crate::FigApp, ui: &mut egui::Ui, doc: &figcraft_doc::Document, node: &figcraft_doc::Node) {
    use figcraft_doc::game::DisabledItems;
    use serde_json::json;
    if node.layout.is_none() || !node.kind.is_frame_like() {
        return;
    }
    let id = node.id;
    crate::widgets::separator(ui);
    crate::widgets::section_header(ui, "Menu navigation", &[], false);
    let menu = doc.game.menus.get(&id);
    let set = |app: &mut crate::FigApp, fields: serde_json::Value| {
        let mut params = json!({"node": id});
        if let (Some(p), Some(f)) = (params.as_object_mut(), fields.as_object()) {
            p.extend(f.clone());
        }
        app.run("game.menu.set", params);
    };
    ui.push_id(("greybox_menu", id), |ui| {
        padded(ui, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(8.0, 6.0);
            if crate::widgets::checkbox(ui, menu.is_some(), "Navigable menu") {
                set(app, json!({"enabled": menu.is_none()}));
            }
            let Some(menu) = menu else { return };
            if crate::widgets::checkbox(ui, menu.wrap, "Loop at the end") {
                set(app, json!({"wrap": !menu.wrap}));
            }
            if crate::widgets::checkbox(ui, menu.remember, "Remember last item") {
                set(app, json!({"remember": !menu.remember}));
            }
            let name = |c: figcraft_doc::NodeId| doc.node(c).map(|n| n.name.clone()).unwrap_or_default();
            let current = menu.start.map_or_else(|| "First item".to_string(), name);
            let t = crate::theme::Tokens::get(ui.ctx());
            crate::widgets::label(ui, "Start at", t.text_secondary);
            let response = crate::widgets::dropdown(ui, &current, ui.available_width(), None);
            crate::right::menu_popup(&response).show(|ui| {
                if crate::right::menu_check(ui, menu.start.is_none(), "First item", "") {
                    set(app, json!({"start": null}));
                    ui.close();
                }
                for child in &node.children {
                    if crate::right::menu_check(ui, menu.start == Some(*child), &name(*child), "") {
                        set(app, json!({"start": child}));
                        ui.close();
                    }
                }
            });
            crate::widgets::label(ui, "Disabled items", t.text_secondary);
            for (label, value, wire) in [("Can be focused", DisabledItems::Focus, "focus"), ("Are skipped", DisabledItems::Skip, "skip")] {
                if crate::widgets::radio(ui, menu.disabled == value, label) {
                    set(app, json!({"disabled": wire}));
                }
            }
            // Graybox: the looks are picked from the items' own variants, no special names needed.
            let items = menu::items(doc, id);
            look_ui::warnings(ui, doc, &items, "Item");
            let component = looks::first_component(doc, &items);
            let params = |key: &'static str| move |look: serde_json::Value| json!({"node": id, key: look});
            for (title, look, role, key) in [
                ("When focused", &menu.focus_look, looks::Role::Selected, "focusLook"),
                ("When pressed", &menu.pressed_look, looks::Role::Pressed, "pressedLook"),
                ("Disabled when", &menu.disabled_look, looks::Role::Disabled, "disabledLook"),
            ] {
                look_ui::picker(app, ui, doc, title, look, component, role, "game.menu.set", &params(key));
            }
        })
    });
}

/// Graybox sections inside the right panel: the same side padding as upstream's rows.
pub(crate) fn padded<R>(ui: &mut egui::Ui, body: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let margin = egui::Margin { left: crate::widgets::PAD as i8, right: crate::widgets::PAD_RIGHT as i8, top: 0, bottom: 4 };
    egui::Frame::NONE.inner_margin(margin).show(ui, body).inner
}
