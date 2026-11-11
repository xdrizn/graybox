//! Where everything stands: the hand-placed ruins, the watchtower, the ladders and the climb wall,
//! then the scattered trees and rocks around them. Always the same, because the only randomness is
//! a fixed seed.
use super::props::{ClimbKind, Climbable, Prop, PropKind, Solid};
use super::terrain::{HALF_EXTENT, RIVER_END, RIVER_HALF_WIDTH, Terrain, ZONES, cliff_z, patch, river_z};
use crate::arena::{Primitive, SceneObject, random};
use glam::Vec3;
use std::f32::consts::FRAC_PI_2;

/// The most trees and rocks the valley holds (the budgets assume these).
pub const TREE_MAX: usize = 190;
pub const ROCK_MAX: usize = 120;

/// Where the hero starts (x, z); the ground there is a small flat pad.
pub const SPAWN_XZ: [f32; 2] = [2.0, 52.0];
const TOWER_XZ: [f32; 2] = [-10.0, 6.0];
const COURTYARD_XZ: [f32; 2] = [34.0, 10.0];
const PLATEAU_RUIN_XZ: [f32; 2] = [-64.0, 8.0];
const TERRACE_XZ: [f32; 2] = [54.0, 36.0];
const BUTTRESS_X: f32 = 24.0;

/// Everything the layout makes.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Layout {
    pub props: Vec<Prop>,
    pub solids: Vec<Solid>,
    pub climbables: Vec<Climbable>,
    pub structures: Vec<SceneObject>,
    pub backdrop: Vec<SceneObject>,
    pub beacon: Vec3,
}

struct Builder<'a> {
    terrain: &'a Terrain,
    out: Layout,
    /// Circles (x, z, radius) kept clear of trees and rocks.
    clear: Vec<[f32; 3]>,
}

fn object(position: Vec3, size: Vec3, tone: u32, emissive: f32) -> SceneObject {
    SceneObject { position, size, rotation: Vec3::ZERO, tone, emissive, roughness: 0.9, collision: false, primitive: Primitive::Box }
}

/// Whether `x`/`z` is near the river's water (within `margin` of its ribbon).
pub(super) fn near_river(x: f32, z: f32, margin: f32) -> bool {
    x.abs() < RIVER_END + 6.0 && (z - river_z(x)).abs() < RIVER_HALF_WIDTH + margin
}

impl<'a> Builder<'a> {
    fn new(terrain: &'a Terrain) -> Self {
        let mut clear: Vec<[f32; 3]> = ZONES.iter().map(|zone| [zone.x, zone.z, zone.radius + 4.0]).collect();
        clear.push([SPAWN_XZ[0], SPAWN_XZ[1], 9.0]);
        clear.push([BUTTRESS_X, cliff_z(BUTTRESS_X), 6.0]);
        Self { terrain, out: Layout::default(), clear }
    }

    fn ground(&self, x: f32, z: f32) -> f32 {
        self.terrain.height(x, z)
    }

    /// A box of the world; `solid` also stops the hero. Returns the solid's index when it is one.
    fn slab(&mut self, center: Vec3, size: Vec3, tone: u32, solid: bool) {
        self.out.structures.push(object(center, size, tone, 0.0));
        if solid {
            self.out.solids.push(Solid::around(center, size));
        }
    }

    /// A box that you can mantle onto (and that stops the hero).
    fn ledge(&mut self, center: Vec3, size: Vec3, tone: u32) {
        self.slab(center, size, tone, true);
        let solid = Solid::around(center, size);
        self.out.climbables.push(Climbable { kind: ClimbKind::Mantle, min: solid.min, max: solid.max, normal: Vec3::ZERO });
    }

    fn glow(&mut self, center: Vec3, size: Vec3, emissive: f32) {
        self.out.structures.push(object(center, size, 0, emissive));
    }

    fn prop_at(&mut self, kind: PropKind, position: Vec3, yaw: f32, scale: f32) {
        let prop = Prop { kind, position, yaw, scale };
        self.out.solids.extend(prop.solids());
        self.out.climbables.extend(prop.climbable());
        self.out.props.push(prop);
    }

