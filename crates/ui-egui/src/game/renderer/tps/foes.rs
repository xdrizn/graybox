//! Foes and arrows (G11.5), drawn each frame into the dynamic vertex buffer with the hero.
//!
//! A husk is an original hunched, faceted figure about 2 m tall with two orange slit eyes (the
//! only emissive on it); its arms rise through the wind-up and fall in the swing. A dummy is a
//! straw sack on a post with a crossbar and a dark target patch. Hit, both flash white and lean
//! away; broken, their parts fly apart, fall and vanish.
use super::super::mesh::{self, Vertex};
use figcraft_game::tps::{ArrowPose, FoeKind, FoePose, PIECES_TIME};
use glam::{Mat4, Vec3};

const DARK: u32 = 0;
const HUSK_SKIN: u32 = 1;
const HUSK_PLATE: u32 = 2;
const STRAW: u32 = 3;
const WOOD: u32 = 2;
const SHAFT: u32 = 4;

/// One box of a foe, in the foe's own space (feet at the origin, looking along -Z).
struct Part {
    /// Where the box hangs from (an arm's shoulder), and the box's middle from there.
    pivot: Vec3,
    offset: Vec3,
    size: Vec3,
    tone: u32,
    emissive: f32,
    /// Turned about X at the pivot by the arms' raise (radians at full raise).
    swing: f32,
}

const fn part(at: [f32; 3], size: [f32; 3], tone: u32) -> Part {
    Part { pivot: Vec3::new(at[0], at[1], at[2]), offset: Vec3::ZERO, size: Vec3::new(size[0], size[1], size[2]), tone, emissive: 0.0, swing: 0.0 }
}

const fn arm(shoulder: [f32; 3], size: [f32; 3], tone: u32) -> Part {
    Part {
        pivot: Vec3::new(shoulder[0], shoulder[1], shoulder[2]),
        offset: Vec3::new(0.0, -size[1] * 0.5, 0.0),
        size: Vec3::new(size[0], size[1], size[2]),
        tone,
        emissive: 0.0,
        swing: 2.6,
    }
}

const fn eye(x: f32) -> Part {
    Part { pivot: Vec3::new(x, 1.9, -0.3), offset: Vec3::ZERO, size: Vec3::new(0.12, 0.035, 0.03), tone: 5, emissive: 1.4, swing: 0.0 }
}

const HUSK: [Part; 12] = [
    part([-0.17, 0.45, 0.0], [0.22, 0.9, 0.24], HUSK_SKIN),
    part([0.17, 0.45, 0.0], [0.22, 0.9, 0.24], HUSK_SKIN),
    part([0.0, 0.98, 0.02], [0.52, 0.26, 0.32], HUSK_PLATE),
    part([0.0, 1.38, -0.05], [0.72, 0.6, 0.46], HUSK_PLATE),
    part([0.0, 1.66, -0.12], [0.84, 0.18, 0.4], HUSK_SKIN),
    part([0.0, 1.88, -0.14], [0.38, 0.34, 0.34], HUSK_SKIN),
    part([0.0, 2.03, -0.1], [0.3, 0.06, 0.3], HUSK_PLATE),
    eye(-0.08),
    eye(0.08),
    arm([-0.46, 1.68, -0.1], [0.17, 0.9, 0.17], HUSK_SKIN),
    arm([0.46, 1.68, -0.1], [0.17, 0.9, 0.17], HUSK_SKIN),
    part([0.0, 1.32, 0.2], [0.5, 0.36, 0.08], DARK),
];

const DUMMY: [Part; 6] = [
    part([0.0, 0.5, 0.0], [0.12, 1.0, 0.12], WOOD),
    part([0.0, 1.22, 0.0], [0.52, 0.72, 0.36], STRAW),
    part([0.0, 1.45, 0.0], [1.15, 0.1, 0.1], WOOD),
    part([0.0, 1.72, 0.0], [0.32, 0.3, 0.3], STRAW),
    part([0.0, 1.25, -0.19], [0.2, 0.2, 0.02], DARK),
    part([0.0, 0.03, 0.0], [0.5, 0.06, 0.5], WOOD),
];

