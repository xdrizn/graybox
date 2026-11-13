//! One mesh for each kind of prop (trees, rocks, ruins, crates), standing on the origin and facing
//! +Z with scale 1. The renderer draws each one instanced, once per [`figcraft_game::tps::Prop`].
use super::super::mesh::{self, Vertex};
use super::shapes::{blob, crown, frustum, material};
use figcraft_game::tps::{ARCH_LEG_HEIGHT, ARCH_LEG_SIZE, ARCH_LEG_X, BLOCK_SIZE, BROKEN_PILLAR_HEIGHT, CRATE_SIZE, PILLAR_HEIGHT, PropKind};
use glam::{Mat4, Vec3};

fn boxed(out: &mut Vec<Vertex>, position: [f32; 3], size: [f32; 3], tone: u32, rotation: [f32; 3]) {
    mesh::object(out, &mesh::part(position, size, tone, rotation), Mat4::IDENTITY, false);
}

fn pine(out: &mut Vec<Vertex>) {
    frustum(out, Vec3::ZERO, 6, [0.2, 0.12, 1.5], material(0, 0.0, 0.9), None);
    for (y, radius, height, tone) in [(0.9, 1.25, 1.7, 1), (1.9, 0.95, 1.5, 2), (2.8, 0.65, 1.4, 3)] {
        frustum(out, Vec3::Y * y, 6, [radius, 0.0, height], crown(tone), None);
    }
}

fn oak(out: &mut Vec<Vertex>) {
    frustum(out, Vec3::ZERO, 6, [0.24, 0.17, 2.3], material(0, 0.0, 0.9), None);
    blob(out, Vec3::new(0.0, 3.2, 0.0), Vec3::new(1.5, 1.2, 1.5), 3, crown(2));
    blob(out, Vec3::new(0.8, 2.6, 0.4), Vec3::new(0.9, 0.8, 0.9), 4, crown(1));
}

fn rock(out: &mut Vec<Vertex>) {
    blob(out, Vec3::new(0.0, 0.45, 0.0), Vec3::new(0.85, 0.7, 0.8), 7, material(2, 0.0, 0.95));
}

fn pillar(out: &mut Vec<Vertex>, broken: bool) {
    boxed(out, [0.0, 0.2, 0.0], [1.35, 0.4, 1.35], 2, [0.0; 3]);
    if broken {
        let height = BROKEN_PILLAR_HEIGHT - 0.4;
        frustum(out, Vec3::Y * 0.4, 8, [0.52, 0.47, height], material(3, 0.0, 0.9), Some(Vec3::new(0.15, 0.35, 0.1)));
        // A drum of the column lies beside it.
        boxed(out, [1.3, 0.22, 0.5], [0.9, 0.45, 0.9], 2, [0.0, 0.5, 0.2]);
    } else {
        let shaft = PILLAR_HEIGHT - 0.8;
        frustum(out, Vec3::Y * 0.4, 8, [0.52, 0.46, shaft], material(3, 0.0, 0.9), None);
        boxed(out, [0.0, PILLAR_HEIGHT - 0.2, 0.0], [1.4, 0.4, 1.4], 2, [0.0; 3]);
    }
}

/// A pointed arch spanning along X: two legs and two leaning slabs meeting at the top.
fn arch(out: &mut Vec<Vertex>) {
    for side in [-1.0, 1.0] {
        let x = ARCH_LEG_X * side;
        boxed(out, [x, ARCH_LEG_HEIGHT * 0.5, 0.0], [ARCH_LEG_SIZE, ARCH_LEG_HEIGHT, ARCH_LEG_SIZE], 3, [0.0; 3]);
        boxed(out, [x, 0.2, 0.0], [ARCH_LEG_SIZE + 0.3, 0.4, ARCH_LEG_SIZE + 0.3], 2, [0.0; 3]);
        boxed(out, [x * 0.5, ARCH_LEG_HEIGHT + 1.0, 0.0], [2.9, 0.7, ARCH_LEG_SIZE], 3, [0.0, 0.0, -0.785 * side]);
    }
    boxed(out, [0.0, ARCH_LEG_HEIGHT + 1.95, 0.0], [0.8, 0.7, ARCH_LEG_SIZE], 2, [0.0; 3]);
}

fn crate_mesh(out: &mut Vec<Vertex>) {
    let half = CRATE_SIZE * 0.5;
    boxed(out, [0.0, half, 0.0], [CRATE_SIZE; 3], 3, [0.0; 3]);
    boxed(out, [0.0, half, 0.0], [CRATE_SIZE + 0.04, 0.12, CRATE_SIZE + 0.04], 1, [0.0; 3]);
    for (x, z) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        boxed(out, [x * half, half, z * half], [0.1, CRATE_SIZE + 0.04, 0.1], 1, [0.0; 3]);
    }
}

fn block(out: &mut Vec<Vertex>) {
    boxed(out, [0.0, BLOCK_SIZE * 0.5, 0.0], [BLOCK_SIZE; 3], 2, [0.0; 3]);
    for y in [0.35, BLOCK_SIZE - 0.35] {
        boxed(out, [0.0, y, 0.0], [BLOCK_SIZE + 0.05, 0.12, BLOCK_SIZE + 0.05], 3, [0.0; 3]);
    }
}

/// The mesh of one kind of prop.
pub(super) fn mesh(kind: PropKind) -> Vec<Vertex> {
    let mut out = Vec::with_capacity(256);
    match kind {
        PropKind::Pine => pine(&mut out),
        PropKind::Oak => oak(&mut out),
        PropKind::Rock => rock(&mut out),
        PropKind::Pillar => pillar(&mut out, false),
        PropKind::BrokenPillar => pillar(&mut out, true),
        PropKind::Arch => arch(&mut out),
        PropKind::Crate => crate_mesh(&mut out),
        PropKind::Block => block(&mut out),
    }
    out
}