    fn prop(&mut self, kind: PropKind, x: f32, z: f32, yaw: f32, scale: f32) {
        let position = Vec3::new(x, self.ground(x, z), z);
        self.prop_at(kind, position, yaw, scale);
    }

    /// A wooden ladder standing on `bottom`, `height` tall, against a wall that is behind it;
    /// `face` is the unit axis pointing away from the wall.
    fn ladder(&mut self, bottom: Vec3, face: Vec3, height: f32) {
        let side = Vec3::new(face.z, 0.0, -face.x);
        let base = bottom + face * 0.15;
        for sign in [-1.0, 1.0] {
            self.out.structures.push(object(base + side * 0.32 * sign + Vec3::Y * height * 0.5, Vec3::new(0.07, height, 0.07), 4, 0.0));
        }
        let rung = Vec3::new(0.05 + side.x.abs() * 0.59, 0.05, 0.05 + side.z.abs() * 0.59);
        let mut y = 0.35;
        while y < height - 0.05 {
            self.out.structures.push(object(base + Vec3::Y * y, rung, 4, 0.0));
            y += 0.4;
        }
        let half = Vec3::new(0.42 * side.x.abs() + 0.3 * face.x.abs(), 0.0, 0.42 * side.z.abs() + 0.3 * face.z.abs());
        let middle = bottom + face * 0.2;
        self.out.climbables.push(Climbable { kind: ClimbKind::Ladder, min: middle - half, max: middle + half + Vec3::Y * height, normal: face });
    }

    /// The watchtower: a stone base, a shaft with orange slits, a platform with a beacon, a ladder.
    fn watchtower(&mut self) {
        let [x, z] = TOWER_XZ;
        let g = self.ground(x, z);
        self.slab(Vec3::new(x, g + 0.6, z), Vec3::new(5.2, 1.6, 5.2), 2, true);
        self.slab(Vec3::new(x, g + 5.6, z), Vec3::new(3.6, 9.0, 3.6), 3, true);
        self.slab(Vec3::new(x, g + 10.3, z), Vec3::new(5.4, 0.5, 5.4), 2, true);
        let top = g + 10.55;
        for side in [-1.0, 1.0] {
            self.slab(Vec3::new(x + 2.5 * side, top + 0.45, z), Vec3::new(0.4, 0.9, 5.4), 2, true);
        }
        self.slab(Vec3::new(x, top + 0.45, z - 2.5), Vec3::new(4.6, 0.9, 0.4), 2, true);
        // The south parapet leaves a gap above the ladder.
        for side in [-1.0, 1.0] {
            self.slab(Vec3::new(x + 1.7 * side, top + 0.45, z + 2.5), Vec3::new(1.2, 0.9, 0.4), 2, true);
        }
        self.slab(Vec3::new(x, top + 0.3, z), Vec3::new(1.0, 0.6, 1.0), 1, true);
        self.slab(Vec3::new(x, top + 0.9, z), Vec3::new(0.5, 0.5, 0.5), 5, false);
        let beacon = Vec3::new(x, top + 1.6, z);
        self.out.beacon = beacon;
        self.out.structures.push(SceneObject { primitive: Primitive::Octahedron, ..object(beacon, Vec3::new(1.1, 1.6, 1.1), 0, 2.4) });
        for (dx, dy, dz, size) in [(-0.8, 4.0, 1.81, [0.25, 1.2, 0.04]), (0.7, 7.0, 1.81, [0.25, 1.2, 0.04]), (1.81, 5.5, 0.0, [0.04, 1.2, 0.25])] {
            self.glow(Vec3::new(x + dx, g + dy, z + dz), Vec3::from_array(size), 1.6);
        }
        self.ladder(Vec3::new(x, g, z + 1.8), Vec3::Z, 10.55);
        self.ledge(Vec3::new(x - 4.2, g + 0.5, z + 1.5), Vec3::new(1.8, 1.0, 1.8), 3);
    }

