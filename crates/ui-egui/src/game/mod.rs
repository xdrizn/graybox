//! Play mode's platform side (Greybox, not upstream FigCraft): drives [`figcraft_game`] from the
//! display's frames, and later draws its scene, reads the keyboard, mouse and controllers and plays
//! its sound. Everything here works on desktop and in the browser build.

use figcraft_game::Game;

#[cfg(test)]
mod tests;

/// A game being played: the simulation and the display time it last advanced to.
#[derive(Clone, Debug, Default)]
pub struct Play {
    game: Game,
    /// egui's input time at the last frame (seconds; None before the first).
    last: Option<f64>,
}

impl Play {
    pub fn new() -> Self {
        Self::default()
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
}
