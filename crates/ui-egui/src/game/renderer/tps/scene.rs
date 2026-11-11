//! Everything the third-person world needs on the GPU, as plain data made on the CPU: so the budgets
//! (triangles, draw calls, memory) can be counted by tests without a graphics device.
use super::super::mesh::Vertex;
use super::grass::{self, Grass};
use super::props;
use super::world::{self, Statics};
use figcraft_game::tps::{PropKind, World};

/// The shadow map's side: 2048 on the desktop, 1024 in the browser.
pub(in crate::game::renderer) const SHADOW_SIZE: u32 = if cfg!(target_arch = "wasm32") { 1024 } else { 2048 };

/// One prop kind: its mesh and where each one stands. An instance is 8 floats:
/// `x, y, z, yaw, scale, 0, 0, 0`.
pub(super) struct Batch {
    #[cfg_attr(not(test), allow(dead_code))]
    pub kind: PropKind,
    pub vertices: Vec<Vertex>,
    pub instances: Vec<[f32; 8]>,
}

pub(in crate::game::renderer) struct SceneData {
    pub(super) statics: Statics,
    pub(super) batches: Vec<Batch>,
    pub(super) tuft: Vec<Vertex>,
    pub(super) grass: Grass,
}

/// The most triangles the hero and the sky may add (the hero is G11.3's; this is its allowance).
#[cfg(test)]
pub(super) const DYNAMIC_TRIANGLES: u32 = 6_000;

impl SceneData {
    pub fn build(world: &World) -> Self {
        let batches = PropKind::ALL
            .into_iter()
            .map(|kind| {
                let instances = world
                    .props()
                    .iter()
                    .filter(|prop| prop.kind == kind)
                    .map(|prop| [prop.position.x, prop.position.y, prop.position.z, prop.yaw, prop.scale, 0.0, 0.0, 0.0])
                    .collect();
                Batch { kind, vertices: props::mesh(kind), instances }
            })
            .collect();
        Self { statics: world::build(world), batches, tuft: grass::tuft_mesh(), grass: grass::build(world) }
    }
}

#[cfg(test)]
impl SceneData {
    /// Triangles drawn into the picture in the worst frame: all the static geometry, every prop, the
    /// most grass any camera position sees, and the hero's and sky's allowance.
    pub fn most_triangles_drawn(&self) -> u32 {
        let statics = self.statics.vertices.len() as u32 / 3;
        let props: u32 = self.batches.iter().map(|batch| batch.vertices.len() as u32 / 3 * batch.instances.len() as u32).sum();
        statics + props + self.grass.most_visible() * grass::TUFT_VERTICES / 3 + DYNAMIC_TRIANGLES
    }

    /// Triangles drawn into the shadow map (no grass, no water, no backdrop).
    pub fn shadow_triangles(&self) -> u32 {
        let statics = self.statics.solid.len() as u32 / 3;
        let props: u32 = self.batches.iter().map(|batch| batch.vertices.len() as u32 / 3 * batch.instances.len() as u32).sum();
        statics + props + DYNAMIC_TRIANGLES
    }

    /// Draw calls in the worst frame: the shadow pass plus the picture.
    pub fn most_draw_calls(&self) -> u32 {
        let used = self.batches.iter().filter(|batch| !batch.instances.is_empty()).count() as u32;
        // Shadow: ground and pieces, props, hero. Picture: ground, water, props, grass rows, hero and sky.
        let grass_rows = (grass::FADE_END * 2.0 / 8.0).ceil() as u32 + 2;
        (1 + used + 1) + (1 + 1 + used + grass_rows + 1)
    }

    /// Bytes of GPU memory the scene holds with a shadow map of `shadow_size` squared (depth, 4 bytes
    /// a texel), without the screen-sized targets.
    pub fn gpu_bytes(&self, shadow_size: u32) -> u64 {
        let vertex = std::mem::size_of::<Vertex>() as u64;
        let mut bytes = u64::from(shadow_size) * u64::from(shadow_size) * 4;
        bytes += self.statics.vertices.len() as u64 * vertex;
        bytes += self.batches.iter().map(|batch| batch.vertices.len() as u64 * vertex + batch.instances.len() as u64 * 32).sum::<u64>();
        bytes += self.tuft.len() as u64 * vertex;
        bytes += self.grass.tufts.len() as u64 * std::mem::size_of::<[f32; 4]>() as u64;
        // The dynamic buffer (the hero, the sky) and the noise texture.
        bytes + u64::from(DYNAMIC_TRIANGLES) * 3 * vertex + 96 * 16 * 4
    }
}
