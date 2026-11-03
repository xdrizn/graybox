//! Greybox's game layer: the simulation of the playable test scene that runs under a prototype in
//! Play mode.
//!
//! Pure simulation: no windowing, GPU, input devices or audio. Drawing the scene, reading the
//! keyboard, mouse and controllers and playing sound live in `figcraft-ui-egui`'s `game` module, so
//! this crate runs the same on desktop, in the browser and in tests. The game advances in fixed
//! steps ([`FixedClock`]) whatever the display's frame rate.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod clock;
pub use clock::{FixedClock, MAX_STEPS, STEP_SECS};

#[cfg(test)]
mod tests;

/// The game: everything that moves in the scene, advanced in fixed steps.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Game {
    clock: FixedClock,
    /// Steps simulated since the game started.
    ticks: u64,
}

impl Game {
    pub fn new() -> Self {
        Self::default()
    }

    /// Advance by `dt` seconds of real time; returns the steps simulated (at most [`MAX_STEPS`]).
    pub fn advance(&mut self, dt: f64) -> u32 {
        let steps = self.clock.advance(dt);
        for _ in 0..steps {
            self.step();
        }
        steps
    }

    /// One fixed step of [`STEP_SECS`].
    fn step(&mut self) {
        self.ticks = self.ticks.saturating_add(1);
    }

    /// Steps simulated since the game started.
    pub fn ticks(&self) -> u64 {
        self.ticks
    }

    /// Game time in seconds: the steps simulated, not the real time that passed (a long stall
    /// isn't caught up).
    pub fn time(&self) -> f64 {
        self.ticks as f64 * STEP_SECS
    }

    /// How far real time is between the last step and the next (0..1), to draw between them.
    pub fn alpha(&self) -> f64 {
        self.clock.alpha()
    }
}