/// A deterministic direction for the `index`th piece of a broken foe (outward and a little up).
fn scatter(index: usize) -> Vec3 {
    let angle = index as f32 * 2.399_963;
    Vec3::new(angle.cos(), 0.6 + (index % 3) as f32 * 0.25, angle.sin())
}

fn box_at(out: &mut Vec<Vertex>, matrix: Mat4, size: Vec3, tone: u32, emissive: f32) {
    let mut part = mesh::part([0.0; 3], size.to_array(), tone, [0.0; 3]);
    part.emissive = emissive;
    part.roughness = 0.85;
    mesh::object(out, &part, matrix, false);
}

pub(super) fn foe(out: &mut Vec<Vertex>, pose: &FoePose) {
    if !pose.position.is_finite() || !pose.facing.is_finite() {
        return;
    }
    let parts: &[Part] = match pose.kind {
        FoeKind::Husk => &HUSK,
        FoeKind::Dummy => &DUMMY,
    };
    let broken = pose.broken.filter(|time| time.is_finite());
    if broken.is_some_and(|time| time >= PIECES_TIME) {
        return;
    }
    let mut base = Mat4::from_translation(pose.position) * Mat4::from_rotation_y(-pose.facing);
    // Leaning away from the last hit, about the feet.
    let toward = Vec3::new(pose.lean_toward.x, 0.0, pose.lean_toward.z);
    if pose.lean.abs() > 1e-3 && toward.length_squared() > 1e-4 {
        let axis = Vec3::Y.cross(toward.normalize());
        let tilt = pose.lean * if pose.kind == FoeKind::Dummy { 0.35 } else { 0.25 };
        base = Mat4::from_translation(pose.position) * Mat4::from_axis_angle(axis, tilt) * Mat4::from_rotation_y(-pose.facing);
    }
    let flash = pose.flash > 0.4;
    for (index, part) in parts.iter().enumerate() {
        let raise = if part.swing != 0.0 { pose.raise.clamp(0.0, 1.0) * part.swing } else { 0.0 };
        let mut local = Mat4::from_translation(part.pivot) * Mat4::from_rotation_x(raise) * Mat4::from_translation(part.offset);
        let mut size = part.size;
        if let Some(time) = broken {
            // Pieces fly out, fall to the ground, and shrink away at the end.
            let out_and_up = scatter(index);
            let fall = (out_and_up.y * 3.0 * time - 4.9 * time * time).max(-(part.pivot.y + part.offset.y) + size.y * 0.5);
            let shift = Vec3::new(out_and_up.x * 1.6 * time, fall, out_and_up.z * 1.6 * time);
            let spin = Mat4::from_rotation_x(time * (2.0 + index as f32)) * Mat4::from_rotation_z(time * 1.5);
            local = Mat4::from_translation(shift) * local * spin;
            size *= (1.0 - ((time - PIECES_TIME * 0.7) / (PIECES_TIME * 0.3)).clamp(0.0, 1.0)).max(0.05);
        }
        let tone = if flash && part.emissive == 0.0 { 5 } else { part.tone };
        box_at(out, base * local, size, tone, part.emissive);
    }
}

/// An arrow: a thin shaft behind its tip, and a small dark fletch.
pub(super) fn arrow(out: &mut Vec<Vertex>, pose: &ArrowPose) {
    let direction = pose.direction.normalize_or_zero();
    if !pose.position.is_finite() || direction == Vec3::ZERO {
        return;
    }
    let rotation = glam::Quat::from_rotation_arc(Vec3::Y, direction);
    let middle = pose.position - direction * 0.38;
    box_at(out, Mat4::from_rotation_translation(rotation, middle), Vec3::new(0.025, 0.76, 0.025), SHAFT, 0.0);
    let tail = pose.position - direction * 0.7;
    box_at(out, Mat4::from_rotation_translation(rotation, tail), Vec3::new(0.08, 0.1, 0.012), DARK, 0.0);
}
