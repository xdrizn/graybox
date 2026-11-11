//! The eclipse: a thin glowing ring high over the far keep, drawn around the camera so it never
//! comes closer however far the hero walks.
use super::super::mesh::Vertex;
use super::shapes::tri;
use glam::Vec3;
use std::f32::consts::TAU;

/// Where the ring hangs, as a direction from the camera: north, a little west, about 16° up.
const DIRECTION: Vec3 = Vec3::new(-0.15, 0.3, -1.0);
/// How far away it is, and its radii (metres at that distance: about 9° across).
const DISTANCE: f32 = 300.0;
const OUTER: f32 = 24.0;
const THIN: f32 = 1.6;
const SIDES: u32 = 96;

/// The ring and a fainter halo around it, as triangles seen from the camera.
pub(super) fn vertices(out: &mut Vec<Vertex>, eye: Vec3) {
    if !eye.is_finite() {
        return;
    }
    let toward = DIRECTION.normalize_or_zero();
    let across = toward.cross(Vec3::Y).normalize_or_zero();
    let up = across.cross(toward).normalize_or_zero();
    let centre = eye + toward * DISTANCE;
    // Flagged like the gun (no fog, no scene lights): the glow comes from the emission alone.
    for (inner, outer, glow) in [(OUTER - THIN, OUTER, 2.4), (OUTER, OUTER + 5.0, 0.35)] {
        for index in 0..SIDES {
            let (a, b) = (index as f32 * TAU / SIDES as f32, (index + 1) as f32 * TAU / SIDES as f32);
            let at = |angle: f32, radius: f32| centre + (across * angle.cos() + up * angle.sin()) * radius;
            let material = [0.0, glow, 0.9, 1.0];
            // Seen from the camera, `across` runs right and `up` runs up, so a to b turns counter-clockwise only with the winding below.
            tri(out, [at(a, inner), at(b, outer), at(b, inner)], material);
            tri(out, [at(a, inner), at(a, outer), at(b, outer)], material);
        }
    }
}
