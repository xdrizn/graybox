//! The third-person world: what the hero stands on and what the renderer draws around it.
//!
//! PLACEHOLDER (G11.1): flat ground with a few blocks and pillars so movement can be seen. G11.2
//! owns this file and `renderer/tps/world.rs`: it replaces both with the real valley (a
//! heightfield, grass, props) behind the same two questions the rest of the game asks, the height
//! of the ground under a point ([`World::ground_height`]) and the pieces to draw ([`World::objects`]).
use crate::arena::{Primitive, SceneObject, random};
use glam::Vec3;

/// The ground reaches this far from the middle in every direction (metres).
pub const HALF_EXTENT: f32 = 180.0;

/// Ground and scenery of the third-person game.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct World;

fn piece(position: Vec3, size: Vec3, tone: u32, emissive: f32) -> SceneObject {
    SceneObject { position, size, rotation: Vec3::ZERO, tone, emissive, roughness: 0.9, collision: false, primitive: Primitive::Box }
}

impl World {
    pub fn new() -> Self {
        Self
    }

    /// Where the hero starts (feet).
    pub fn spawn(&self) -> Vec3 {
        Vec3::new(0.0, self.ground_height(0.0, 0.0), 0.0)
    }

    /// The height of the ground at a point of the map (flat for now).
    pub fn ground_height(&self, _x: f32, _z: f32) -> f32 {
        0.0
    }

    /// `position` kept on the map (and finite): the hero cannot walk off the edge.
    pub fn keep_on_map(&self, position: Vec3) -> Vec3 {
        if !position.is_finite() {
            return self.spawn();
        }
        Vec3::new(position.x.clamp(-HALF_EXTENT, HALF_EXTENT), position.y, position.z.clamp(-HALF_EXTENT, HALF_EXTENT))
    }

    /// Everything the renderer draws as part of the world, once (full extents, Y up).
    pub fn objects(&self) -> Vec<SceneObject> {
        let mut out = Vec::with_capacity(64);
        let mut ground = piece(Vec3::new(0.0, -0.5, 0.0), Vec3::new(HALF_EXTENT * 2.0, 1.0, HALF_EXTENT * 2.0), 1, 0.0);
        ground.roughness = 0.9;
        out.push(ground);
        // Scatter of blocks and pillars, always the same, none close to where the hero starts.
        let mut seed = 31_u32;
        for index in 0..48 {
            let angle = random(&mut seed) * std::f32::consts::TAU;
            let distance = 7.0 + random(&mut seed) * 80.0;
            let width = 0.8 + random(&mut seed) * 2.4;
            let height = 0.5 + random(&mut seed) * 4.5;
            let tone = 2 + (index % 2);
            let position = Vec3::new(angle.cos() * distance, height * 0.5, angle.sin() * distance);
            out.push(piece(position, Vec3::new(width, height, width), tone, 0.0));
            // A few pillars carry an orange light on top: the only accent in the scene.
            if index % 8 == 0 {
                out.push(piece(position + Vec3::Y * (height * 0.5 + 0.2), Vec3::splat(0.3), 0, 1.6));
            }
        }
        out
    }
}
