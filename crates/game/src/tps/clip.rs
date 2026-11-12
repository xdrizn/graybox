//! The clip format (G11.3): keyed bone rotations and a pelvis offset, with easing between keys.
//!
//! A clip is a list of [`Key`]s. Each key gives every bone's turn (Euler XYZ, degrees, the rule in
//! `rig.rs`) and the pelvis offset (metres) at one time. Between two keys the values follow the
//! first key's [`Ease`]: `Smooth` passes through the keys on a curve (Catmull-Rom, keeping the
//! speed through each key: cycles and follow-through), the others ease plainly from one key to the
//! next (anticipation holds, snaps). Angles are interpolated as numbers, so a key may turn a bone
//! more than half a turn from the previous one (a roll goes round 360°).
use super::rig::{BONES, Bone, Pose, lowest_sole};
use glam::{EulerRot, Quat, Vec3};

/// How the values go from a key to the next.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ease {
    /// A curve through the neighbouring keys (no stop at the key).
    Smooth,
    Linear,
    /// Starts slowly, ends fast (into an impact).
    In,
    /// Starts fast, ends slowly (out of an impact, into a hold).
    Out,
    /// Slow at both ends.
    InOut,
}

impl Ease {
    fn apply(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Ease::Smooth | Ease::Linear => t,
            Ease::In => t * t * t,
            Ease::Out => 1.0 - (1.0 - t).powi(3),
            Ease::InOut => t * t * (3.0 - 2.0 * t),
        }
    }
}

/// What happens at the end of a clip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Plays round and round (the last key runs into the first).
    Loop,
    /// Plays once, then the animator goes back to moving.
    Once,
    /// Plays once and keeps its last pose until something else is played.
    Hold,
}

/// Which bones a clip moves when played over the legs' own motion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    /// Replaces the whole body (attacks, rolls, climbing, falling down).
    Base,
    /// Over the moving body, from the waist up (drawing a weapon, the bow).
    Upper,
    /// Over the moving body, every bone, at a weight (a landing, a flinch).
    Over,
}

impl Layer {
    /// How much of each bone an overlay of this kind takes.
    pub fn mask(self) -> [f32; BONES] {
        let mut out = [1.0; BONES];
        if self == Layer::Upper {
            for bone in Bone::ALL {
                out[bone.index()] = match bone {
                    Bone::Pelvis | Bone::CloakUpper | Bone::CloakLower => 0.0,
                    Bone::Spine => 0.5,
                    leg if leg.is_leg() => 0.0,
                    _ => 1.0,
                };
            }
        }
        out
    }
}

/// Every clip the hero has.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ClipId {
    Idle,
    Walk,
    Jog,
    Run,
    CrouchIdle,
    CrouchWalk,
    JumpUp,
    Fall,
    Land,
    LandRoll,
    ClimbLadder,
    ClimbWall,
    Mantle,
    Sword1,
    Sword2,
    Sword3,
    SpearThrust,
    ClubSwing,
    BowDraw,
    BowShoot,
    HitReact,
    Knockdown,
    GetUp,
    Die,
    WeaponDraw,
    WeaponSheathe,
}

impl ClipId {
    pub const ALL: [ClipId; 26] = [
        ClipId::Idle,
        ClipId::Walk,
        ClipId::Jog,
        ClipId::Run,
        ClipId::CrouchIdle,
        ClipId::CrouchWalk,
        ClipId::JumpUp,
        ClipId::Fall,
        ClipId::Land,
        ClipId::LandRoll,
        ClipId::ClimbLadder,
        ClipId::ClimbWall,
        ClipId::Mantle,
        ClipId::Sword1,
        ClipId::Sword2,
        ClipId::Sword3,
        ClipId::SpearThrust,
        ClipId::ClubSwing,
        ClipId::BowDraw,
        ClipId::BowShoot,
        ClipId::HitReact,
        ClipId::Knockdown,
        ClipId::GetUp,
        ClipId::Die,
        ClipId::WeaponDraw,
        ClipId::WeaponSheathe,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    /// The name used by the control channel and the debug preview.
    pub fn name(self) -> &'static str {
        match self {
            ClipId::Idle => "idle",
            ClipId::Walk => "walk",
            ClipId::Jog => "jog",
            ClipId::Run => "run",
            ClipId::CrouchIdle => "crouchIdle",
            ClipId::CrouchWalk => "crouchWalk",
            ClipId::JumpUp => "jumpUp",
            ClipId::Fall => "fall",
            ClipId::Land => "land",
            ClipId::LandRoll => "landRoll",
            ClipId::ClimbLadder => "climbLadder",
            ClipId::ClimbWall => "climbWall",
            ClipId::Mantle => "mantle",
            ClipId::Sword1 => "sword1",
            ClipId::Sword2 => "sword2",
            ClipId::Sword3 => "sword3",
            ClipId::SpearThrust => "spearThrust",
            ClipId::ClubSwing => "clubSwing",
            ClipId::BowDraw => "bowDraw",
            ClipId::BowShoot => "bowShoot",
            ClipId::HitReact => "hitReact",
            ClipId::Knockdown => "knockdown",
            ClipId::GetUp => "getUp",
            ClipId::Die => "die",
            ClipId::WeaponDraw => "weaponDraw",
            ClipId::WeaponSheathe => "weaponSheathe",
        }
    }

