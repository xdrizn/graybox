/// The length of one simulation step (seconds): the game runs at 60 steps a second.
pub const STEP_SECS: f64 = 1.0 / 60.0;

/// Most steps one frame runs. A frame that comes late (the window was hidden, the machine stalled)
/// drops the rest of its backlog, and the next step is a whole step away, instead of
/// fast-forwarding the game.
pub const MAX_STEPS: u32 = 5;

/// Turns the real time between display frames into whole simulation steps.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FixedClock {
    /// Real time not yet simulated (seconds, below [`STEP_SECS`] between calls).
    pending: f64,
}

impl FixedClock {
    /// Add `dt` seconds of real time; returns the steps due now. A `dt` that is negative, zero or
    /// not finite adds nothing.
    pub fn advance(&mut self, dt: f64) -> u32 {
        if !dt.is_finite() || dt <= 0.0 {
            return 0;
        }
        self.pending += dt;
        let mut steps = 0;
        while self.pending >= STEP_SECS && steps < MAX_STEPS {
            self.pending -= STEP_SECS;
            steps += 1;
        }
        // Still behind (or the sum overflowed): drop the backlog.
        if self.pending >= STEP_SECS || !self.pending.is_finite() {
            self.pending = 0.0;
        }
        steps
    }

    /// How far the time not yet simulated is into the next step (0..1).
    pub fn alpha(&self) -> f64 {
        (self.pending / STEP_SECS).clamp(0.0, 1.0)
    }
}
