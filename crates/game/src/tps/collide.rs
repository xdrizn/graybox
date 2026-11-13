//! What the hero bumps into (G11.4): the world's solid boxes stop a standing cylinder, their tops
//! are ground to stand on, and the camera's line of sight stops at them and at the terrain.
use super::hero::{HEIGHT, RADIUS};
use super::props::Solid;
use super::world::World;
use glam::{Vec2, Vec3};

/// The hero walks up anything this much higher than its feet (a paving stone, a low step).
pub(crate) const STEP: f32 = 0.3;

/// Whether the cylinder of the hero (feet at `feet`) is beside `solid` in height (not over or under it).
fn beside(solid: &Solid, feet: Vec3) -> bool {
    solid.max.y > feet.y + STEP && solid.min.y < feet.y + HEIGHT
}

/// The point of `solid`'s footprint nearest to `p` (in XZ).
fn nearest(solid: &Solid, p: Vec3) -> Vec2 {
    Vec2::new(p.x.clamp(solid.min.x, solid.max.x), p.z.clamp(solid.min.z, solid.max.z))
}

/// How far `p` is from `solid`'s footprint (0 inside it).
pub(crate) fn footprint_distance(solid: &Solid, p: Vec3) -> f32 {
    (Vec2::new(p.x, p.z) - nearest(solid, p)).length()
}

/// Push the hero (feet at `feet`) out of every solid it overlaps, and take the part of `velocity`
/// that runs into them away. Two passes settle corners. `from_y` is the feet's height before this
/// move: a box whose top they were above is landed on (see [`ground`]), not pushed out of, however
/// fast the fall.
pub(crate) fn push_out(world: &World, feet: &mut Vec3, velocity: &mut Vec3, from_y: f32) {
    for _ in 0..2 {
        for solid in world.solids() {
            let higher = Vec3::new(feet.x, feet.y.max(from_y), feet.z);
            if !solid.is_finite() || !beside(solid, higher) {
                continue;
            }
            let here = Vec2::new(feet.x, feet.z);
            let gap = here - nearest(solid, *feet);
            let distance = gap.length();
            if distance >= RADIUS {
                continue;
            }
            let (normal, depth) = if distance > 1e-4 {
                (gap / distance, RADIUS - distance)
            } else {
                // The middle is inside the box: leave by the nearest side.
                let sides = [
                    (Vec2::NEG_X, feet.x - solid.min.x),
                    (Vec2::X, solid.max.x - feet.x),
                    (Vec2::NEG_Y, feet.z - solid.min.z),
                    (Vec2::Y, solid.max.z - feet.z),
                ];
                let (normal, inside) = sides.into_iter().fold((Vec2::X, f32::MAX), |best, side| if side.1 < best.1 { side } else { best });
                (normal, inside + RADIUS)
            };
            feet.x += normal.x * depth;
            feet.z += normal.y * depth;
            let into = velocity.x * normal.x + velocity.z * normal.y;
            if into < 0.0 {
                velocity.x -= normal.x * into;
                velocity.z -= normal.y * into;
            }
        }
    }
}

/// The ground under the hero: the terrain, or the top of a solid under its feet that is no higher
/// than `reach` (so a box top is ground once the hero is on it, or steps or falls onto it).
pub(crate) fn ground(world: &World, feet: Vec3, reach: f32) -> f32 {
    let terrain = world.ground_height(feet.x, feet.z);
    world
        .solids()
        .iter()
        .filter(|solid| solid.is_finite() && solid.max.y <= reach && solid.max.y > terrain && footprint_distance(solid, feet) < RADIUS * 0.6)
        .fold(terrain, |top, solid| top.max(solid.max.y))
}

/// Whether the hero standing at `feet` would overlap a solid.
pub(crate) fn blocked(world: &World, feet: Vec3) -> bool {
    blocked_below(world, feet, f32::INFINITY)
}

/// The same, counting only solids whose top is lower than `top` (what stands in front of a climb
/// wall, not the wall and the ledge on top of it).
pub(crate) fn blocked_below(world: &World, feet: Vec3, top: f32) -> bool {
    world.solids().iter().any(|solid| solid.is_finite() && solid.max.y < top && beside(solid, feet) && footprint_distance(solid, feet) < RADIUS)
}

/// Where along the segment `from`..`to` (0..1) it first enters `solid`, if it does.
fn ray_box(solid: &Solid, from: Vec3, to: Vec3) -> Option<f32> {
    let direction = to - from;
    let (mut near, mut far) = (0.0_f32, 1.0_f32);
    for axis in 0..3 {
        let (origin, step, low, high) = (from[axis], direction[axis], solid.min[axis], solid.max[axis]);
        if step.abs() < 1e-6 {
            if origin < low || origin > high {
                return None;
            }
            continue;
        }
        let (a, b) = ((low - origin) / step, (high - origin) / step);
        near = near.max(a.min(b));
        far = far.min(a.max(b));
        if near > far {
            return None;
        }
    }
    Some(near)
}

/// Where along `from`..`to` (0..1) a straight path first enters a solid, if it does (an arrow).
pub(crate) fn first_solid(world: &World, from: Vec3, to: Vec3) -> Option<f32> {
    if !from.is_finite() || !to.is_finite() {
        return None;
    }
    world.solids().iter().filter(|solid| solid.is_finite()).filter_map(|solid| ray_box(solid, from, to)).reduce(f32::min)
}

/// The part of the way from `target` to `eye` (0..1) that is clear of solids and terrain, so the
/// camera can stop short of a wall or a hill instead of looking through it.
pub(crate) fn clear_sight(world: &World, target: Vec3, eye: Vec3) -> f32 {
    if !target.is_finite() || !eye.is_finite() {
        return 1.0;
    }
    let mut clear = world
        .solids()
        .iter()
        .filter(|solid| solid.is_finite())
        // A box the target is inside of (the hero standing in a doorway) does not count.
        .filter(|solid| !(target.cmpge(solid.min).all() && target.cmple(solid.max).all()))
        .filter_map(|solid| ray_box(solid, target, eye))
        .fold(1.0_f32, f32::min);
    const SAMPLES: usize = 16;
    for sample in 1..=SAMPLES {
        let t = sample as f32 / SAMPLES as f32;
        if t >= clear {
            break;
        }
        let point = target.lerp(eye, t);
        if point.y < world.ground_height(point.x, point.z) + 0.15 {
            clear = clear.min((sample - 1) as f32 / SAMPLES as f32 + 0.5 / SAMPLES as f32);
            break;
        }
    }
    clear.clamp(0.0, 1.0)
}
