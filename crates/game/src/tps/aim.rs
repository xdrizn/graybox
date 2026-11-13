//! Where the middle of the screen points (G11.6): a ray from the camera through its target, to the
//! first foe, solid or ground past the hero. Arrows fly there, so they hit what the crosshair is on.
use super::collide;
use super::foes::{self, Foe};
use super::world::World;
use glam::{Vec2, Vec3};

/// How far the crosshair reaches (metres past the hero).
pub(crate) const REACH: f32 = 80.0;
/// Ground checks along the ray, then halvings to pin the spot down.
const GROUND_SAMPLES: usize = 160;
const GROUND_REFINE: usize = 8;

/// The point the screen centre looks at, for a camera at `eye` looking at `target`. The ray starts
/// a little past `target` (the hero's middle or shoulder), so the hero never stops it.
pub(crate) fn point(world: &World, foes: &[Foe], eye: Vec3, target: Vec3) -> Vec3 {
    let direction = (target - eye).normalize_or_zero();
    if !eye.is_finite() || !target.is_finite() || direction == Vec3::ZERO {
        return if target.is_finite() { target } else { Vec3::ZERO };
    }
    let from = target + direction * 0.6;
    let to = from + direction * REACH;
    let mut nearest = collide::first_solid(world, from, to).unwrap_or(1.0);
    for foe in foes.iter().filter(|foe| foe.alive()) {
        if let Some(t) = through(foe, from, to) {
            nearest = nearest.min(t);
        }
    }
    if let Some(t) = ground(world, from, to, nearest) {
        nearest = t;
    }
    from.lerp(to, nearest.clamp(0.0, 1.0))
}

/// Where along `from`..`to` (0..1) the ray first enters `foe`'s standing cylinder.
fn through(foe: &Foe, from: Vec3, to: Vec3) -> Option<f32> {
    let flat = |v: Vec3| Vec2::new(v.x, v.z);
    let (along, offset) = (flat(to) - flat(from), flat(from) - flat(foe.position));
    let radius = foes::FOE_RADIUS + 0.1;
    // Inside the circle for t in t0..t1: |offset + t·along|² ≤ r².
    let (qa, qb, qc) = (along.length_squared(), 2.0 * offset.dot(along), offset.length_squared() - radius * radius);
    let (t0, t1) = if qa < 1e-8 {
        if qc > 0.0 {
            return None;
        }
        (0.0, 1.0)
    } else {
        let disc = qb * qb - 4.0 * qa * qc;
        if disc < 0.0 {
            return None;
        }
        let root = disc.sqrt();
        ((-qb - root) / (2.0 * qa), (-qb + root) / (2.0 * qa))
    };
    let (t0, t1) = (t0.max(0.0), t1.min(1.0));
    if t0 > t1 {
        return None;
    }
    // The first part of that stretch within the foe's height.
    let (bottom, top) = (foe.position.y, foe.position.y + foe.height());
    let (y0, y1) = (from.lerp(to, t0).y, from.lerp(to, t1).y);
    if (bottom..=top).contains(&y0) {
        return Some(t0);
    }
    let rise = y1 - y0;
    if rise.abs() < 1e-6 {
        return None;
    }
    let edge = if y0 > top { top } else { bottom };
    let t = t0 + (edge - y0) / rise * (t1 - t0);
    (t0..=t1).contains(&t).then_some(t)
}

/// Where along `from`..`to` the ray goes under the ground, before `before`.
fn ground(world: &World, from: Vec3, to: Vec3, before: f32) -> Option<f32> {
    let under = |t: f32| {
        let p = from.lerp(to, t);
        p.y < world.ground_height(p.x, p.z)
    };
    if under(0.0) {
        return Some(0.0);
    }
    let mut last = 0.0;
    for sample in 1..=GROUND_SAMPLES {
        let t = sample as f32 / GROUND_SAMPLES as f32;
        if t > before {
            return None;
        }
        if under(t) {
            let (mut above, mut below) = (last, t);
            for _ in 0..GROUND_REFINE {
                let middle = (above + below) * 0.5;
                if under(middle) { below = middle } else { above = middle }
            }
            return Some(below.min(before));
        }
        last = t;
    }
    None
}
