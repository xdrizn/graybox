//! Grass: tufts placed once over the whole valley (a jittered grid, thinner where the ground is
//! steep, wet, paved or bare), sorted in chunks so each frame draws only the ones near the camera.
//! The sway, the bend away from the hero and the fade with distance happen in the vertex shader.
use super::super::mesh::Vertex;
use super::shapes::{hash, material, tri};
use figcraft_game::tps::{HALF_EXTENT, World};
use glam::Vec3;
use std::f32::consts::FRAC_PI_3;
use std::ops::Range;

/// Metres between tufts on the grid (before the jitter).
pub(super) const SPACING: f32 = 0.65;
/// The grass starts to shrink here and is gone at `FADE_END` (metres from the camera).
pub(super) const FADE_START: f32 = 26.0;
pub(super) const FADE_END: f32 = 35.0;
/// Chunks are squares of this many metres; a frame draws one run of instances per row of chunks.
const CHUNK: f32 = 8.0;
const CHUNKS: usize = (HALF_EXTENT * 2.0 / CHUNK) as usize;
/// Vertices of one tuft: three blades of three triangles.
pub(super) const TUFT_VERTICES: u32 = 27;

/// One tuft: where it grows and a number in `[0, 1)` that picks its turn, height and shade.
pub(super) type Tuft = [f32; 4];

pub(super) struct Grass {
    pub tufts: Vec<Tuft>,
    /// Where each chunk's tufts start in `tufts` (chunks in row order, one extra at the end).
    starts: Vec<u32>,
}

fn chunk_of(coordinate: f32) -> usize {
    (((coordinate + HALF_EXTENT) / CHUNK).floor().max(0.0) as usize).min(CHUNKS - 1)
}

/// Place the grass over the whole valley.
pub(super) fn build(world: &World) -> Grass {
    let mut buckets: Vec<Vec<Tuft>> = vec![Vec::new(); CHUNKS * CHUNKS];
    let cells = (HALF_EXTENT * 2.0 / SPACING) as u32;
    for gz in 0..cells {
        for gx in 0..cells {
            let x = -HALF_EXTENT + (gx as f32 + hash(gx, gz.wrapping_add(7919))) * SPACING;
            let z = -HALF_EXTENT + (gz as f32 + hash(gx.wrapping_add(104_729), gz)) * SPACING;
            let cover = world.grass_cover(x, z);
            if cover <= 0.0 || hash(gx ^ 0x5bd1, gz ^ 0x1b87) > cover {
                continue;
            }
            let seed = hash(gx.wrapping_mul(31).wrapping_add(gz), 977);
            if let Some(bucket) = buckets.get_mut(chunk_of(z) * CHUNKS + chunk_of(x)) {
                bucket.push([x, world.ground_height(x, z) - 0.03, z, seed]);
            }
        }
    }
    let mut starts = Vec::with_capacity(CHUNKS * CHUNKS + 1);
    let mut tufts = Vec::new();
    for bucket in buckets {
        starts.push(tufts.len() as u32);
        tufts.extend(bucket);
    }
    starts.push(tufts.len() as u32);
    Grass { tufts, starts }
}

impl Grass {
    fn start(&self, index: usize) -> u32 {
        self.starts.get(index).copied().unwrap_or(0)
    }

    /// The runs of `tufts` worth drawing from a camera at `camera`: for each row of chunks, the
    /// chunks the circle of `range` metres reaches. (One draw call each; rows are contiguous.)
    pub(super) fn visible(&self, camera: Vec3, range: f32) -> Vec<Range<u32>> {
        let mut runs = Vec::new();
        if !camera.is_finite() {
            return runs;
        }
        for row in 0..CHUNKS {
            let (z0, z1) = (-HALF_EXTENT + row as f32 * CHUNK, -HALF_EXTENT + (row + 1) as f32 * CHUNK);
            let along = (z0 - camera.z).max(camera.z - z1).max(0.0);
            if along >= range {
                continue;
            }
            let reach = (range * range - along * along).sqrt();
            let (first, last) = (chunk_of(camera.x - reach), chunk_of(camera.x + reach));
            if camera.x + reach < -HALF_EXTENT || camera.x - reach > HALF_EXTENT {
                continue;
            }
            let run = self.start(row * CHUNKS + first)..self.start(row * CHUNKS + last + 1);
            if !run.is_empty() {
                runs.push(run);
            }
        }
        runs
    }

    #[cfg(test)]
    /// The most tufts a frame can draw, over a camera standing anywhere on the map.
    pub(super) fn most_visible(&self) -> u32 {
        let mut most = 0;
        for k in 0..=16 {
            for l in 0..=16 {
                let camera = Vec3::new(-HALF_EXTENT + k as f32 * HALF_EXTENT / 8.0, 0.0, -HALF_EXTENT + l as f32 * HALF_EXTENT / 8.0);
                most = most.max(self.visible(camera, FADE_END).iter().map(|run| run.end - run.start).sum());
            }
        }
        most
    }
}

/// One tuft's mesh, 1 m tall and about 0.6 m across: three blades turned 60° from each other, each
/// a base quad and a tip (three triangles), leaning a little. Greys get lighter towards the tip.
pub(super) fn tuft_mesh() -> Vec<Vertex> {
    let mut out = Vec::with_capacity(TUFT_VERTICES as usize);
    for blade in 0..3 {
        let angle = blade as f32 * FRAC_PI_3 + 0.3;
        let across = Vec3::new(angle.cos(), 0.0, angle.sin());
        let lean = Vec3::new(-across.z, 0.0, across.x);
        let centre = lean * (0.08 * (blade as f32 - 1.0));
        let point = |half_width: f32, side: f32, height: f32, lean_by: f32| centre + across * (half_width * side) + lean * lean_by + Vec3::Y * height;
        let (base_left, base_right) = (point(0.04, -1.0, 0.0, 0.0), point(0.04, 1.0, 0.0, 0.0));
        let (mid_left, mid_right) = (point(0.028, -1.0, 0.5, 0.08), point(0.028, 1.0, 0.5, 0.08));
        let tip = point(0.0, 0.0, 1.0, 0.24);
        // The blade's shading normal leans up and a little to the side it leans to.
        let up = (Vec3::Y + lean * 0.4).normalize_or_zero();
        let mut add = |points: [Vec3; 3], tone: u32| {
            tri(&mut out, points, material(tone, 0.0, 0.95));
            for vertex in out.iter_mut().rev().take(3) {
                vertex.normal = up.to_array();
            }
        };
        add([base_left, base_right, mid_left], 1);
        add([base_right, mid_right, mid_left], 2);
        add([mid_left, mid_right, tip], 3);
    }
    out
}
