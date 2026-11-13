//! The hero's animator (G11.3): chooses and blends clips, a step at a time.
//!
//! Layers, bottom to top:
//! 1. the **base**: moving on the ground (idle, walk, jog and run blended by speed, crouched or
//!    not by a 0..1 weight, every cycle on one shared phase so feet stay in step), in the air (jump
//!    up, fall) or a whole-body clip (an attack, a roll, climbing, a fall to the ground);
//! 2. an **overlay** clip over it: from the waist up (drawing a weapon, the bow) or every bone at a
//!    weight (a landing, a flinch), fading out at its end;
//! 3. **secondary motion** that no clip holds: the body leans into turns and speed changes, the
//!    head looks into a turn and nods on landing, the cloak flares with speed and swings behind.
//!
//! Every change of clip crossfades from the pose shown last, carried on at its own speed and
//! slowing down (so a swing in progress does not stop dead), to the new one. Nothing jumps.
//!
//! The animator runs in the simulation's fixed steps; [`Animator::look`] returns the pose between
//! the last two steps, for drawing.
use super::clip::{ClipId, Layer, Mode};
use super::clips::{CROUCH_WALK_SPEED, JOG_SPEED, RUN_SPEED, WALK_SPEED, clip};
use super::hero::wrap_pi;
use super::rig::{BONES, Bone, Pose};
use glam::{Quat, Vec3};

/// The weapons the hero can hold (drawn on the right hand, the bow on the left, or on the back).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Weapon {
    None,
    #[default]
    Sword,
    Spear,
    Club,
    Bow,
}

impl Weapon {
    pub const ALL: [Weapon; 4] = [Weapon::Sword, Weapon::Spear, Weapon::Club, Weapon::Bow];

    pub fn name(self) -> &'static str {
        match self {
            Weapon::None => "none",
            Weapon::Sword => "sword",
            Weapon::Spear => "spear",
            Weapon::Club => "club",
            Weapon::Bow => "bow",
        }
    }
}

/// What the movement tells the animator each step.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Drive {
    /// Metres a second, the vertical part included.
    pub velocity: Vec3,
    /// Which way the hero faces (radians, the rule of `Hero::facing`).
    pub facing: f32,
    pub grounded: bool,
    pub crouch: bool,
}

impl Drive {
    fn sanitized(self) -> Drive {
        Drive {
            velocity: if self.velocity.is_finite() { self.velocity.clamp_length_max(60.0) } else { Vec3::ZERO },
            facing: if self.facing.is_finite() { wrap_pi(self.facing) } else { 0.0 },
            ..self
        }
    }
}

/// What a frame draws: the skeleton, how the whole body tilts about the feet, and how far the
/// bowstring is drawn (0..1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub pose: Pose,
    pub tilt: Quat,
    pub bow_draw: f32,
}

impl Default for Look {
    fn default() -> Self {
        Look { pose: Pose::REST, tilt: Quat::IDENTITY, bow_draw: 0.0 }
    }
}

/// A clip being played: which, how far in (seconds), and mirrored (left and right swapped) or not.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Playing {
    clip: ClipId,
    time: f32,
    mirror: bool,
}

impl Playing {
    fn new(clip: ClipId, mirror: bool) -> Self {
        Playing { clip, time: 0.0, mirror }
    }

    fn sample(&self) -> Pose {
        let pose = clip(self.clip).sample(self.time);
        if self.mirror { pose.mirrored() } else { pose }
    }

