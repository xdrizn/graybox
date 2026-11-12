//! The orbit camera: turned by the mouse and the right stick on its own, the hero always centred.
//!
//! G11.4: it stops short of walls, trunks and hills between it and the hero. Still to come (G11.6):
//! easing back behind the hero after a while, the bow aim over the shoulder, lock-on.
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

#[derive(Clone, Debug, PartialEq)]
pub struct OrbitCamera {
    /// 0 looks along -Z; a positive turn is to the right (like the first-person game).
    pub yaw: f32,
    /// Positive looks up.
    pub pitch: f32,
    pub distance: f32,
}

impl Default for OrbitCamera {
    fn default() -> Self {
        Self { yaw: 0.0, pitch: -0.25, distance: DISTANCE }
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

    /// Where the camera is and what it looks at, for a hero whose feet are at `feet`.
    pub fn eye_and_target(&self, feet: Vec3, world: &World) -> (Vec3, Vec3) {
        let target = feet + Vec3::Y * FOCUS_HEIGHT;
        let distance = self.distance.clamp(1.0, 12.0);
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
