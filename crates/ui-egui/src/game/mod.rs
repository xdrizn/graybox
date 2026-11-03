//! Greybox Play mode: the platform input, GPU scene and game editor around the pure simulation.

use figcraft_game::Game;

pub mod animation;
pub mod audio;
pub mod brand;
pub mod editor;
pub(crate) mod input;
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
    pub(crate) role_paused: bool,
    pub(crate) role_released: bool,
    pub(crate) applied_capture: Option<bool>,
    pub(crate) capture_requested_at: f64,
    pub(crate) paused_before_role: bool,
    pub(crate) pads_held: u32,
    pub(crate) variable_ids: std::collections::BTreeMap<String, String>,
}

impl Play {
    pub fn new() -> Self {
        Self { captured: true, ..Self::default() }
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

pub(crate) fn settings_window(app: &mut crate::FigApp, ctx: &egui::Context) {
    if !app.ui.game_settings {
        return;
    }
    let mut open = true;
    egui::Window::new(crate::i18n::tr("Game settings"))
        .id(egui::Id::unique("greybox_game_settings"))
        .open(&mut open)
        .default_width(540.0)
        .default_height(650.0)
        .max_height((ctx.content_rect().height() - 80.0).max(200.0))
        .default_pos(ctx.content_rect().center() - egui::vec2(270.0, 325.0))
        .resizable(true)
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().max_height(680.0).show(ui, |ui| {
                editor::show(app, ui);
                ui.separator();
                audio::show(app, ui);
                ui.separator();
                share::show(app, ui);
            });
        });
    app.ui.game_settings = open;
}
