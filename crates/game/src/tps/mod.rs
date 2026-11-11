//! The third-person game (G11): a deterministic fixed-step simulation, like [`crate::Game`] (the
//! first-person one) but a separate type, so the two share nothing but the clock and the input.
//!
//! The boundary with the renderer is [`Tps::frame`]: everything a picture needs (the world to draw
//! once, and each frame the hero's pose and the camera, interpolated between two steps).
//!
//! Who owns which file (so phases can be built side by side):
//! - G11.2 world: `world.rs` (and new files for terrain, props, climbable tags).
//! - G11.3 hero and animation: `hero.rs` (the pose) and new rig/animation files.
//! - G11.4 movement and camera: `controller.rs`, `camera.rs`.
//! - G11.5 combat: new files, plus the events and variables of this module.
mod camera;
mod controller;
mod hero;
mod world;

use crate::STEP_SECS;
use crate::clock::FixedClock;
use crate::state::Input;
use glam::Vec3;

pub use camera::{OrbitCamera, PITCH_MAX, PITCH_MIN};
pub use hero::{HEIGHT as HERO_HEIGHT, Hero, HeroPose, RADIUS as HERO_RADIUS};
pub use world::{HALF_EXTENT, World};

#[cfg(test)]
mod tests;

const DT: f32 = STEP_SECS as f32;

/// What the renderer draws this display frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub hero: HeroPose,
    pub eye: Vec3,
    /// What the camera looks at (the hero's middle: the hero is centred on screen).
    pub target: Vec3,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Tps {
    clock: FixedClock,
    ticks: u64,
    /// Held keys and sticks, set each display frame (the same input the first-person game takes:
    /// `forward` and `strafe` move, `look` turns the camera, `jump` is a press).
    pub input: Input,
    pub paused: bool,
    pub hero: Hero,
    pub camera: OrbitCamera,
    world: World,
    /// A jump pressed since the last step (a press shorter than a step is not lost).
    pending_jump: bool,
}

impl Default for Tps {
    fn default() -> Self {
        let world = World::new();
        let hero = Hero::at(world.spawn());
        Self {
            clock: FixedClock::default(),
            ticks: 0,
            input: Input::default(),
            paused: false,
            hero,
            camera: OrbitCamera::default(),
            world,
            pending_jump: false,
        }
    }
}

impl Tps {
    pub fn new() -> Self {
        Self::default()
    }

    /// Run `dt` seconds of real time: the camera turns by the look input once, then as many whole
    /// steps as are due move the hero. Returns the steps run. An invalid `dt` changes nothing.
    pub fn advance(&mut self, dt: f64) -> u32 {
        if !dt.is_finite() || dt <= 0.0 {
            return 0;
        }
        if self.paused {
            self.input.look = [0.0; 2];
            self.input.jump = false;
            self.pending_jump = false;
            return 0;
        }
        self.camera.turn(self.input.look);
        self.input.look = [0.0; 2];
        self.pending_jump |= std::mem::take(&mut self.input.jump);
        let steps = self.clock.advance(dt);
        for _ in 0..steps {
            self.ticks = self.ticks.saturating_add(1);
            let jump = std::mem::take(&mut self.pending_jump);
            controller::step(&mut self.hero, &self.input, self.camera.yaw, &self.world, jump, DT);
        }
        steps
    }

    /// The hero and the camera as they are now, between the last two steps.
    pub fn frame(&self) -> Frame {
        let hero = self.hero.pose(self.clock.alpha() as f32);
        let (eye, target) = self.camera.eye_and_target(hero.position, &self.world);
        Frame { hero, eye, target }
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    pub fn ticks(&self) -> u64 {
        self.ticks
    }

    pub fn time(&self) -> f64 {
        self.ticks as f64 * STEP_SECS
    }
}