    pub fn from_name(name: &str) -> Option<ClipId> {
        ClipId::ALL.into_iter().find(|clip| clip.name() == name)
    }
}

/// One key: the time (seconds from the start), how to go to the next key, the pelvis offset and
/// every bone's turn in degrees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Key {
    pub time: f32,
    pub ease: Ease,
    pub root: Vec3,
    pub angles: [Vec3; BONES],
}

impl Key {
    /// A key at `time` with every bone at rest, easing smoothly.
    pub fn at(time: f32) -> Key {
        Key { time, ease: Ease::Smooth, root: Vec3::ZERO, angles: [Vec3::ZERO; BONES] }
    }

    pub fn ease(mut self, ease: Ease) -> Key {
        self.ease = ease;
        self
    }

    /// Set one bone's turn (degrees).
    pub fn b(mut self, bone: Bone, angles: [f32; 3]) -> Key {
        self.angles[bone.index()] = Vec3::from_array(angles);
        self
    }

    /// Set a left bone and give the right one the mirrored turn (both sides alike).
    pub fn sym(self, left: Bone, angles: [f32; 3]) -> Key {
        let [x, y, z] = angles;
        self.b(left, angles).b(left.mirror(), [x, -y, -z])
    }

    /// The pelvis offset (metres; +Y up, -Z forward).
    pub fn root(mut self, root: [f32; 3]) -> Key {
        self.root = Vec3::from_array(root);
        self
    }

    /// Raise or lower the pelvis so the lowest sole is `lift` metres above the ground.
    pub fn plant(mut self, lift: f32) -> Key {
        self.root.y = 0.0;
        let lowest = lowest_sole(&self.pose().model_matrices());
        self.root.y = lift - lowest;
        self
    }

    /// The same key seen in a mirror (left and right swapped).
    pub fn mirrored(&self) -> Key {
        let mut out = *self;
        for bone in Bone::ALL {
            let v = self.angles[bone.mirror().index()];
            out.angles[bone.index()] = Vec3::new(v.x, -v.y, -v.z);
        }
        out.root.x = -self.root.x;
        out
    }

    pub fn pose(&self) -> Pose {
        pose_of(&self.angles, self.root)
    }
}

fn pose_of(angles: &[Vec3; BONES], root: Vec3) -> Pose {
    let mut pose = Pose { bones: [Quat::IDENTITY; BONES], root };
    for (bone, angle) in pose.bones.iter_mut().zip(angles) {
        let a = *angle * (std::f32::consts::PI / 180.0);
        if a.is_finite() {
            *bone = Quat::from_euler(EulerRot::XYZ, a.x, a.y, a.z);
        }
    }
    if !pose.root.is_finite() {
        pose.root = Vec3::ZERO;
    }
    pose
}

/// A clip: keys, how long it lasts, what happens at the end and how it blends in and out.
#[derive(Clone, Debug, PartialEq)]
pub struct Clip {
    pub id: ClipId,
    pub duration: f32,
    pub mode: Mode,
    pub layer: Layer,
    /// Crossfade into this clip and (for `Once`) out of it, seconds.
    pub fade_in: f32,
    pub fade_out: f32,
    /// For a weapon clip: when (seconds) the weapon moves between the back and the hand.
    pub swap_at: Option<f32>,
    pub keys: Vec<Key>,
}

impl Clip {
    pub fn new(id: ClipId, duration: f32, mode: Mode, keys: Vec<Key>) -> Clip {
        Clip { id, duration, mode, layer: Layer::Base, fade_in: 0.2, fade_out: 0.25, swap_at: None, keys }
    }

    pub fn layer(mut self, layer: Layer) -> Clip {
        self.layer = layer;
        self
    }

    pub fn fades(mut self, fade_in: f32, fade_out: f32) -> Clip {
        self.fade_in = fade_in;
        self.fade_out = fade_out;
        self
    }