    fn ended(&self) -> bool {
        let clip = clip(self.clip);
        clip.mode != Mode::Loop && self.time >= clip.duration
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Base {
    Locomotion,
    Air(Playing),
    Clip(Playing),
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Overlay {
    playing: Playing,
    weight: f32,
}

impl Overlay {
    /// The weight now: a `Once` overlay fades out over its last `fade_out` seconds.
    fn weight_now(&self) -> f32 {
        let clip = clip(self.playing.clip);
        if clip.mode != Mode::Once {
            return self.weight;
        }
        let fade = clip.fade_out.max(1e-3);
        let t = ((self.playing.time - (clip.duration - fade)) / fade).clamp(0.0, 1.0);
        self.weight * (1.0 - smooth(t))
    }
}

/// A crossfade from the pose shown when it began, carried on at that moment's turning speed.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Fade {
    from: Pose,
    /// Each bone's turning speed when the fade began (axis × radians a second).
    spin: [Vec3; BONES],
    drift: Vec3,
    elapsed: f32,
    duration: f32,
    /// The new pose of the last step, each rotation kept on the same side as the step before
    /// (a blend between poses more than half a turn apart then still moves smoothly).
    track: [Quat; BONES],
}

impl Fade {
    /// From the carried-on old pose to `to`, `t` (0..1) of the way.
    fn blend(&mut self, to: &Pose, t: f32) -> Pose {
        let from = self.shown();
        let mut out = *to;
        for (index, bone) in out.bones.iter_mut().enumerate() {
            let live = if to.bones[index].dot(self.track[index]) < 0.0 { -to.bones[index] } else { to.bones[index] };
            self.track[index] = live;
            *bone = slerp(from.bones[index], live, t);
        }
        out.root = from.root.lerp(to.root, t);
        out
    }

    fn shown(&self) -> Pose {
        // The carried-on motion slows down: it covers at most `keep` seconds' worth.
        let keep = self.duration * 0.35;
        let carried = keep * (1.0 - (-self.elapsed / keep.max(1e-3)).exp());
        let mut pose = self.from;
        for (bone, spin) in pose.bones.iter_mut().zip(&self.spin) {
            *bone = (Quat::from_scaled_axis(*spin * carried) * *bone).normalize();
        }
        pose.root += self.drift * carried;
        pose
    }
}

/// A damped spring towards a target (`zeta` 1 settles without overshoot, lower swings a little).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Spring {
    value: f32,
    velocity: f32,
}

impl Spring {
    fn step(&mut self, target: f32, omega: f32, zeta: f32, dt: f32) {
        let target = if target.is_finite() { target } else { 0.0 };
        let accel = omega * omega * (target - self.value) - 2.0 * zeta * omega * self.velocity;
        self.velocity += accel * dt;
        self.value += self.velocity * dt;
        if !self.value.is_finite() || !self.velocity.is_finite() {
            *self = Spring::default();
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Preview {
    clip: ClipId,
    /// Seconds since a clip that stops reached its end (it plays again after a pause).
    wait: f32,
}

/// How the hero moved this step (m/s, m/s, rad/s, m/s²).
#[derive(Clone, Copy)]
struct Motion {
    speed: f32,
    vertical: f32,
    turn: f32,
    accel: f32,
}

/// Shortest crossfade, so even a snap takes a few frames.
const MIN_FADE: f32 = 0.05;
/// A one-shot clip in the preview plays again after this pause (seconds).
const PREVIEW_PAUSE: f32 = 0.8;

/// Constant-speed blend from `a` to `b` without turning `b` to the short side (the caller keeps it
/// on a side that does not jump from one step to the next).
fn slerp(a: Quat, b: Quat, t: f32) -> Quat {
    let dot = a.dot(b).clamp(-1.0, 1.0);
    let angle = dot.acos();
    let sin = angle.sin();
    if sin < 1e-3 {
        let mixed = a.lerp(b, t);
        return if mixed.length_squared() > 1e-6 { mixed.normalize() } else { b };
    }
    let q = a * (((1.0 - t) * angle).sin() / sin) + b * ((t * angle).sin() / sin);
    if q.is_finite() && q.length_squared() > 1e-6 { q.normalize() } else { b }
}

/// Smoothstep on 0..1 (clamped): eases in and out.
pub(crate) fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// How much each standing locomotion clip weighs at `speed` (idle, walk, jog, run); sums to 1.
fn standing_weights(speed: f32) -> [f32; 4] {
    let s = if speed.is_finite() { speed.max(0.0) } else { 0.0 };
    if s <= WALK_SPEED {
        let t = s / WALK_SPEED;
        [1.0 - t, t, 0.0, 0.0]
    } else if s <= JOG_SPEED {
        let t = (s - WALK_SPEED) / (JOG_SPEED - WALK_SPEED);
        [0.0, 1.0 - t, t, 0.0]
    } else if s <= RUN_SPEED {
        let t = (s - JOG_SPEED) / (RUN_SPEED - JOG_SPEED);
        [0.0, 0.0, 1.0 - t, t]
    } else {
        [0.0, 0.0, 0.0, 1.0]
    }
}

/// The locomotion clips, in the order of [`locomotion_weights`].
pub const LOCOMOTION: [ClipId; 6] = [ClipId::Idle, ClipId::Walk, ClipId::Jog, ClipId::Run, ClipId::CrouchIdle, ClipId::CrouchWalk];
const LOCOMOTION_SPEEDS: [f32; 6] = [0.0, WALK_SPEED, JOG_SPEED, RUN_SPEED, 0.0, CROUCH_WALK_SPEED];

/// How much each clip of [`LOCOMOTION`] weighs at `speed` (m/s) and `crouch` (0 standing, 1
/// crouched). The weights are never negative and sum to 1.
pub fn locomotion_weights(speed: f32, crouch: f32) -> [f32; 6] {
    let c = if crouch.is_finite() { crouch.clamp(0.0, 1.0) } else { 0.0 };
    let [idle, walk, jog, run] = standing_weights(speed);
    let s = if speed.is_finite() { speed.max(0.0) } else { 0.0 };
    let low = (s / CROUCH_WALK_SPEED).clamp(0.0, 1.0);
    [idle * (1.0 - c), walk * (1.0 - c), jog * (1.0 - c), run * (1.0 - c), (1.0 - low) * c, low * c]
}

#[derive(Clone, Debug, PartialEq)]
pub struct Animator {
    base: Base,
    overlay: Option<Overlay>,
    fade: Option<Fade>,
    /// Where the locomotion cycles are (0..1: left foot forward at 0, right at 0.5).
    phase: f32,
    idle_time: f32,
    crouch: Spring,
    /// The speed the gait blends by: follows the real speed closely, but never jumps.
    blend: Spring,
    /// Playback speed of a whole-body clip (climbing goes as fast as the hero climbs).
    rate: f32,
    lean: Spring,
    pitch: Spring,
    head_yaw: Spring,
    head_nod: Spring,
    cloak: Spring,
    cloak_tail: Spring,
    cloak_sway: Spring,
    facing: f32,
    speed: f32,
    air_time: f32,
    /// How fast the hero was falling while in the air (m/s, positive down).
    fall_speed: f32,
    weapon: Weapon,
    in_hand: bool,
    preview: Option<Preview>,
    crouch_input: bool,
    /// The layered pose of the last step and the one before (fades start from it).
    layered: Pose,
    layered_before: Pose,
    last_dt: f32,
    previous: Look,
    current: Look,
}

impl Default for Animator {
    fn default() -> Self {
        let rest = clip(ClipId::Idle).sample(0.0);
        let look = Look { pose: rest, ..Look::default() };
        Animator {
            base: Base::Locomotion,
            overlay: None,
            fade: None,
            phase: 0.0,
            idle_time: 0.0,
            crouch: Spring::default(),
            blend: Spring::default(),
            rate: 1.0,
            lean: Spring::default(),
            pitch: Spring::default(),
            head_yaw: Spring::default(),
            head_nod: Spring::default(),
            cloak: Spring::default(),
            cloak_tail: Spring::default(),
            cloak_sway: Spring::default(),
            facing: 0.0,
            speed: 0.0,
            air_time: 0.0,
            fall_speed: 0.0,
            weapon: Weapon::Sword,
            in_hand: false,
            preview: None,
            crouch_input: false,
            layered: rest,
            layered_before: rest,
            last_dt: crate::STEP_SECS as f32,
            previous: look,
            current: look,
        }
    }
}

impl Animator {
    pub fn new() -> Self {
        Self::default()
    }

    // ---- What the controller and combat call -------------------------------------------------

    /// Play a clip: a whole-body one replaces the base, an overlay one goes over it. Crossfades.
    pub fn play(&mut self, id: ClipId) {
        let clip = clip(id);
        let mirror = matches!(id, ClipId::WeaponDraw | ClipId::WeaponSheathe) && self.weapon == Weapon::Bow;
        let playing = Playing::new(id, mirror);
        self.begin_fade(clip.fade_in);
        match clip.layer {
            Layer::Base => {
                self.base = Base::Clip(playing);
                self.rate = 1.0;
            }
            Layer::Upper | Layer::Over => self.overlay = Some(Overlay { playing, weight: 1.0 }),
        }
    }

    /// Play `id` from `time` seconds in (the bow drawn from the nocked pose of aim mode).
    pub fn play_from(&mut self, id: ClipId, time: f32) {
        self.play(id);
        let time = if time.is_finite() { time.clamp(0.0, clip(id).duration) } else { 0.0 };
        match (&mut self.base, &mut self.overlay) {
            (_, Some(overlay)) if overlay.playing.clip == id => overlay.playing.time = time,
            (Base::Clip(playing), _) if playing.clip == id => playing.time = time,
            _ => {}
        }
    }

    /// Back to moving (from a held clip such as a fall to the ground or the bow aim).
    pub fn stop(&mut self) {
        self.begin_fade(0.2);
        self.base = Base::Locomotion;
        self.overlay = None;
    }

    /// The clip playing over the body (a draw, the bow, a flinch), if any.
    pub fn overlay_clip(&self) -> Option<ClipId> {
        self.overlay.map(|overlay| overlay.playing.clip)
    }

    /// Let go of an upper-body clip (the bow drawn) and keep the rest as it is.
    pub fn release(&mut self) {
        if self.overlay.is_some() {
            self.begin_fade(0.2);
            self.overlay = None;
        }
    }

    /// How fast a whole-body clip plays (1 is as authored; 0 holds it, as on a ladder standing still).
    pub fn set_rate(&mut self, rate: f32) {
        self.rate = if rate.is_finite() { rate.clamp(-3.0, 3.0) } else { 1.0 };
    }

    pub fn set_weapon(&mut self, weapon: Weapon, in_hand: bool) {
        self.weapon = weapon;
        self.in_hand = in_hand && weapon != Weapon::None;
    }

    pub fn weapon(&self) -> Weapon {
        self.weapon
    }

    /// Whether the weapon is in the hand (else on the back).
    pub fn in_hand(&self) -> bool {
        self.in_hand
    }

    /// Show one clip over and over, standing still (debug: the pose viewer). `None` ends it.
    pub fn preview(&mut self, clip: Option<ClipId>) {
        match clip {
            None => {
                if self.preview.take().is_some() {
                    self.stop();
                }
            }
            Some(id) => {
                self.preview = Some(Preview { clip: id, wait: 0.0 });
                self.start_preview(id);
            }
        }
    }

    pub fn previewing(&self) -> Option<ClipId> {
        self.preview.map(|preview| preview.clip)
    }

    /// The clip that shows most now (an overlay, a whole-body clip, or the heaviest locomotion one).
    pub fn current(&self) -> ClipId {
        if let Some(overlay) = &self.overlay
            && overlay.weight_now() > 0.5
        {
            return overlay.playing.clip;
        }
        match self.base {
            Base::Air(playing) | Base::Clip(playing) => playing.clip,
            Base::Locomotion => {
                let weights = locomotion_weights(self.blend.value, self.crouch.value);
                let mut best = 0;
                for (index, weight) in weights.iter().enumerate() {
                    if *weight > weights[best] {
                        best = index;
                    }
                }
                LOCOMOTION[best]
            }
        }
    }

    /// The pose `alpha` (0..1) of the way from the previous step to the last one.
    pub fn look(&self, alpha: f32) -> Look {
        let alpha = if alpha.is_finite() { alpha.clamp(0.0, 1.0) } else { 1.0 };
        let (a, b) = (&self.previous, &self.current);
        let tilt = if a.tilt.dot(b.tilt) < 0.0 { -b.tilt } else { b.tilt };
        Look {
            pose: a.pose.blend(&b.pose, alpha),
            tilt: a.tilt.lerp(tilt, alpha).normalize(),
            bow_draw: a.bow_draw + (b.bow_draw - a.bow_draw) * alpha,
        }
    }

    // ---- One step ----------------------------------------------------------------------------

    /// Advance `dt` seconds (a simulation step) with what the movement says.
    pub fn step(&mut self, drive: &Drive, dt: f32) {
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }
        let dt = dt.min(0.1);
        let mut drive = drive.sanitized();
        if self.preview.is_some() {
            drive = Drive { velocity: Vec3::ZERO, facing: drive.facing, grounded: true, crouch: false };
        }
        let speed = Vec3::new(drive.velocity.x, 0.0, drive.velocity.z).length();
        let vertical = drive.velocity.y;
        let turn = (wrap_pi(drive.facing - self.facing) / dt).clamp(-12.0, 12.0);
        let accel = ((speed - self.speed) / dt).clamp(-80.0, 80.0);

        if self.preview.is_some() {
            self.preview_step(dt);
        } else {
            self.transitions(&drive, vertical, dt);
        }
        self.crouch_input = drive.crouch;
        self.blend.step(speed, 18.0, 1.0, dt);
        let gait = self.blend.value.max(0.0);
        self.advance(dt, gait);

        let mut pose = self.layers(gait);
        if let Some(fade) = &mut self.fade {
            fade.elapsed += dt;
            let t = fade.elapsed / fade.duration.max(1e-3);
            pose = fade.blend(&pose, smooth(t));
            if t >= 1.0 {
                self.fade = None;
            }
        }
        self.layered_before = self.layered;
        self.layered = pose;
        self.last_dt = dt;

        let look = self.secondary(pose, &drive, Motion { speed, vertical, turn, accel }, dt);
        self.previous = self.current;
        self.current = look;
        self.facing = drive.facing;
        self.speed = speed;
    }

    /// Start a crossfade from what was shown last.
    fn begin_fade(&mut self, duration: f32) {
        let duration = if duration.is_finite() { duration.clamp(MIN_FADE, 1.0) } else { MIN_FADE };
        let dt = self.last_dt.max(1e-4);
        let mut spin = [Vec3::ZERO; BONES];
        for (index, spin) in spin.iter_mut().enumerate() {
            let delta = self.layered.bones[index] * self.layered_before.bones[index].inverse();
            let delta = if delta.w < 0.0 { -delta } else { delta };
            let axis = delta.to_scaled_axis() / dt;
            *spin = if axis.is_finite() { axis.clamp_length_max(15.0) } else { Vec3::ZERO };
        }
        let drift = (self.layered.root - self.layered_before.root) / dt;
        let drift = if drift.is_finite() { drift.clamp_length_max(5.0) } else { Vec3::ZERO };
        self.fade = Some(Fade { from: self.layered, spin, drift, elapsed: 0.0, duration, track: self.layered.bones });
    }

    fn set_base(&mut self, base: Base, fade: f32) {
        self.begin_fade(fade);
        self.base = base;
        self.rate = 1.0;
    }

    /// Ground, air and clip ends: the changes the movement makes by itself.
    fn transitions(&mut self, drive: &Drive, vertical: f32, dt: f32) {
        if !drive.grounded {
            self.fall_speed = (-vertical).max(0.0);
        }
        match self.base {
            Base::Locomotion => {
                if drive.grounded {
                    self.air_time = 0.0;
                } else {
                    self.air_time += dt;
                    if vertical > 1.5 {
                        self.set_base(Base::Air(Playing::new(ClipId::JumpUp, false)), clip(ClipId::JumpUp).fade_in);
                    } else if self.air_time > 0.15 {
                        self.set_base(Base::Air(Playing::new(ClipId::Fall, false)), clip(ClipId::Fall).fade_in);
                    }
                }
            }
            Base::Air(playing) => {
                if drive.grounded {
                    let impact = std::mem::take(&mut self.fall_speed);
                    self.air_time = 0.0;
                    self.set_base(Base::Locomotion, 0.1);
                    if impact > 3.0 {
                        let land = Playing::new(ClipId::Land, false);
                        self.overlay = Some(Overlay { playing: land, weight: (0.3 + (impact - 3.0) / 10.0).clamp(0.3, 1.0) });
                        self.head_nod.velocity -= (impact * 0.35).min(5.0);
                    }
                } else if playing.clip == ClipId::JumpUp && vertical < -1.0 {
                    self.set_base(Base::Air(Playing::new(ClipId::Fall, false)), 0.35);
                }
            }
            Base::Clip(playing) => {
                let clip = clip(playing.clip);
                if clip.mode == Mode::Once && playing.time >= clip.duration - clip.fade_out {
                    let next = if drive.grounded { Base::Locomotion } else { Base::Air(Playing::new(ClipId::Fall, false)) };
                    self.set_base(next, clip.fade_out);
                }
            }
        }
        if self.overlay.is_some_and(|overlay| overlay.playing.ended()) {
            // A `Once` overlay has faded to nothing by its end; a `Hold` one stays until stopped.
            if self.overlay.is_some_and(|overlay| clip(overlay.playing.clip).mode == Mode::Once) {
                self.overlay = None;
            }
        }
    }

    fn start_preview(&mut self, id: ClipId) {
        let weapon = match id {
            ClipId::Sword1 | ClipId::Sword2 | ClipId::Sword3 => Some((Weapon::Sword, true)),
            ClipId::SpearThrust => Some((Weapon::Spear, true)),
            ClipId::ClubSwing => Some((Weapon::Club, true)),
            ClipId::BowDraw | ClipId::BowShoot | ClipId::BowAim => Some((Weapon::Bow, true)),
            ClipId::WeaponDraw => Some((self.weapon_or_sword(), false)),
            ClipId::WeaponSheathe => Some((self.weapon_or_sword(), true)),
            _ => None,
        };
        if let Some((weapon, in_hand)) = weapon {
            self.set_weapon(weapon, in_hand);
        }
        let layer = clip(id).layer;
        if layer == Layer::Base {
            self.overlay = None;
            self.play(id);
        } else {
            self.begin_fade(0.2);
            self.base = Base::Locomotion;
            self.play(id);
        }
    }

    fn weapon_or_sword(&self) -> Weapon {
        if self.weapon == Weapon::None { Weapon::Sword } else { self.weapon }
    }

    /// In the preview a clip that stops plays again after a pause.
    fn preview_step(&mut self, dt: f32) {
        let Some(mut preview) = self.preview else {
            return;
        };
        let playing = match (self.base, self.overlay) {
            (Base::Clip(playing), _) => Some(playing),
            (_, Some(overlay)) => Some(overlay.playing),
            _ => None,
        };
        let ended = playing.is_none_or(|playing| playing.ended());
        if ended {
            preview.wait += dt;
            if preview.wait >= PREVIEW_PAUSE {
                preview.wait = 0.0;
                self.start_preview(preview.clip);
            }
        }
        self.preview = Some(preview);
    }

    fn advance(&mut self, dt: f32, speed: f32) {
        self.idle_time = (self.idle_time + dt) % 3600.0;
        let rate = self.rate;
        match &mut self.base {
            Base::Locomotion => {}
            Base::Air(playing) => playing.time += dt,
            Base::Clip(playing) => {
                playing.time += dt * rate;
                let clip = clip(playing.clip);
                if clip.mode == Mode::Loop {
                    playing.time = playing.time.rem_euclid(clip.duration.max(1e-3));
                } else {
                    playing.time = playing.time.clamp(0.0, clip.duration + 10.0);
                }
            }
        }
        if let Some(overlay) = &mut self.overlay {
            let before = overlay.playing.time;
            overlay.playing.time = (overlay.playing.time + dt).min(clip(overlay.playing.clip).duration + 10.0);
            if let Some(at) = clip(overlay.playing.clip).swap_at
                && before < at
                && overlay.playing.time >= at
            {
                match overlay.playing.clip {
                    ClipId::WeaponDraw => self.in_hand = self.weapon != Weapon::None,
                    ClipId::WeaponSheathe => self.in_hand = false,
                    _ => {}
                }
            }
        }
        let crouch = if self.preview.is_none() && self.crouch_input { 1.0 } else { 0.0 };
        self.crouch.step(crouch, 12.0, 1.0, dt);
        // The cycles share one phase, moving at the blended rate of the clips that move.
        let weights = locomotion_weights(speed, self.crouch.value);
        let (mut moving, mut frequency, mut nominal) = (0.0, 0.0, 0.0);
        for (index, weight) in weights.iter().enumerate() {
            if LOCOMOTION_SPEEDS[index] > 0.0 {
                moving += weight;
                frequency += weight / clip(LOCOMOTION[index]).duration.max(0.1);
                nominal += weight * LOCOMOTION_SPEEDS[index];
            }
        }
        if moving > 1e-4 {
            let ratio = (speed / (nominal / moving)).clamp(0.6, 1.5);
            self.phase = (self.phase + dt * frequency / moving * ratio).rem_euclid(1.0);
        }
    }

    /// The base and the overlay, layered (before the crossfade and secondary motion).
    fn layers(&self, speed: f32) -> Pose {
        let mut pose = match self.base {
            Base::Locomotion => self.locomotion(speed),
            Base::Air(playing) | Base::Clip(playing) => playing.sample(),
        };
        if let Some(overlay) = &self.overlay {
            let weight = overlay.weight_now();
            if weight > 0.0 {
                let layer = clip(overlay.playing.clip).layer;
                let weights = layer.mask().map(|w| w * weight);
                let root = if layer == Layer::Upper { 0.0 } else { weight };
                pose = pose.blend_masked(&overlay.playing.sample(), &weights, root);
            }
        }
        pose
    }

    fn locomotion(&self, speed: f32) -> Pose {
        let weights = locomotion_weights(speed, self.crouch.value);
        let mut parts = [(Pose::REST, 0.0); 6];
        for (index, part) in parts.iter_mut().enumerate() {
            let weight = weights[index];
            if weight <= 1e-4 {
                continue;
            }
            let clip = clip(LOCOMOTION[index]);
            let time = if LOCOMOTION_SPEEDS[index] > 0.0 { self.phase * clip.duration } else { self.idle_time };
            *part = (clip.sample(time), weight);
        }
        Pose::mix(&parts)
    }

    /// Lean, head and cloak on top of the layered pose.
    fn secondary(&mut self, mut pose: Pose, drive: &Drive, motion: Motion, dt: f32) -> Look {
        let Motion { speed, vertical, turn, accel } = motion;
        let moving_on_ground = self.base == Base::Locomotion && drive.grounded && self.preview.is_none();
        let ground = if moving_on_ground { 1.0 } else { 0.0 };
        self.lean.step((turn * speed * 0.045).clamp(-0.35, 0.35) * ground, 10.0, 1.0, dt);
        self.pitch.step((accel * 0.012).clamp(-0.1, 0.12) * ground, 8.0, 1.0, dt);
        self.head_yaw.step((turn * 0.12).clamp(-0.4, 0.4) * ground, 9.0, 0.8, dt);
        self.head_nod.step(0.0, 14.0, 0.45, dt);

        // How far the chest leans forward: the cloak hangs down from it, not along it.
        let matrices = pose.model_matrices();
        let up = matrices[Bone::Chest.index()].transform_vector3(Vec3::Y);
        let lean_forward = (-up.z).atan2(up.y).clamp(-1.0, 1.0);
        let falling = if drive.grounded { 0.0 } else { (-vertical).max(0.0) };
        let flare = -(speed * 0.055 + falling * 0.05).min(1.1);
        let flutter = (self.phase * std::f32::consts::TAU * 2.0).sin() * 0.05 * (speed / RUN_SPEED).min(1.0);
        self.cloak.step(flare + 0.35 * lean_forward, 9.0, 0.5, dt);
        self.cloak_tail.step(self.cloak.value * 0.6 + 0.55 * lean_forward + flutter, 7.0, 0.35, dt);
        self.cloak_sway.step((-turn * speed * 0.02).clamp(-0.5, 0.5), 8.0, 0.45, dt);

        let cloak = Bone::CloakUpper.index();
        pose.bones[cloak] = (Quat::from_rotation_z(self.cloak_sway.value) * Quat::from_rotation_x(self.cloak.value) * pose.bones[cloak]).normalize();
        let tail = Bone::CloakLower.index();
        pose.bones[tail] = (Quat::from_rotation_x((self.cloak_tail.value - self.cloak.value).clamp(-1.2, 1.2)) * pose.bones[tail]).normalize();
        let head = Bone::Head.index();
        pose.bones[head] = (Quat::from_rotation_y(-self.head_yaw.value)
            * Quat::from_rotation_x(self.head_nod.value.clamp(-0.6, 0.6))
            * Quat::from_rotation_z(self.lean.value * 0.5)
            * pose.bones[head])
            .normalize();
        let tilt = (Quat::from_rotation_z(-self.lean.value) * Quat::from_rotation_x(-self.pitch.value)).normalize();
        Look { pose, tilt, bow_draw: self.bow_draw() }
    }

    /// How far the bowstring is drawn (0..1).
    fn bow_draw(&self) -> f32 {
        let Some(overlay) = &self.overlay else {
            return 0.0;
        };
        match overlay.playing.clip {
            ClipId::BowDraw => smooth((overlay.playing.time - 0.25) / 0.35),
            ClipId::BowShoot => 1.0 - smooth(overlay.playing.time / 0.05),
            _ => 0.0,
        }
    }
}