    /// A stone buttress built into the cliff, with a ladder up its face to the meadow.
    fn buttress(&mut self) {
        let zc = cliff_z(BUTTRESS_X);
        let top = self.ground(BUTTRESS_X, zc + 3.6);
        let foot = self.ground(BUTTRESS_X, zc - 3.6);
        let low = foot - 0.4;
        self.slab(Vec3::new(BUTTRESS_X, (top + low) * 0.5, zc), Vec3::new(4.4, top - low, 6.0), 3, true);
        self.ladder(Vec3::new(BUTTRESS_X, foot, zc - 3.0), -Vec3::Z, top - foot);
        // Posts on top mark the way up to the meadow.
        self.glow(Vec3::new(BUTTRESS_X - 1.6, top + 0.15, zc - 2.4), Vec3::new(0.3, 0.3, 0.3), 1.6);
        self.glow(Vec3::new(BUTTRESS_X + 1.6, top + 0.15, zc - 2.4), Vec3::new(0.3, 0.3, 0.3), 1.6);
    }

    /// Flat paving stones, 3 × 3.
    fn paving(&mut self, x: f32, z: f32, cells: i32, step: f32) {
        let g = self.ground(x, z);
        let from = -(cells - 1) as f32 * 0.5;
        for a in 0..cells {
            for b in 0..cells {
                let center = Vec3::new(x + (from + a as f32) * step, g, z + (from + b as f32) * step);
                self.slab(center, Vec3::new(step - 0.25, 0.1, step - 0.25), 2, false);
            }
        }
    }

    /// A ring of pillars (some broken) and arches around a point: (dx, dz, kind, yaw).
    fn ruin_ring(&mut self, [x, z]: [f32; 2], pieces: &[(f32, f32, PropKind, f32)]) {
        for (dx, dz, kind, yaw) in pieces {
            self.prop(*kind, x + dx, z + dz, *yaw, 1.0);
        }
    }

    fn courtyard(&mut self) {
        let [x, z] = COURTYARD_XZ;
        self.paving(x, z, 3, 3.5);
        use PropKind::{Arch, BrokenPillar, Pillar};
        self.ruin_ring(
            COURTYARD_XZ,
            &[
                (-8.0, 0.0, Pillar, 0.0),
                (0.0, 8.0, BrokenPillar, 0.0),
                (5.7, 5.7, Pillar, 0.0),
                (-5.7, 5.7, BrokenPillar, 0.0),
                (5.7, -5.7, BrokenPillar, 0.0),
                (-5.7, -5.7, Pillar, 0.0),
                (0.0, -8.0, Arch, 0.0),
                (8.0, 0.0, Arch, FRAC_PI_2),
            ],
        );
        // Low wall pieces along the south-east, each one a ledge to mantle onto.
        for (dx, dz, w, d) in [(6.0, 7.5, 4.0, 0.8), (9.6, 4.0, 0.8, 3.4)] {
            let g = self.ground(x + dx, z + dz);
            self.ledge(Vec3::new(x + dx, g + 0.65, z + dz), Vec3::new(w, 1.5, d), 2);
        }
        // Crates: a stack of two and a single one, to climb up by.
        self.prop(PropKind::Crate, x - 3.0, z + 2.0, 0.0, 1.0);
        let stacked = Vec3::new(x - 3.0, self.ground(x - 3.0, z + 2.0) + 1.1, z + 2.0);
        self.prop_at(PropKind::Crate, stacked, 0.0, 1.0);
        self.prop(PropKind::Crate, x - 1.8, z + 2.2, 0.0, 1.0);
        self.climb_wall(x, z + 9.2);
    }

    /// A rock face with handholds and a ledge on top.
    fn climb_wall(&mut self, x: f32, z: f32) {
        let g = self.ground(x, z);
        let (width, height, thick) = (9.0, 7.0, 1.6);
        self.slab(Vec3::new(x, g + height * 0.5, z), Vec3::new(width, height, thick), 3, true);
        self.slab(Vec3::new(x, g + height + 0.15, z), Vec3::new(width + 0.6, 0.3, thick + 0.6), 2, true);
        let face = z - thick * 0.5;
        for row in 0..5 {
            for column in 0..4 {
                let stagger = if row % 2 == 0 { 0.0 } else { 0.9 };
                let hx = x - 3.0 + column as f32 * 2.0 + stagger;
                self.slab(Vec3::new(hx, g + 0.9 + row as f32 * 1.25, face - 0.12), Vec3::new(0.55, 0.22, 0.28), 5, false);
            }
        }
        let normal = -Vec3::Z;
        self.out.climbables.push(Climbable {
            kind: ClimbKind::Wall,
            min: Vec3::new(x - width * 0.5, g, face - 0.4),
            max: Vec3::new(x + width * 0.5, g + height, face),
            normal,
        });
    }

