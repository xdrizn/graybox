//! The orbit camera: turned by the mouse and the right stick on its own, the hero always centred.
//!
//! G11.4: it stops short of walls, trunks and hills between it and the hero.
//!
//! G11.6: the bow's aim mode brings it in over a shoulder (the right one at first, the hero on the
//! left of the screen, the middle of the screen free for the crosshair), easing in and out; the
//! shoulder button swaps sides, easing across. After a while without camera input (and with the hero
//! not steered) it eases back behind the hero; something coming between it and the hero pulls it in
//! at once, but it eases back out; a jump moves the hero up the screen instead of bobbing the view.
use super::hero::wrap_pi;
use super::world::World;
use glam::Vec3;

/// Looking down this far at most / up this far at most (radians).
pub const PITCH_MIN: f32 = -1.2;
pub const PITCH_MAX: f32 = 0.6;
/// Metres from the hero to the camera.
const DISTANCE: f32 = 4.5;
/// How far it looks down when it starts, and when it eases back behind the hero.
const DEFAULT_PITCH: f32 = -0.25;
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
const AIM_HEIGHT: f32 = 1.6;
const AIM_SHOULDER: f32 = 1.0;
/// How fast the camera eases into and out of aim mode, and to the other shoulder (per second).
const AIM_RATE: f32 = 10.0;
const SIDE_RATE: f32 = 8.0;
/// Seconds without camera input (and without steering) before the camera eases back behind the hero.
pub(crate) const RECENTER_AFTER: f32 = 2.5;
/// How stiff that easing is (a critically damped spring: about 2 s to settle, starting gently).
const RECENTER_STIFFNESS: f32 = 2.4;
/// How fast it eases back out once nothing is in the way (per second).
const PULL_OUT_RATE: f32 = 5.0;
/// In a jump or a fall the view's height follows the hero's on a spring this stiff, and trails it by
/// at most `LIFT_MAX`; on the ground the trail springs back to nothing, so hills never move the hero
/// off the middle of the screen.
const AIR_STIFFNESS: f32 = 5.0;
const GROUND_STIFFNESS: f32 = 9.0;
const LIFT_MAX: f32 = 1.0;

/// What the camera follows each step.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Follow {
    /// The hero's feet and facing.
    pub feet: Vec3,
    pub facing: f32,
    /// The bow's aim mode is on.
    pub aiming: bool,
    /// In a jump or a fall (not on the ground, a ladder or a wall).
    pub airborne: bool,
    /// The player is moving the hero (the camera never turns then: it would turn the steering).
    pub steering: bool,
}

/// One step of a critically damped spring from `value` (moving at `speed`) toward `goal`.
fn spring(value: f32, speed: f32, goal: f32, stiffness: f32, dt: f32) -> (f32, f32) {
    let offset = value - goal;
    let decay = (-stiffness * dt).exp();
    let push = (speed + stiffness * offset) * dt;
    let (value, speed) = (goal + (offset + push) * decay, (speed - stiffness * push) * decay);
    if value.is_finite() && speed.is_finite() { (value, speed) } else { (goal, 0.0) }
}

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
    /// Seconds without camera input and steering; the easing back's speed (yaw, pitch); how much
    /// it turned in the last step (drawn in between, like the hero).
    idle: f32,
    recenter_speed: [f32; 2],
    recentered: [f32; 2],
    /// The share of the free distance with nothing in the way (eased back out), this step and the last.
    reach: f32,
    last_reach: f32,
    /// How far the view's height trails the hero's (metres, below is negative), how fast it changes,
    /// the last step's, and the feet's height in the last step.
    lift: f32,
    lift_speed: f32,
    last_lift: f32,
    last_feet: Option<f32>,
}

