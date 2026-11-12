//! The hero's rigid-part skeleton (G11.3): 18 bones, each moving one group of meshes (no skinning).
//!
//! Model space: feet on Y = 0, the hero looks along -Z, its right hand is on +X (the same rule as
//! the camera: yaw 0 looks along -Z and strafing right goes to +X).
//!
//! Rotation rule for the clips (Euler XYZ, degrees, in the parent's frame at rest):
//! - a limb that hangs down (arms, legs, cloak) swings **forward** with +X, outwards to the hero's
//!   right with +Z (so the left arm lifts sideways with -Z), and twists with Y;
//! - a bone that points up (pelvis, spine, chest, head) bends **forward** with -X, tilts to the
//!   hero's left with +Z and turns to its left with +Y;
//! - a foot lifts its toes with +X.
use glam::{Mat4, Quat, Vec3};

/// The bones, parents before children.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Bone {
    Pelvis,
    Spine,
    Chest,
    Head,
    CloakUpper,
    CloakLower,
    UpperArmL,
    ForearmL,
    HandL,
    UpperArmR,
    ForearmR,
    HandR,
    ThighL,
    ShinL,
    FootL,
    ThighR,
    ShinR,
    FootR,
}

/// How many bones the rig has.
pub const BONES: usize = 18;
/// The renderer and the tests hold every pose in fixed arrays; a rig must stay under this.
pub const MAX_BONES: usize = 24;

impl Bone {
    pub const ALL: [Bone; BONES] = [
        Bone::Pelvis,
        Bone::Spine,
        Bone::Chest,
        Bone::Head,
        Bone::CloakUpper,
        Bone::CloakLower,
        Bone::UpperArmL,
        Bone::ForearmL,
        Bone::HandL,
        Bone::UpperArmR,
        Bone::ForearmR,
        Bone::HandR,
        Bone::ThighL,
        Bone::ShinL,
        Bone::FootL,
        Bone::ThighR,
        Bone::ShinR,
        Bone::FootR,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    /// The bone this one hangs from (None for the pelvis, the root).
    pub fn parent(self) -> Option<Bone> {
        use Bone::*;
        match self {
            Pelvis => None,
            Spine | ThighL | ThighR => Some(Pelvis),
            Chest => Some(Spine),
            Head | CloakUpper | UpperArmL | UpperArmR => Some(Chest),
            CloakLower => Some(CloakUpper),
            ForearmL => Some(UpperArmL),
            HandL => Some(ForearmL),
            ForearmR => Some(UpperArmR),
            HandR => Some(ForearmR),
            ShinL => Some(ThighL),
            FootL => Some(ShinL),
            ShinR => Some(ThighR),
            FootR => Some(ShinR),
        }
    }

    /// Where the bone's joint sits, from its parent's joint, at rest (metres, model axes).
    pub fn offset(self) -> Vec3 {
        use Bone::*;
        match self {
            Pelvis => Vec3::new(0.0, PELVIS_HEIGHT, 0.0),
            Spine => Vec3::new(0.0, 0.12, 0.0),
            Chest => Vec3::new(0.0, 0.18, 0.0),
            Head => Vec3::new(0.0, 0.24, 0.0),
            CloakUpper => Vec3::new(0.0, 0.2, 0.1),
            CloakLower => Vec3::new(0.0, -0.44, 0.05),
            UpperArmL => Vec3::new(-0.2, 0.18, 0.0),
            UpperArmR => Vec3::new(0.2, 0.18, 0.0),
            ForearmL | ForearmR => Vec3::new(0.0, -UPPER_ARM, 0.0),
            HandL | HandR => Vec3::new(0.0, -FOREARM, 0.0),
            ThighL => Vec3::new(-0.1, -0.07, 0.0),
            ThighR => Vec3::new(0.1, -0.07, 0.0),
            ShinL | ShinR => Vec3::new(0.0, -THIGH, 0.0),
            FootL | FootR => Vec3::new(0.0, -SHIN, 0.0),
        }
    }

    /// The same bone on the other side (itself for the middle ones).
    pub fn mirror(self) -> Bone {
        use Bone::*;
        match self {
            UpperArmL => UpperArmR,
            ForearmL => ForearmR,
            HandL => HandR,
            UpperArmR => UpperArmL,
            ForearmR => ForearmL,
            HandR => HandL,
            ThighL => ThighR,
            ShinL => ShinR,
            FootL => FootR,
            ThighR => ThighL,
            ShinR => ShinL,
            FootR => FootL,
            other => other,
        }
    }

    /// The six leg bones (an upper-body clip leaves them to the legs' own motion).
    pub fn is_leg(self) -> bool {
        matches!(self, Bone::ThighL | Bone::ShinL | Bone::FootL | Bone::ThighR | Bone::ShinR | Bone::FootR)
    }
}

/// Rest heights and lengths (metres). The ankle sits [`ANKLE_HEIGHT`] above the sole.
pub const PELVIS_HEIGHT: f32 = 0.95;
pub const UPPER_ARM: f32 = 0.27;
pub const FOREARM: f32 = 0.25;
pub const THIGH: f32 = 0.42;
pub const SHIN: f32 = 0.40;
pub const ANKLE_HEIGHT: f32 = 0.06;

/// One pose of the skeleton: each bone's rotation relative to its parent, and how far the pelvis
/// moved from where it stands at rest (model space).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub bones: [Quat; BONES],
    pub root: Vec3,
}