    fn plateau_ruin(&mut self) {
        let [x, z] = PLATEAU_RUIN_XZ;
        self.paving(x, z, 2, 3.6);
        use PropKind::{Arch, BrokenPillar, Pillar};
        self.ruin_ring(
            PLATEAU_RUIN_XZ,
            &[
                (-7.0, 0.0, Pillar, 0.0),
                (0.0, 7.0, BrokenPillar, 0.0),
                (0.0, -7.0, Pillar, 0.0),
                (7.0, 0.0, Arch, FRAC_PI_2),
                (-4.5, 5.0, BrokenPillar, 0.0),
            ],
        );
        self.prop(PropKind::Block, x + 3.0, z + 3.0, 0.0, 1.0);
    }

    fn terrace(&mut self) {
        let [x, z] = TERRACE_XZ;
        self.paving(x, z, 2, 3.6);
        use PropKind::{Arch, BrokenPillar, Pillar};
        self.ruin_ring(
            TERRACE_XZ,
            &[
                (-6.0, -6.0, Pillar, 0.0),
                (6.0, -6.0, BrokenPillar, 0.0),
                (-6.0, 6.0, BrokenPillar, 0.0),
                (6.0, 6.0, Pillar, 0.0),
                (0.0, -7.5, Arch, 0.0),
            ],
        );
        self.prop(PropKind::Crate, x + 2.0, z + 1.0, 0.0, 1.0);
        self.prop(PropKind::Block, x - 3.0, z + 2.5, 0.0, 1.0);
    }

    /// Whether a tree or rock may stand at a point.
    fn open_ground(&self, x: f32, z: f32, margin: f32) -> bool {
        let inside = HALF_EXTENT - 10.0;
        if x.abs() > inside || z.abs() > inside || near_river(x, z, margin + 1.0) || self.terrain.slope(x, z) > 0.55 {
            return false;
        }
        self.clear.iter().all(|[cx, cz, radius]| (x - cx).hypot(z - cz) > *radius)
    }

    fn trees(&mut self, seed: &mut u32) {
        let mut spots: Vec<[f32; 2]> = Vec::with_capacity(TREE_MAX);
        for _ in 0..6000 {
            if spots.len() >= TREE_MAX {
                break;
            }
            let x = (random(seed) * 2.0 - 1.0) * (HALF_EXTENT - 8.0);
            let z = (random(seed) * 2.0 - 1.0) * (HALF_EXTENT - 8.0);
            let forest = patch(x, z, 0.045, 11);
            let roll = random(seed);
            if !self.open_ground(x, z, 1.5) || (forest < 0.55 && roll > 0.06) {
                continue;
            }
            if spots.iter().any(|[sx, sz]| (x - sx).hypot(z - sz) < 3.4) {
                continue;
            }
            spots.push([x, z]);
            let high = self.ground(x, z) > 7.5 || patch(x, z, 0.09, 12) < 0.5;
            let kind = if high { PropKind::Pine } else { PropKind::Oak };
            let (yaw, scale) = (random(seed) * std::f32::consts::TAU, 0.8 + random(seed) * 0.7);
            self.prop(kind, x, z, yaw, scale);
        }
    }

    fn rocks(&mut self, seed: &mut u32) {
        let mut placed = 0;
        // Along the river banks, then scattered about, more of them where the ground is steeper.
        let mut x = -RIVER_END + 4.0;
        while x < RIVER_END && placed < ROCK_MAX / 3 {
            let side = if random(seed) < 0.5 { -1.0 } else { 1.0 };
            let z = river_z(x) + side * (RIVER_HALF_WIDTH + 1.0 + random(seed) * 2.5);
            let scale = 0.5 + random(seed) * 1.3;
            if self.open_ground(x, z, 0.0) {
                self.prop(PropKind::Rock, x, z, random(seed) * std::f32::consts::TAU, scale);
                placed += 1;
            }
            x += 3.0 + random(seed) * 5.0;
        }
        for _ in 0..4000 {
            if placed >= ROCK_MAX {
                break;
            }
            let x = (random(seed) * 2.0 - 1.0) * (HALF_EXTENT - 8.0);
            let z = (random(seed) * 2.0 - 1.0) * (HALF_EXTENT - 8.0);
            let bias = patch(x, z, 0.07, 13);
            let roll = random(seed);
            if !self.open_ground(x, z, 0.5) || roll > 0.15 + bias * 0.5 {
                continue;
            }
            let scale = 0.4 + random(seed) * random(seed) * 2.6;
            self.prop(PropKind::Rock, x, z, random(seed) * std::f32::consts::TAU, scale);
            placed += 1;
        }
    }

