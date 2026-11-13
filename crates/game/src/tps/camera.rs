//! The orbit camera: turned by the mouse and the right stick on its own, the hero always centred.
//!
//! G11.4: it stops short of walls, trunks and hills between it and the hero.
//!
//! G11.6: the bow's aim mode brings it in over a shoulder (the right one at first, the hero on the
//! left of the screen, the middle of the screen free for the crosshair), easing in and out; the
//! shoulder button swaps sides, easing across.
use super::world::World;
use glam::Vec3;

/// Looking down this far at most / up this far at most (radians).
pub const PITCH_MIN: f32 = -1.2;
pub const PITCH_MAX: f32 = 0.6;
/// Metres from the hero to the camera.
const DISTANCE: f32 = 4.5;
/// The camera looks at this height above the hero's feet, so the hero sits in the middle of the screen.
const FOCUS_HEIGHT: f32 = 1.0;
/// The camera never goes lower than this above the ground.
const GROUND_CLEARANCE: f32 = 0.3;
/// When something is in the way, the camera stops this far in front of it, and never closer to the
/// hero's middle than `MIN_DISTANCE`.
const NEAR_GAP: f32 = 0.25;
const MIN_DISTANCE: f32 = 0.6;
/// Aim mode: this far behind, looking at this height, this far to the side of the hero.
const AIM_DISTANCE: f32 = 1.9;
const AIM_HEIGHT: f32 = 1.5;
const AIM_SHOULDER: f32 = 0.65;
/// How fast the camera eases into and out of aim mode, and to the other shoulder (per second).
const AIM_RATE: f32 = 10.0;
const SIDE_RATE: f32 = 8.0;

#[derive(Clone, Debug, PartialEq)]
pub struct OrbitCamera {
    /// 0 looks along -Z; a positive turn is to the right (like the first-person game).
    pub yaw: f32,
    /// Positive looks up.
    pub pitch: f32,
    pub distance: f32,
    /// How far into aim mode (0 behind the hero, 1 over the shoulder), this step and the last.
    aim: f32,
    last_aim: f32,
    /// Which shoulder (1 right, -1 left, between while swapping), this step and the last.
    side: f32,
    last_side: f32,
    /// The shoulder it goes to: the right one (the hero on the left of the screen).
    right_shoulder: bool,
}

impl Default for OrbitCamera {
    fn default() -> Self {
        Self { yaw: 0.0, pitch: -0.25, distance: DISTANCE, aim: 0.0, last_aim: 0.0, side: 1.0, last_side: 1.0, right_shoulder: true }
    }
}

impl OrbitCamera {
    /// Turn by `look` (radians: yaw, pitch), as the mouse or right stick gives it. Junk is ignored.
    pub fn turn(&mut self, look: [f32; 2]) {
        if !self.yaw.is_finite() {
            self.yaw = 0.0;
        }
        if !self.pitch.is_finite() {
            self.pitch = 0.0;
        }
        if look[0].is_finite() {
            self.yaw = (self.yaw + look[0].rem_euclid(std::f32::consts::TAU)).rem_euclid(std::f32::consts::TAU);
        }
        if look[1].is_finite() {
            self.pitch = (self.pitch + look[1].clamp(-std::f32::consts::PI, std::f32::consts::PI)).clamp(PITCH_MIN, PITCH_MAX);
        }
    }

    /// The direction the camera looks (unit).
    pub fn direction(&self) -> Vec3 {
        Vec3::new(self.yaw.sin() * self.pitch.cos(), self.pitch.sin(), -self.yaw.cos() * self.pitch.cos())
    }

    /// How far into aim mode it is (0 behind the hero, 1 over the shoulder).
    pub fn aim_blend(&self) -> f32 {
        self.aim
    }

    /// Which shoulder it looks over in aim mode (1 right, -1 left, between while swapping).
    pub fn shoulder(&self) -> f32 {
        self.side
    }

    /// Go over the other shoulder (eased across in the next steps).
    pub fn swap_shoulder(&mut self) {
        self.right_shoulder = !self.right_shoulder;
    }

    /// One fixed step: ease toward aim mode (`aiming`) or back, and toward the chosen shoulder.
    pub(crate) fn follow(&mut self, aiming: bool, dt: f32) {
        let ease = |from: f32, to: f32, rate: f32| {
            let from = if from.is_finite() { from } else { to };
            let next = from + (to - from) * (1.0 - (-rate * dt.max(0.0)).exp());
            if (next - to).abs() < 1e-3 { to } else { next }
        };
        self.last_aim = self.aim;
        self.last_side = self.side;
        self.aim = ease(self.aim, if aiming { 1.0 } else { 0.0 }, AIM_RATE).clamp(0.0, 1.0);
        self.side = ease(self.side, if self.right_shoulder { 1.0 } else { -1.0 }, SIDE_RATE).clamp(-1.0, 1.0);
    }

    /// Where the camera is and what it looks at, for a hero whose feet are at `feet` (this step).
    pub fn eye_and_target(&self, feet: Vec3, world: &World) -> (Vec3, Vec3) {
        self.eye_and_target_at(feet, world, 1.0)
    }

    /// The same, `alpha` (0..1) of the way from the last step to this one (the aim easing).
    pub fn eye_and_target_at(&self, feet: Vec3, world: &World, alpha: f32) -> (Vec3, Vec3) {
        let alpha = if alpha.is_finite() { alpha.clamp(0.0, 1.0) } else { 1.0 };
        let aim = (self.last_aim + (self.aim - self.last_aim) * alpha).clamp(0.0, 1.0);
        let side = (self.last_side + (self.side - self.last_side) * alpha).clamp(-1.0, 1.0);
        let aim = if aim.is_finite() { aim } else { 0.0 };
        let side = if side.is_finite() { side } else { 1.0 };
        let head = feet + Vec3::Y * (FOCUS_HEIGHT + (AIM_HEIGHT - FOCUS_HEIGHT) * aim);
        // Over the shoulder: the target moves to the side, short of a wall there.
        let right = Vec3::new(self.yaw.cos(), 0.0, self.yaw.sin());
        let reach = AIM_SHOULDER * aim * side;
        let mut target = head + right * reach;
        if reach.abs() > 1e-3 {
            let clear = super::collide::clear_sight(world, head, target);
            if clear < 1.0 {
                let keep = ((clear * reach.abs() - 0.1) / reach.abs()).max(0.0);
                target = head + right * reach * keep;
            }
        }
        let distance = self.distance.clamp(1.0, 12.0);
        let distance = distance + (AIM_DISTANCE - distance) * aim;
        let mut eye = target - self.direction() * distance;
        // G11.4: stop short of a wall, a trunk or a hill between the hero and the camera.
        let clear = super::collide::clear_sight(world, target, eye);
        if clear < 1.0 {
            eye = target - self.direction() * (clear * distance - NEAR_GAP).max(MIN_DISTANCE);
        }
        let floor = world.ground_height(eye.x, eye.z) + GROUND_CLEARANCE;
        if eye.y < floor {
            eye.y = floor;
        }
        (eye, target)
    }
}
