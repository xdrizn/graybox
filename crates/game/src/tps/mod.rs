//! The third-person game (G11): a deterministic fixed-step simulation, like [`crate::Game`] (the
//! first-person one) but a separate type, so the two share nothing but the clock and the input.
//!
//! The boundary with the renderer is [`Tps::frame`]: everything a picture needs (the world to draw
//! once, and each frame the hero's pose and the camera, interpolated between two steps).
//!
//! Who owns which file (so phases can be built side by side):
//! - G11.2 world: `world.rs` (and new files for terrain, props, climbable tags).
//! - G11.3 hero and animation: `hero.rs` (the pose), `rig.rs`, `clip.rs`, `clips.rs`, `animator.rs`.
//! - G11.4 movement and camera: `controller.rs`, `collide.rs`, `camera.rs`.
//! - G11.5 combat: `combat.rs` (weapons and strikes) and new files, plus the events and variables of this module.
mod animator;
mod camera;
mod clip;
mod clips;
mod collide;
mod combat;
mod controller;
mod hero;
mod layout;
mod props;
pub mod rig;
mod terrain;
mod world;

use crate::STEP_SECS;
use crate::clock::FixedClock;
use crate::state::Input;
use glam::Vec3;

pub use animator::{Animator, Drive, LOCOMOTION, Look, Weapon, locomotion_weights};
pub use camera::{OrbitCamera, PITCH_MAX, PITCH_MIN};
pub use clip::{Clip, ClipId, Ease, Key, Layer, Mode};
pub use clips::clip;
pub use combat::Combat;
pub use controller::{Action, Moves, Presses, STAMINA_MAX};
pub use hero::{HEIGHT as HERO_HEIGHT, Hero, HeroPose, RADIUS as HERO_RADIUS};
pub use layout::{ROCK_MAX, TREE_MAX};
pub use props::{
    ARCH_LEG_HEIGHT, ARCH_LEG_SIZE, ARCH_LEG_X, BLOCK_SIZE, BROKEN_PILLAR_HEIGHT, CRATE_SIZE, ClimbKind, Climbable, PILLAR_HEIGHT, PILLAR_SIZE, Prop,
    PropKind, Solid,
};
pub use terrain::{CELL, CELLS, FALL_X, RIVER_END, RIVER_HALF_WIDTH, Terrain, WATER_DEPTH, ZONES, Zone};
pub use world::{HALF_EXTENT, World};

#[cfg(test)]
mod animation_tests;
#[cfg(test)]
mod movement_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod world_tests;

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
    /// Presses since the last step (a press shorter than a step is not lost): jump, roll, attack, weapon.
    pending: Presses,
    /// The attack button was held last frame (a press is when it goes down).
    attack_held: bool,
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
            pending: Presses::default(),
            attack_held: false,
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
            self.input.roll = false;
            self.input.weapon = 0;
            self.pending = Presses::default();
            return 0;
        }
        self.camera.turn(self.input.look);
        self.input.look = [0.0; 2];
        let held = self.input.shoot;
        self.pending.jump |= std::mem::take(&mut self.input.jump);
        self.pending.roll |= std::mem::take(&mut self.input.roll);
        self.pending.attack |= held && !self.attack_held;
        self.attack_held = held;
        let weapon = std::mem::take(&mut self.input.weapon);
        if weapon != 0 {
            self.pending.weapon = weapon;
        }
        let steps = self.clock.advance(dt);
        for _ in 0..steps {
            self.ticks = self.ticks.saturating_add(1);
            let presses = std::mem::take(&mut self.pending);
            // The pose viewer (debug) shows a clip with the hero standing still.
            let previewing = self.hero.animator.previewing().is_some();
            let input = if previewing { Input { forward: 0.0, strafe: 0.0, ..self.input } } else { self.input };
            let presses = if previewing { Presses::default() } else { presses };
            combat::step(&mut self.hero, presses, held && !previewing, DT);
            controller::step(&mut self.hero, &input, self.camera.yaw, &self.world, presses, DT);
            self.hero.animate(input.crouch, DT);
        }
        steps
    }

    /// The hero and the camera as they are now, between the last two steps.
    pub fn frame(&self) -> Frame {
        let hero = self.hero.pose(self.clock.alpha() as f32);
        let (eye, target) = self.camera.eye_and_target(hero.position, &self.world);
        Frame { hero, eye, target }
    }

    /// Debug pose viewer: show `clip` over and over on the hero standing still (`None` ends it).
    pub fn preview_clip(&mut self, clip: Option<ClipId>) {
        // The camera comes closer while a clip is shown.
        self.camera.distance = if clip.is_some() { 3.0 } else { OrbitCamera::default().distance };
        self.hero.animator.preview(clip);
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
