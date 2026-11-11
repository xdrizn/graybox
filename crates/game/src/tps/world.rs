//! The third-person world: one hand-built valley, about 160 × 160 m. What the hero stands on and
//! what the renderer draws around it, as plain data.
//!
//! - [`World::ground_height`] is the one lookup that answers "how high is the ground here?"; it reads the
//!   same heightfield the renderer draws ([`Terrain`]).
//! - [`World::props`] are the repeated things (trees, rocks, pillars, arches, crates), drawn instanced;
//!   [`World::structures`] are the one-off pieces (watchtower, ladders, climb wall, paving), drawn as boxes.
//! - [`World::solids`] are boxes the hero cannot walk through and [`World::climbables`] tag what can be
//!   climbed (G11.4 reads both).
//! - [`World::river`] is the water's middle line; the waterfall is where it drops.
//! - [`World::backdrop`] are the mountains and the far keep, outside the map, drawn in the fog.
use super::layout::{self, Layout, SPAWN_XZ};
use super::props::{Climbable, Prop, Solid};
use super::terrain::{self, Terrain};
use crate::arena::SceneObject;
use glam::Vec3;

pub use super::terrain::HALF_EXTENT;

/// The hero walks this close to the edge of the map and no closer (metres from the edge).
const EDGE_MARGIN: f32 = 3.0;

/// Ground and scenery of the third-person game.
#[derive(Clone, Debug, PartialEq)]
pub struct World {
    terrain: Terrain,
    layout: Layout,
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

impl World {
    pub fn new() -> Self {
        let terrain = Terrain::new();
        let layout = layout::build(&terrain);
        Self { terrain, layout }
    }

    /// Where the hero starts (feet), on a small flat pad of the hill, looking north over the valley.
    pub fn spawn(&self) -> Vec3 {
        Vec3::new(SPAWN_XZ[0], self.ground_height(SPAWN_XZ[0], SPAWN_XZ[1]), SPAWN_XZ[1])
    }

    /// The height of the ground at a point of the map (the nearest edge's height beyond the map).
    pub fn ground_height(&self, x: f32, z: f32) -> f32 {
        self.terrain.height(x, z)
    }

    /// How steep the ground is at a point (rise over run).
    pub fn slope(&self, x: f32, z: f32) -> f32 {
        self.terrain.slope(x, z)
    }

    /// `position` kept on the map (and finite): the hero cannot walk off the edge.
    pub fn keep_on_map(&self, position: Vec3) -> Vec3 {
        if !position.is_finite() {
            return self.spawn();
        }
        let limit = HALF_EXTENT - EDGE_MARGIN;
        Vec3::new(position.x.clamp(-limit, limit), position.y, position.z.clamp(-limit, limit))
    }

    /// The most the hero walks from the middle (metres).
    pub fn walk_limit(&self) -> f32 {
        HALF_EXTENT - EDGE_MARGIN
    }

    pub fn terrain(&self) -> &Terrain {
        &self.terrain
    }

    /// Trees, rocks, pillars, arches, crates and blocks.
    pub fn props(&self) -> &[Prop] {
        &self.layout.props
    }

    /// The one-off pieces: watchtower, ladders, climb wall, paving, glowing slits (full extents, Y up).
    pub fn structures(&self) -> &[SceneObject] {
        &self.layout.structures
    }

    /// Mountains and the far keep, outside the map.
    pub fn backdrop(&self) -> &[SceneObject] {
        &self.layout.backdrop
    }

    /// Boxes the hero cannot walk through (trunks, pillars, walls, crates, the tower…).
    pub fn solids(&self) -> &[Solid] {
        &self.layout.solids
    }

    /// Ladders, ledges to mantle onto and the climb wall.
    pub fn climbables(&self) -> &[Climbable] {
        &self.layout.climbables
    }

    /// The beacon on top of the watchtower (the world's brightest light).
    pub fn beacon(&self) -> Vec3 {
        self.layout.beacon
    }

    /// The river's middle line, a point every metre from the plateau's spring to the valley's end:
    /// `y` is the surface of the water. Where it drops fast, it is the waterfall.
    pub fn river(&self) -> Vec<Vec3> {
        let count = (2.0 * terrain::RIVER_END) as i32;
        (0..=count)
            .map(|step| {
                let x = -terrain::RIVER_END + step as f32;
                let z = terrain::river_z(x);
                // Flat on the plateau and in the valley; over the lip it follows the drawn ground.
                let y = if (x - terrain::FALL_X).abs() < 5.0 { self.ground_height(x, z) + terrain::WATER_DEPTH } else { terrain::water_level(x) };
                Vec3::new(x, y, z)
            })
            .collect()
    }

    /// Whether a point is in the river's water.
    pub fn in_water(&self, x: f32, z: f32) -> bool {
        layout::near_river(x, z, 0.0) && self.ground_height(x, z) < terrain::water_level(x)
    }

    /// How thickly grass grows at a point, 0 (bare) to 1: none on steep ground, in the water, on the
    /// ruins' floors or under the map's rim, thinner on patches.
    pub fn grass_cover(&self, x: f32, z: f32) -> f32 {
        let inside = HALF_EXTENT - 6.0;
        if !x.is_finite() || !z.is_finite() || x.abs() > inside || z.abs() > inside || layout::near_river(x, z, 0.4) {
            return 0.0;
        }
        let slope = self.terrain.slope(x, z);
        if slope > 0.62 {
            return 0.0;
        }
        let paved = terrain::ZONES.iter().take(4).any(|zone| (x - zone.x).hypot(z - zone.z) < zone.radius * 0.55);
        let bare = terrain::patch(x, z, 0.09, 21);
        if paved || bare < 0.2 {
            return 0.0;
        }
        let steep = 1.0 - terrain::smooth(0.3, 0.62, slope) * 0.75;
        (0.55 + bare * 0.6).min(1.0) * steep
    }

    /// Bytes the world's data holds (for the memory budget).
    pub fn bytes(&self) -> usize {
        self.terrain.bytes()
            + std::mem::size_of_val(self.layout.props.as_slice())
            + std::mem::size_of_val(self.layout.solids.as_slice())
            + std::mem::size_of_val(self.layout.climbables.as_slice())
            + std::mem::size_of_val(self.layout.structures.as_slice())
            + std::mem::size_of_val(self.layout.backdrop.as_slice())
    }

    /// A number that changes if anything in the world changes (the test of determinism uses it).
    pub fn checksum(&self) -> u64 {
        let mut sum = self.terrain.checksum();
        let mut mix = |value: f32| sum = (sum ^ u64::from(value.to_bits())).wrapping_mul(0x0100_0000_01b3);
        for prop in &self.layout.props {
            [prop.position.x, prop.position.y, prop.position.z, prop.yaw, prop.scale, prop.kind.index() as f32].into_iter().for_each(&mut mix);
        }
        for solid in &self.layout.solids {
            solid.min.to_array().into_iter().chain(solid.max.to_array()).for_each(&mut mix);
        }
        for object in self.layout.structures.iter().chain(&self.layout.backdrop) {
            object.position.to_array().into_iter().chain(object.size.to_array()).chain([object.emissive, object.tone as f32]).for_each(&mut mix);
        }
        sum
    }
}
