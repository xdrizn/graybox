//! The world's one-off geometry for the renderer, built once: the ground, the ruins and the tower,
//! the mountains and keep in the fog, and the river with its waterfall.
use super::super::mesh::{self, Vertex};
use super::shapes::{hash, material, tri};
use figcraft_game::tps::{CELLS, RIVER_HALF_WIDTH, World};
use glam::{Mat4, Vec3};
use std::ops::Range;

/// A river segment that drops more than this in one metre is the waterfall.
const FALL_DROP: f32 = 0.6;

/// What `build` makes: one vertex list in three runs.
pub(super) struct Statics {
    pub vertices: Vec<Vertex>,
    /// The ground and the one-off pieces: drawn lit, and into the shadow map.
    pub solid: Range<u32>,
    /// Mountains and the keep (no fog, no shadow), drawn after the solid run with the same shader.
    pub backdrop: Range<u32>,
    /// The river and the waterfall: their own shader, no shadow.
    pub water: Range<u32>,
}

/// The tone a ground triangle gets: dark where flat, lighter and rockier where steep.
fn ground_tone(normal_y: f32, cell: u32, facet: u32) -> u32 {
    // Flat ground is the darkest grey; it gets lighter as it gets steeper, and a few facets of the
    // slopes are a shade lighter so a hillside is not one flat colour.
    let speckle = hash(cell, facet) > 0.75;
    let tone = if normal_y > 0.93 {
        0
    } else if normal_y > 0.8 {
        1
    } else if normal_y > 0.55 {
        2
    } else {
        3
    };
    if speckle && normal_y <= 0.93 { tone + 1 } else { tone }
}

fn terrain(out: &mut Vec<Vertex>, world: &World) {
    let ground = world.terrain();
    for j in 0..CELLS {
        for i in 0..CELLS {
            for (facet, points) in ground.cell_triangles(i, j).into_iter().enumerate() {
                let normal = (points[1] - points[0]).cross(points[2] - points[0]).normalize_or_zero();
                let tone = ground_tone(normal.y, (j * CELLS + i) as u32, facet as u32);
                // Low, wet ground by the river shines a little in the beacon's light.
                let middle = (points[0].y + points[1].y + points[2].y) / 3.0;
                let roughness = if middle < 1.0 { 0.45 } else { 0.9 };
                tri(out, points, material(tone, 0.0, roughness));
            }
        }
    }
}

fn pieces(out: &mut Vec<Vertex>, world: &World) {
    for piece in world.structures() {
        mesh::object(out, piece, Mat4::IDENTITY, false);
    }
}

fn backdrop(out: &mut Vec<Vertex>, world: &World) {
    for piece in world.backdrop() {
        // Flagged like the first-person gun: no fog and no scene lights, so the far shapes stay
        // dark silhouettes however far away they are.
        mesh::object(out, piece, Mat4::IDENTITY, true);
    }
}

/// One flat strip of water from the middle line out to a bank: `edge` runs 0 to 1 across it.
fn strip(out: &mut Vec<Vertex>, [near_a, near_b]: [Vec3; 2], [far_a, far_b]: [Vec3; 2], base: [f32; 3]) {
    let at = |edge: f32| [base[0], base[1], base[2], edge];
    let mut push = |points: [Vec3; 3], edges: [f32; 3]| {
        let normal = (points[1] - points[0]).cross(points[2] - points[0]).normalize_or_zero();
        // Water faces up whatever way the strip is wound.
        let (points, edges, normal) =
            if normal.y < 0.0 { ([points[0], points[2], points[1]], [edges[0], edges[2], edges[1]], -normal) } else { (points, edges, normal) };
        for (point, edge) in points.into_iter().zip(edges) {
            out.push(Vertex { position: point.to_array(), normal: normal.to_array(), material: at(edge) });
        }
    };
    push([near_a, far_a, near_b], [0.0, 1.0, 0.0]);
    push([near_b, far_a, far_b], [0.0, 1.0, 1.0]);
}

/// The river as a ribbon along its middle line, with the waterfall where it drops.
fn water(out: &mut Vec<Vertex>, world: &World) {
    let points = world.river();
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let flow = Vec3::new(b.x - a.x, 0.0, b.z - a.z).normalize_or_zero();
        let side = Vec3::new(flow.z, 0.0, -flow.x);
        let falls = (a.y - b.y).abs() > FALL_DROP;
        let (half, base) = if falls { (RIVER_HALF_WIDTH * 0.85, [2.0, 2.0, 0.5]) } else { (RIVER_HALF_WIDTH, [0.0, 0.9, 0.12]) };
        for sign in [-1.0, 1.0] {
            strip(out, [a, b], [a + side * half * sign, b + side * half * sign], base);
        }
    }
}

/// Build the world's one-off geometry.
pub(super) fn build(world: &World) -> Statics {
    let mut vertices = Vec::with_capacity(60_000);
    terrain(&mut vertices, world);
    pieces(&mut vertices, world);
    let solid_end = vertices.len() as u32;
    backdrop(&mut vertices, world);
    let backdrop_end = vertices.len() as u32;
    water(&mut vertices, world);
    let water_end = vertices.len() as u32;
    Statics { vertices, solid: 0..solid_end, backdrop: solid_end..backdrop_end, water: backdrop_end..water_end }
}