    /// Mountains and the far keep: big cards at the edge of the fog, outside the map.
    fn backdrop(&mut self) {
        let mountains = [
            (-140.0, -190.0, 150.0, 120.0, 90.0),
            (0.0, -245.0, 210.0, 160.0, 100.0),
            (150.0, -185.0, 140.0, 110.0, 80.0),
            (-235.0, -60.0, 120.0, 110.0, 90.0),
            (240.0, -40.0, 130.0, 95.0, 80.0),
            (-215.0, 110.0, 130.0, 80.0, 90.0),
            (225.0, 120.0, 140.0, 85.0, 90.0),
            (-70.0, 235.0, 150.0, 70.0, 80.0),
            (110.0, 230.0, 160.0, 75.0, 80.0),
        ];
        // Each mountain is a main peak with two smaller ones beside it, turned a little, so the
        // skyline is a ridge and not a row of pyramids.
        for (index, (x, z, w, h, d)) in mountains.into_iter().enumerate() {
            let tilt = 0.2 + 0.15 * (index % 4) as f32;
            for (dx, scale, turn) in [(0.0, 1.0, tilt), (-0.34, 0.62, -tilt * 1.7), (0.3, 0.5, tilt * 2.3)] {
                let mut mountain =
                    object(Vec3::new(x + w * dx, h * scale * 0.2, z + d * dx * 0.3), Vec3::new(w * scale * 0.9, h * scale, d * scale), 0, 0.0);
                mountain.primitive = Primitive::Octahedron;
                mountain.rotation = Vec3::new(0.0, turn, 0.0);
                self.out.backdrop.push(mountain);
            }
        }
        self.far_keep(-30.0, -205.0);
    }

    fn far_keep(&mut self, x: f32, z: f32) {
        let mut push = |center: [f32; 3], size: [f32; 3], tone: u32, emissive: f32| {
            self.out.backdrop.push(object(Vec3::from_array(center), Vec3::from_array(size), tone, emissive));
        };
        push([x, 5.0, z], [74.0, 40.0, 34.0], 0, 0.0);
        push([x, 31.0, z + 10.0], [62.0, 12.0, 5.0], 1, 0.0);
        for dx in [-27.0, 27.0] {
            push([x + dx, 38.0, z + 8.0], [9.0, 30.0, 9.0], 1, 0.0);
        }
        push([x, 46.0, z - 4.0], [16.0, 34.0, 16.0], 1, 0.0);
        push([x - 10.0, 58.0, z - 4.0], [5.0, 14.0, 5.0], 1, 0.0);
        push([x + 10.0, 58.0, z - 4.0], [5.0, 14.0, 5.0], 1, 0.0);
        for (dx, dy) in [(-4.0, 52.0), (2.0, 52.0), (-1.0, 40.0), (-27.0, 46.0), (27.0, 46.0)] {
            push([x + dx, dy, z + 12.6], [1.4, 3.4, 0.2], 0, 1.8);
        }
    }
}

/// Place everything on `terrain`.
pub(super) fn build(terrain: &Terrain) -> Layout {
    let mut builder = Builder::new(terrain);
    builder.watchtower();
    builder.buttress();
    builder.courtyard();
    builder.plateau_ruin();
    builder.terrace();
    // Crates by the start, to try the first climb on.
    builder.prop(PropKind::Crate, 6.0, 42.0, 0.0, 1.0);
    builder.prop(PropKind::Block, 8.0, 41.0, 0.0, 1.0);
    let mut seed = 20_261_111_u32 % 233_280;
    builder.trees(&mut seed);
    builder.rocks(&mut seed);
    builder.backdrop();
    builder.out
}
