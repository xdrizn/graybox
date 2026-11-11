//! Small flat-shaded shapes the world's props are made of (frustums, faceted blobs).
//! Every triangle is wound counter-clockwise seen from outside, like `mesh::object`'s.
use super::super::mesh::Vertex;
use glam::Vec3;
use std::f32::consts::TAU;

/// `[tone, emissive, roughness, flag]` of a vertex.
pub(super) type Material = [f32; 4];

pub(super) fn material(tone: u32, emissive: f32, roughness: f32) -> Material {
    [tone.min(5) as f32, emissive, roughness, 0.0]
}

/// One flat triangle; the normal comes from the winding.
pub(super) fn tri(out: &mut Vec<Vertex>, points: [Vec3; 3], material: Material) {
    let normal = (points[1] - points[0]).cross(points[2] - points[0]).normalize_or_zero();
    out.extend(points.map(|point| Vertex { position: point.to_array(), normal: normal.to_array(), material }));
}

/// A deterministic number in `[0, 1)` for a pair of small integers.
pub(super) fn hash(a: u32, b: u32) -> f32 {
    let mut h = a.wrapping_mul(0x9E37_79B1) ^ b.wrapping_mul(0x85EB_CA6B);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    (h >> 8) as f32 / 16_777_216.0
}

/// An `sides`-sided frustum standing on `base` (its middle of the bottom): radius `bottom` at the
/// foot, `top` at `height` (0 for a cone). `cap` closes the top; the underside is never drawn.
/// `peak` raises the middle of the cap and moves it sideways (a broken top).
pub(super) fn frustum(out: &mut Vec<Vertex>, base: Vec3, sides: u32, [bottom, top, height]: [f32; 3], material: Material, peak: Option<Vec3>) {
    let sides = sides.clamp(3, 16);
    let ring = |index: u32, radius: f32, y: f32| {
        let angle = index as f32 * TAU / sides as f32;
        base + Vec3::new(angle.cos() * radius, y, angle.sin() * radius)
    };
    let middle = base + Vec3::Y * height + peak.unwrap_or(Vec3::ZERO);
    for index in 0..sides {
        let (foot, head) = (ring(index, bottom, 0.0), ring(index, top, height));
        let (next_foot, next_head) = (ring(index + 1, bottom, 0.0), ring(index + 1, top, height));
        if top > 0.0 {
            tri(out, [foot, head, next_head], material);
            tri(out, [foot, next_head, next_foot], material);
            tri(out, [middle, next_head, head], material);
        } else {
            tri(out, [foot, middle, next_foot], material);
        }
    }
}

/// A faceted lump (a boulder, a tree's crown): twenty triangles around `center`, squashed to the
/// `radii`, with every vertex nudged a little by `seed`.
pub(super) fn blob(out: &mut Vec<Vertex>, center: Vec3, radii: Vec3, seed: u32, material: Material) {
    let nudge = |index: u32, amount: f32| 1.0 + (hash(seed, index) - 0.5) * amount;
    let upper = |k: u32| {
        let angle = k as f32 * TAU / 5.0 + (hash(seed, 40 + k) - 0.5) * 0.4;
        center + Vec3::new(angle.cos() * 0.85, 0.45, angle.sin() * 0.85) * radii * nudge(k, 0.3)
    };
    let lower = |k: u32| {
        let angle = (k as f32 + 0.5) * TAU / 5.0 + (hash(seed, 60 + k) - 0.5) * 0.4;
        center + Vec3::new(angle.cos(), -0.3, angle.sin()) * radii * nudge(10 + k, 0.3)
    };
    let top = center + Vec3::new((hash(seed, 80) - 0.5) * 0.2, 1.0, (hash(seed, 81) - 0.5) * 0.2) * radii;
    let bottom = center + Vec3::new(0.0, -0.75, 0.0) * radii;
    for k in 0..5 {
        let next = (k + 1) % 5;
        tri(out, [top, upper(next), upper(k)], material);
        tri(out, [lower(k), upper(k), upper(next)], material);
        tri(out, [lower(k), upper(next), lower(next)], material);
        tri(out, [bottom, lower(k), lower(next)], material);
    }
}