    pub fn swap_at(mut self, time: f32) -> Clip {
        self.swap_at = Some(time);
        self
    }

    /// `time` brought inside the clip: wrapped for a loop, clamped otherwise.
    pub fn local_time(&self, time: f32) -> f32 {
        let duration = self.duration.max(1e-3);
        if !time.is_finite() {
            return 0.0;
        }
        match self.mode {
            Mode::Loop => time.rem_euclid(duration),
            Mode::Once | Mode::Hold => time.clamp(0.0, duration),
        }
    }

    /// The pose at `time` seconds.
    pub fn sample(&self, time: f32) -> Pose {
        let count = self.keys.len();
        let Some(first) = self.keys.first() else {
            return Pose::REST;
        };
        if count == 1 {
            return first.pose();
        }
        let t = self.local_time(time);
        let looping = self.mode == Mode::Loop;
        // The segment [keys[i], keys[i + 1]] holding `t` (for a loop the last runs into the first).
        let i = self.keys.iter().rposition(|key| key.time <= t).unwrap_or(0);
        let next = if i + 1 < count {
            Some(i + 1)
        } else if looping {
            Some(0)
        } else {
            None
        };
        let Some(j) = next else {
            return self.keys[i].pose();
        };
        let time_of = |index: usize, wraps: f32| self.keys[index].time + wraps * self.duration;
        let t1 = time_of(i, 0.0);
        let t2 = if j == 0 { time_of(0, 1.0) } else { time_of(j, 0.0) };
        let span = (t2 - t1).max(1e-4);
        let u = ((t - t1) / span).clamp(0.0, 1.0);
        let (a, b) = (&self.keys[i], &self.keys[j]);
        if a.ease != Ease::Smooth {
            let e = a.ease.apply(u);
            let mut angles = [Vec3::ZERO; BONES];
            for (index, angle) in angles.iter_mut().enumerate() {
                *angle = a.angles[index].lerp(b.angles[index], e);
            }
            return pose_of(&angles, a.root.lerp(b.root, e));
        }
        // Hermite through the neighbours, with tangents scaled to uneven key spacing. A clip that
        // stops (not a loop) starts and ends at rest speed.
        let gap_after = |index: usize| {
            if index + 1 < count {
                Some((index + 1, self.keys[index + 1].time - self.keys[index].time))
            } else if looping {
                Some((0, self.duration - self.keys[index].time + self.keys[0].time))
            } else {
                None
            }
        };
        let before = if i > 0 {
            Some((i - 1, t1 - (self.keys[i].time - self.keys[i - 1].time)))
        } else if looping {
            Some((count - 1, t1 - (self.duration - self.keys[count - 1].time + self.keys[0].time)))
        } else {
            None
        };
        let after = gap_after(j).map(|(index, gap)| (index, t2 + gap));
        let slope = |prev: Option<(usize, f32)>, next: Option<(usize, f32)>, pick: &dyn Fn(&Key) -> Vec3| -> Vec3 {
            match (prev, next) {
                (Some((p, pt)), Some((n, nt))) => (pick(&self.keys[n]) - pick(&self.keys[p])) / (nt - pt).max(1e-4),
                _ => Vec3::ZERO,
            }
        };
        let (u2, u3) = (u * u, u * u * u);
        let (h00, h10, h01, h11) = (2.0 * u3 - 3.0 * u2 + 1.0, u3 - 2.0 * u2 + u, -2.0 * u3 + 3.0 * u2, u3 - u2);
        let hermite = |pick: &dyn Fn(&Key) -> Vec3| -> Vec3 {
            let m1 = slope(before, Some((j, t2)), pick);
            let m2 = slope(Some((i, t1)), after, pick);
            pick(a) * h00 + m1 * (h10 * span) + pick(b) * h01 + m2 * (h11 * span)
        };
        let mut angles = [Vec3::ZERO; BONES];
        for (index, angle) in angles.iter_mut().enumerate() {
            *angle = hermite(&|key: &Key| key.angles[index]);
        }
        pose_of(&angles, hermite(&|key: &Key| key.root))
    }
}

/// A walking-style cycle from its first half: the second half is the first seen in a mirror, half
/// a cycle later (the right foot does what the left did).
pub fn cycle(duration: f32, half: Vec<Key>) -> Vec<Key> {
    let mut out = half.clone();
    out.extend(half.iter().map(|key| {
        let mut mirrored = key.mirrored();
        mirrored.time += duration * 0.5;
        mirrored
    }));
    out
}