impl Default for Pose {
    fn default() -> Self {
        Self::REST
    }
}

/// `b` turned to the same hemisphere as `a`, so blending takes the short way round.
fn aligned(a: Quat, b: Quat) -> Quat {
    if a.dot(b) < 0.0 { -b } else { b }
}

fn finite_quat(q: Quat) -> Quat {
    if q.is_finite() && q.length_squared() > 1e-8 { q.normalize() } else { Quat::IDENTITY }
}

impl Pose {
    pub const REST: Pose = Pose { bones: [Quat::IDENTITY; BONES], root: Vec3::ZERO };

    pub fn rotation(&self, bone: Bone) -> Quat {
        self.bones[bone.index()]
    }

    /// The pose `t` (0..1) of the way to `other` (normalised lerp, the short way).
    pub fn blend(&self, other: &Pose, t: f32) -> Pose {
        let t = if t.is_finite() { t.clamp(0.0, 1.0) } else { 1.0 };
        let mut out = *self;
        for (index, bone) in out.bones.iter_mut().enumerate() {
            let to = aligned(*bone, other.bones[index]);
            *bone = finite_quat(bone.lerp(to, t));
        }
        out.root = self.root.lerp(other.root, t);
        if !out.root.is_finite() {
            out.root = Vec3::ZERO;
        }
        out
    }

    /// Blend towards `other` bone by bone, `weights[bone]` of the way (0..1 each).
    pub fn blend_masked(&self, other: &Pose, weights: &[f32; BONES], root: f32) -> Pose {
        let mut out = *self;
        for (index, bone) in out.bones.iter_mut().enumerate() {
            let t = weights[index].clamp(0.0, 1.0);
            if t > 0.0 {
                let to = aligned(*bone, other.bones[index]);
                *bone = finite_quat(bone.lerp(to, t));
            }
        }
        out.root = self.root.lerp(other.root, root.clamp(0.0, 1.0));
        if !out.root.is_finite() {
            out.root = Vec3::ZERO;
        }
        out
    }

    /// The weighted average of several poses (weights need not sum to 1; zero weights are skipped).
    pub fn mix(parts: &[(Pose, f32)]) -> Pose {
        let total: f32 = parts.iter().map(|(_, w)| w.max(0.0)).filter(|w| w.is_finite()).sum();
        if total <= 1e-6 {
            return Pose::REST;
        }
        let mut bones = [Quat::from_xyzw(0.0, 0.0, 0.0, 0.0); BONES];
        let mut root = Vec3::ZERO;
        let reference = parts.iter().find(|(_, w)| *w > 0.0).map(|(p, _)| p.bones).unwrap_or([Quat::IDENTITY; BONES]);
        for (pose, weight) in parts {
            let weight = if weight.is_finite() { weight.max(0.0) / total } else { 0.0 };
            if weight <= 0.0 {
                continue;
            }
            for (index, sum) in bones.iter_mut().enumerate() {
                let q = aligned(reference[index], pose.bones[index]);
                *sum = Quat::from_vec4(glam::Vec4::from(*sum) + glam::Vec4::from(q) * weight);
            }
            root += pose.root * weight;
        }
        Pose { bones: bones.map(finite_quat), root: if root.is_finite() { root } else { Vec3::ZERO } }
    }

    /// The pose seen in a mirror (left and right swapped).
    pub fn mirrored(&self) -> Pose {
        let mut out = *self;
        for bone in Bone::ALL {
            let q = self.bones[bone.mirror().index()];
            // Reflecting across the YZ plane keeps the X turn and flips the Y and Z turns.
            out.bones[bone.index()] = Quat::from_xyzw(q.x, -q.y, -q.z, q.w);
        }
        out.root.x = -self.root.x;
        out
    }

