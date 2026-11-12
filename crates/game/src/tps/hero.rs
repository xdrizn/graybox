//! The hero's state, and the pose the renderer reads (G11.3: with the animated skeleton).
//!
//! G11.3 (hero and animation) owns the use of [`HeroPose`]: it is everything a rig needs to choose
//! and blend clips. G11.4 owns how the state changes (`controller.rs`).
use super::animator::{Animator, Drive, Weapon};
use super::clip::ClipId;
use super::controller::Moves;
use super::rig::Pose;
use glam::{Quat, Vec3};

/// The hero is about this tall and wide (metres).
pub const HEIGHT: f32 = 1.7;
pub const RADIUS: f32 = 0.3;

/// Wrap an angle to `[-PI, PI)`.
pub(crate) fn wrap_pi(angle: f32) -> f32 {
    if !angle.is_finite() {
        return 0.0;
    }
    (angle + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

#[derive(Clone, Debug, PartialEq)]
pub struct Hero {
    /// Feet position.
    pub position: Vec3,
    /// Metres a second, including the vertical part.
    pub velocity: Vec3,
    /// Which way the hero faces: 0 looks along -Z and a positive turn is to the right.
    pub facing: f32,
    pub grounded: bool,
    /// Where it was one step ago, so the renderer can draw between two steps.
    pub(crate) previous_position: Vec3,
    pub(crate) previous_facing: f32,
    /// Chooses and blends the clips (G11.3); stepped after each move.
    pub animator: Animator,
    /// Climbing, rolling, stamina… (G11.4).
    pub moves: Moves,
}

impl Hero {
    /// A hero standing at `position`, which must be on the ground.
    pub fn at(position: Vec3) -> Self {
        Self {
            position,
            velocity: Vec3::ZERO,
            facing: 0.0,
            grounded: true,
            previous_position: position,
            previous_facing: 0.0,
            animator: Animator::new(),
            moves: Moves::default(),
        }
    }

    /// Horizontal speed (metres a second).
    pub fn speed(&self) -> f32 {
        Vec3::new(self.velocity.x, 0.0, self.velocity.z).length()
    }

    /// Step the animation after a move: speed, facing, ground and crouch drive the clips.
    pub fn animate(&mut self, crouch: bool, dt: f32) {
        let drive = Drive { velocity: self.velocity, facing: self.facing, grounded: self.grounded, crouch };
        self.animator.step(&drive, dt);
    }

    /// The pose `alpha` (0..1) of the way from the previous step to this one.
    pub fn pose(&self, alpha: f32) -> HeroPose {
        let alpha = if alpha.is_finite() { alpha.clamp(0.0, 1.0) } else { 1.0 };
        let look = self.animator.look(alpha);
        HeroPose {
            position: self.previous_position.lerp(self.position, alpha),
            facing: wrap_pi(self.previous_facing + wrap_pi(self.facing - self.previous_facing) * alpha),
            speed: self.speed(),
            vertical_speed: self.velocity.y,
            grounded: self.grounded,
            skeleton: look.pose,
            tilt: look.tilt,
            weapon: self.animator.weapon(),
            in_hand: self.animator.in_hand(),
            bow_draw: look.bow_draw,
            clip: self.animator.current(),
        }
    }
}

/// What the hero looks like right now: where, which way, and how it is moving.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeroPose {
    /// Feet position, between two simulation steps.
    pub position: Vec3,
    pub facing: f32,
    /// Horizontal speed (m/s).
    pub speed: f32,
    pub vertical_speed: f32,
    pub grounded: bool,
    /// Each bone turned (G11.3), between two steps like the rest.
    pub skeleton: Pose,
    /// How the whole body tilts about the feet (leaning into turns and speed changes).
    pub tilt: Quat,
    pub weapon: Weapon,
    /// The weapon is in the hand (else on the back).
    pub in_hand: bool,
    /// How far the bowstring is drawn (0..1).
    pub bow_draw: f32,
    /// The clip that shows most (for the control channel and the preview).
    pub clip: ClipId,
}
