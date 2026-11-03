//! Greybox Play mode: the platform input, GPU scene and game editor around the pure simulation.

use figcraft_game::Game;

pub mod animation;
pub mod audio;
pub mod brand;
pub mod editor;
pub(crate) mod input;
pub mod menu;
pub mod renderer;
pub mod share;

/// Platform hosts read these markers inside the browser's trusted input event, before egui's
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

    pub fn game(&self) -> &Game {
        &self.game
    }

    pub fn inspect(&self) -> serde_json::Value {
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
    type Part = fn(&mut crate::FigApp, &mut egui::Ui);
    let parts: [(&str, Part); 5] = [
        ("Game controls", editor::controls),
        ("Game events", editor::events),
        ("Sounds", audio::show),
        ("Share Play link", share::show),
        ("Example screens", editor::examples),
    ];
    for (title, show) in parts {
        egui::CollapsingHeader::new(title).id_salt(("greybox_prototype_section", title)).default_open(false).show(ui, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(8.0, 6.0);
            show(app, ui);
        });
    }
}