    /// Each bone's transform in model space (feet at the origin, before the hero is placed).
    pub fn model_matrices(&self) -> [Mat4; BONES] {
        let mut out = [Mat4::IDENTITY; BONES];
        for bone in Bone::ALL {
            let mut local = Mat4::from_rotation_translation(finite_quat(self.bones[bone.index()]), bone.offset());
            if bone == Bone::Pelvis {
                let root = if self.root.is_finite() { self.root } else { Vec3::ZERO };
                local = Mat4::from_rotation_translation(finite_quat(self.bones[0]), bone.offset() + root);
            }
            out[bone.index()] = match bone.parent() {
                // Parents come first in `Bone::ALL`, so theirs is already set.
                Some(parent) => out[parent.index()] * local,
                None => local,
            };
        }
        out
    }

    /// The largest turn any bone makes between two poses (radians), and how far the pelvis moves.
    pub fn distance(&self, other: &Pose) -> (f32, f32) {
        let mut angle: f32 = 0.0;
        for index in 0..BONES {
            let delta = self.bones[index] * other.bones[index].inverse();
            angle = angle.max(2.0 * delta.xyz().length().atan2(delta.w.abs()));
        }
        (angle, (self.root - other.root).length())
    }
}

/// The sole runs this far in front of the ankle (toe) and behind it (heel), metres.
pub const TOE: f32 = 0.18;
pub const HEEL: f32 = 0.06;

/// The height of the lowest point of either sole (heel or toe), model space.
pub fn lowest_sole(matrices: &[Mat4; BONES]) -> f32 {
    let mut lowest = f32::INFINITY;
    for foot in [Bone::FootL, Bone::FootR] {
        for z in [-TOE, HEEL] {
            lowest = lowest.min(matrices[foot.index()].transform_point3(Vec3::new(0.0, -ANKLE_HEIGHT, z)).y);
        }
    }
    if lowest.is_finite() { lowest } else { 0.0 }
}

/// The model-space position of the ankle of the left (`false`) or right foot.
pub fn ankle(matrices: &[Mat4; BONES], right: bool) -> Vec3 {
    let foot = if right { Bone::FootR } else { Bone::FootL };
    matrices[foot.index()].transform_point3(Vec3::ZERO)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rig_is_small_and_parents_come_first() {
        const { assert!(BONES <= MAX_BONES) };
        assert_eq!(Bone::ALL.len(), BONES);
        for (index, bone) in Bone::ALL.iter().enumerate() {
            assert_eq!(bone.index(), index);
            if let Some(parent) = bone.parent() {
                assert!(parent.index() < index, "{bone:?} comes after its parent");
            }
            assert_eq!(bone.mirror().mirror(), *bone);
        }
    }

    #[test]
    fn at_rest_the_hero_stands_on_the_ground_about_1_7_m_tall() {
        let m = Pose::REST.model_matrices();
        for right in [false, true] {
            assert!((ankle(&m, right).y - ANKLE_HEIGHT).abs() < 0.01, "ankle {:?}", ankle(&m, right));
        }
        let head = m[Bone::Head.index()].transform_point3(Vec3::ZERO);
        assert!((1.45..1.55).contains(&head.y), "neck at {}", head.y);
        let hand = m[Bone::HandR.index()].transform_point3(Vec3::ZERO);
        assert!(hand.x > 0.1, "the right hand is on +X");
    }

    #[test]
    fn mixing_and_mirroring_keep_poses_valid() {
        let mut bent = Pose::REST;
        bent.bones[Bone::ThighL.index()] = Quat::from_rotation_x(0.6);
        bent.bones[Bone::Chest.index()] = Quat::from_rotation_y(0.4);
        let half = Pose::mix(&[(Pose::REST, 1.0), (bent, 1.0)]);
        let (angle, _) = half.distance(&Pose::REST);
        assert!((angle - 0.3).abs() < 0.01, "{angle}");
        let mirrored = bent.mirrored();
        assert!((mirrored.rotation(Bone::ThighR).to_euler(glam::EulerRot::XYZ).0 - 0.6).abs() < 1e-4);
        assert!((mirrored.rotation(Bone::Chest).to_euler(glam::EulerRot::XYZ).1 + 0.4).abs() < 1e-4);
        assert_eq!(Pose::mix(&[]), Pose::REST);
        assert_eq!(Pose::mix(&[(bent, f32::NAN)]), Pose::REST);
    }
}