impl Default for OrbitCamera {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            pitch: DEFAULT_PITCH,
            distance: DISTANCE,
            aim: 0.0,
            last_aim: 0.0,
            side: 1.0,
            last_side: 1.0,
            right_shoulder: true,
            idle: 0.0,
            recenter_speed: [0.0; 2],
            recentered: [0.0; 2],
            reach: 1.0,
            last_reach: 1.0,
            lift: 0.0,
            lift_speed: 0.0,
            last_lift: 0.0,
            last_feet: None,
        }
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
        if look.iter().any(|value| *value != 0.0) {
            // The player has the camera: easing back waits again, and stops where it is.
            self.idle = 0.0;
            self.recenter_speed = [0.0; 2];
            self.recentered = [0.0; 2];
        }
    }

    /// The direction the camera looks (unit).
    pub fn direction(&self) -> Vec3 {
        direction(self.yaw, self.pitch)
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

    /// One fixed step: ease toward aim mode or back and toward the chosen shoulder, back behind the
    /// hero after a while, back out after something was in the way, and after the hero's height.
    pub(crate) fn follow(&mut self, hero: Follow, world: &World, dt: f32) {
        let dt = if dt.is_finite() { dt.clamp(0.0, 0.25) } else { 0.0 };
        let ease = |from: f32, to: f32, rate: f32| {
            let from = if from.is_finite() { from } else { to };
            let next = from + (to - from) * (1.0 - (-rate * dt).exp());
            if (next - to).abs() < 1e-3 { to } else { next }
        };
        self.last_aim = self.aim;
        self.last_side = self.side;
        self.aim = ease(self.aim, if hero.aiming { 1.0 } else { 0.0 }, AIM_RATE).clamp(0.0, 1.0);
        self.side = ease(self.side, if self.right_shoulder { 1.0 } else { -1.0 }, SIDE_RATE).clamp(-1.0, 1.0);
        self.recenter(hero, dt);
        self.follow_height(hero, dt);
        // Pulled in at once (the picture also checks every frame), eased back out.
        self.last_reach = self.reach;
        let (_, _, used) = self.place(hero.feet, world, [self.yaw, self.pitch], [self.aim, self.side, self.lift], 1.0);
        let clear = used / self.free_distance(self.aim);
        self.reach = if clear.is_finite() && clear < self.reach { clear.max(0.0) } else { ease(self.reach, 1.0, PULL_OUT_RATE).clamp(0.0, 1.0) };
    }

    /// Back behind the hero (and to the usual height) after a while without camera input or steering.
    fn recenter(&mut self, hero: Follow, dt: f32) {
        self.recentered = [0.0; 2];
        if hero.steering || hero.aiming || !hero.facing.is_finite() {
            self.idle = 0.0;
            self.recenter_speed = [0.0; 2];
            return;
        }
        self.idle = (self.idle + dt).min(RECENTER_AFTER);
        if self.idle < RECENTER_AFTER {
            return;
        }
        let off = wrap_pi(self.yaw - hero.facing);
        let (next_off, yaw_speed) = spring(off, self.recenter_speed[0], 0.0, RECENTER_STIFFNESS, dt);
        let (pitch, pitch_speed) = spring(self.pitch, self.recenter_speed[1], DEFAULT_PITCH, RECENTER_STIFFNESS, dt);
        let pitch = pitch.clamp(PITCH_MIN, PITCH_MAX);
        let turn = next_off - off;
        self.recentered = [turn, pitch - self.pitch];
        self.recenter_speed = [yaw_speed, pitch_speed];
        self.yaw = (self.yaw + turn).rem_euclid(std::f32::consts::TAU);
        self.pitch = pitch;
    }

    /// The view's height trails the hero's in the air (the hero moves up the screen in a jump) and
    /// springs back to it on the ground, a ladder or a wall.
    fn follow_height(&mut self, hero: Follow, dt: f32) {
        self.last_lift = self.lift;
        let feet = hero.feet.y;
        if !feet.is_finite() {
            return;
        }
        let moved = self.last_feet.map_or(0.0, |last| feet - last);
        self.last_feet = Some(feet);
        let (lift, speed) = if hero.airborne {
            // The height it looks from stays put while the feet move, then springs after them.
            spring(self.lift - moved, self.lift_speed, 0.0, AIR_STIFFNESS, dt)
        } else {
            spring(self.lift, self.lift_speed, 0.0, GROUND_STIFFNESS, dt)
        };
        self.lift = lift.clamp(-LIFT_MAX, LIFT_MAX);
        self.lift_speed = if lift.abs() >= LIFT_MAX && lift * speed > 0.0 { 0.0 } else { speed };
        if self.lift.abs() < 1e-4 && self.lift_speed.abs() < 1e-3 {
            self.lift = 0.0;
            self.lift_speed = 0.0;
        }
    }

    /// The distance behind the target when nothing is in the way, `aim` of the way into aim mode.
    fn free_distance(&self, aim: f32) -> f32 {
        let distance = self.distance.clamp(1.0, 12.0);
        distance + (AIM_DISTANCE - distance) * aim
    }

    /// Where the camera is and what it looks at, for a hero whose feet are at `feet` (this step).
    pub fn eye_and_target(&self, feet: Vec3, world: &World) -> (Vec3, Vec3) {
        self.eye_and_target_at(feet, world, 1.0)
    }

    /// The same, `alpha` (0..1) of the way from the last step to this one (the easings).
    pub fn eye_and_target_at(&self, feet: Vec3, world: &World, alpha: f32) -> (Vec3, Vec3) {
        let alpha = if alpha.is_finite() { alpha.clamp(0.0, 1.0) } else { 1.0 };
        let between = |last: f32, now: f32, fallback: f32| {
            let value = last + (now - last) * alpha;
            if value.is_finite() { value } else { fallback }
        };
        let aim = between(self.last_aim, self.aim, 0.0).clamp(0.0, 1.0);
        let side = between(self.last_side, self.side, 1.0).clamp(-1.0, 1.0);
        let lift = between(self.last_lift, self.lift, 0.0).clamp(-LIFT_MAX, LIFT_MAX);
        let reach = between(self.last_reach, self.reach, 1.0).clamp(0.0, 1.0);
        // The easing back behind the hero is drawn between steps; the player's own turns are not.
        let yaw = self.yaw - self.recentered[0] * (1.0 - alpha);
        let pitch = self.pitch - self.recentered[1] * (1.0 - alpha);
        let angles = if yaw.is_finite() && pitch.is_finite() { [yaw, pitch] } else { [self.yaw, self.pitch] };
        let (eye, target, _) = self.place(feet, world, angles, [aim, side, lift], reach);
        (eye, target)
    }

    /// Where the camera is and what it looks at for these angles (yaw, pitch) and easings (aim mode,
    /// shoulder, height trail), at most `reach` of the free distance away; and how far back it is.
    fn place(&self, feet: Vec3, world: &World, [yaw, pitch]: [f32; 2], [aim, side, lift]: [f32; 3], reach: f32) -> (Vec3, Vec3, f32) {
        let head = feet + Vec3::Y * (FOCUS_HEIGHT + (AIM_HEIGHT - FOCUS_HEIGHT) * aim + lift);
        // Over the shoulder: the target moves to the side, short of a wall there.
        let right = Vec3::new(yaw.cos(), 0.0, yaw.sin());
        let shoulder = AIM_SHOULDER * aim * side;
        let mut target = head + right * shoulder;
        if shoulder.abs() > 1e-3 {
            let clear = super::collide::clear_sight(world, head, target);
            if clear < 1.0 {
                let keep = ((clear * shoulder.abs() - 0.1) / shoulder.abs()).max(0.0);
                target = head + right * shoulder * keep;
            }
        }
        let look = direction(yaw, pitch);
        let free = self.free_distance(aim);
        let distance = (free * reach).max(MIN_DISTANCE.min(free));
        // G11.4: stop short of a wall, a trunk or a hill between the hero and the camera.
        let clear = super::collide::clear_sight(world, target, target - look * distance);
        let distance = if clear < 1.0 { (clear * distance - NEAR_GAP).max(MIN_DISTANCE) } else { distance };
        let mut eye = target - look * distance;
        let floor = world.ground_height(eye.x, eye.z) + GROUND_CLEARANCE;
        if eye.y < floor {
            eye.y = floor;
        }
        (eye, target, distance)
    }
}

/// The unit vector a camera at `yaw` and `pitch` looks along.
fn direction(yaw: f32, pitch: f32) -> Vec3 {
    Vec3::new(yaw.sin() * pitch.cos(), pitch.sin(), -yaw.cos() * pitch.cos())
}
