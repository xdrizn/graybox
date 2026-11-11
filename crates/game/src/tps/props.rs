//! The things standing in the valley, as data: repeated props (drawn instanced), solid boxes the hero
//! cannot walk through, and tags for what can be climbed.
use glam::Vec3;
use std::f32::consts::FRAC_PI_2;

/// The shapes drawn many times over. The renderer holds one mesh per kind and an instance per [`Prop`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropKind {
    /// A tall faceted tree with a stacked, pointed crown.
    Pine,
    /// A round-crowned tree.
    Oak,
    /// A faceted boulder (scaled from a pebble to a big rock).
    Rock,
    Pillar,
    BrokenPillar,
    /// A pointed ruined arch. Spans along local X.
    Arch,
    /// A wooden crate you can climb onto.
    Crate,
    /// A heavy stone block you can climb onto.
    Block,
}

impl PropKind {
    pub const ALL: [PropKind; 8] =
        [PropKind::Pine, PropKind::Oak, PropKind::Rock, PropKind::Pillar, PropKind::BrokenPillar, PropKind::Arch, PropKind::Crate, PropKind::Block];

    /// This kind's place in [`PropKind::ALL`].
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|kind| *kind == self).unwrap_or(0)
    }
}

/// Where the arch's two legs stand (metres from its middle along local X), and how thick and tall
/// they are. The renderer's mesh uses the same numbers.
pub const ARCH_LEG_X: f32 = 2.0;
pub const ARCH_LEG_SIZE: f32 = 1.2;
pub const ARCH_LEG_HEIGHT: f32 = 3.4;
pub const PILLAR_SIZE: f32 = 1.1;
pub const PILLAR_HEIGHT: f32 = 5.0;
pub const BROKEN_PILLAR_HEIGHT: f32 = 2.3;
pub const CRATE_SIZE: f32 = 1.1;
pub const BLOCK_SIZE: f32 = 1.6;

/// One instance of a [`PropKind`]. `position` is on the ground (the foot of the prop).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Prop {
    pub kind: PropKind,
    pub position: Vec3,
    /// Turn about the vertical axis (radians). Props that block the hero (pillars, arches, crates,
    /// blocks) are only ever turned by quarter turns, so their boxes are exact.
    pub yaw: f32,
    pub scale: f32,
}

/// An axis-aligned box the hero cannot walk through.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Solid {
    pub min: Vec3,
    pub max: Vec3,
}

impl Solid {
    /// A box with its feet-centre at `base`, `size` wide and deep and `height` tall.
    pub fn standing(base: Vec3, size: f32, height: f32) -> Self {
        let half = Vec3::new(size * 0.5, 0.0, size * 0.5);
        Self { min: base - half, max: base + half + Vec3::Y * height }
    }

    /// A box around `center` with the full extents `size`.
    pub fn around(center: Vec3, size: Vec3) -> Self {
        Self { min: center - size * 0.5, max: center + size * 0.5 }
    }

    pub fn is_finite(&self) -> bool {
        self.min.is_finite() && self.max.is_finite() && self.min.cmplt(self.max).all()
    }
}

/// What kind of climbing a tagged volume allows (G11.4 reads these).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClimbKind {
    /// Walk into it to climb up and down; `min`..`max` is the strip the hero hangs in.
    Ladder,
    /// A ledge at about chest height: vault or mantle onto its top. `min`..`max` is the whole solid.
    Mantle,
    /// A rock face with handholds, climbed on a grid and costing stamina.
    Wall,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Climbable {
    pub kind: ClimbKind,
    pub min: Vec3,
    pub max: Vec3,
    /// The way the climbable faces: where the hero stands to start (a unit axis; zero for a mantle
    /// that can be climbed from any side).
    pub normal: Vec3,
}

/// Whether a yaw is an odd number of quarter turns (so a box's width and depth swap).
fn quarter_swapped(yaw: f32) -> bool {
    ((yaw / FRAC_PI_2).round() as i64).rem_euclid(2) == 1
}

impl Prop {
    /// The boxes that stop the hero: a trunk, a pillar, a boulder's body, an arch's two legs.
    pub fn solids(&self) -> Vec<Solid> {
        let s = self.scale;
        match self.kind {
            PropKind::Pine => vec![Solid::standing(self.position, 0.7 * s, 3.2 * s)],
            PropKind::Oak => vec![Solid::standing(self.position, 0.8 * s, 3.0 * s)],
            PropKind::Rock => vec![Solid::standing(self.position - Vec3::Y * 0.2 * s, 1.5 * s, 1.1 * s)],
            PropKind::Pillar => vec![Solid::standing(self.position, PILLAR_SIZE * s, PILLAR_HEIGHT * s)],
            PropKind::BrokenPillar => vec![Solid::standing(self.position, PILLAR_SIZE * s, BROKEN_PILLAR_HEIGHT * s)],
            PropKind::Crate => vec![Solid::standing(self.position, CRATE_SIZE * s, CRATE_SIZE * s)],
            PropKind::Block => vec![Solid::standing(self.position, BLOCK_SIZE * s, BLOCK_SIZE * s)],
            PropKind::Arch => {
                let along = if quarter_swapped(self.yaw) { Vec3::Z } else { Vec3::X };
                [-1.0, 1.0]
                    .map(|side| Solid::standing(self.position + along * ARCH_LEG_X * s * side, ARCH_LEG_SIZE * s, ARCH_LEG_HEIGHT * s))
                    .to_vec()
            }
        }
    }

    /// The tag for a prop you can climb onto, if it is one.
    pub fn climbable(&self) -> Option<Climbable> {
        matches!(self.kind, PropKind::Crate | PropKind::Block).then(|| {
            let solid = self.solids().first().copied().unwrap_or(Solid { min: self.position, max: self.position });
            Climbable { kind: ClimbKind::Mantle, min: solid.min, max: solid.max, normal: Vec3::ZERO }
        })
    }
}
