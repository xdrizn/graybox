//! Arrows (G11.5): shot from the bow along where the camera looks, they fly fast and drop a
//! little, hit the first foe in their way, and stick in solids and the ground for a while.
use super::collide;
use super::damage::Events;
use super::foes::{self, Foe};
use super::world::World;
use crate::state::GameEvent;
use glam::{Vec2, Vec3};

/// The most arrows in the air or stuck at once (the oldest goes first).
pub const MAX_ARROWS: usize = 16;
pub const ARROW_SPEED: f32 = 32.0;
const ARROW_GRAVITY: f32 = 9.0;
pub const ARROW_DAMAGE: f32 = 25.0;
const ARROW_PUSH: f32 = 2.5;
/// Seconds an arrow flies before it is gone, and stays stuck after it lands.
const FLIGHT: f32 = 3.0;
const STUCK: f32 = 6.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Arrow {
    pub position: Vec3,
    pub(crate) previous_position: Vec3,
    pub velocity: Vec3,
    /// Seconds since it stuck in something, once it did.
    pub stuck: Option<f32>,
    age: f32,
}

/// An arrow as drawn this display frame: its tip and the way it points.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ArrowPose {
    pub position: Vec3,
    pub direction: Vec3,
}

/// The way to shoot from `from` so the arrow comes down on `at` (aimed a little high for the drop);
/// `fallback` when `at` is too close to tell.
pub(crate) fn toward(from: Vec3, at: Vec3, fallback: Vec3) -> Vec3 {
    let gap = at - from;
    let length = gap.length();
    if !length.is_finite() || length < 0.5 {
        return fallback;
    }
    let time = length / ARROW_SPEED;
    (gap / time + Vec3::Y * (0.5 * ARROW_GRAVITY * time)).normalize_or(fallback)
}

pub(crate) fn shoot(arrows: &mut Vec<Arrow>, from: Vec3, direction: Vec3, events: &mut Events) {
    let direction = direction.normalize_or_zero();
    if !from.is_finite() || direction == Vec3::ZERO {
        return;
    }
    if arrows.len() >= MAX_ARROWS {
        arrows.remove(0);
    }
    arrows.push(Arrow { position: from, previous_position: from, velocity: direction * ARROW_SPEED, stuck: None, age: 0.0 });
    events.push(GameEvent::Shot);
}

/// Where along `from`..`to` (0..1) it passes through `foe`'s standing cylinder, if it does.
fn through(foe: &Foe, from: Vec3, to: Vec3) -> Option<f32> {
    let flat = |v: Vec3| Vec2::new(v.x, v.z);
    let (a, b, centre) = (flat(from), flat(to), flat(foe.position));
    let along = b - a;
    let length = along.length_squared();
    let t = if length > 1e-8 { ((centre - a).dot(along) / length).clamp(0.0, 1.0) } else { 0.0 };
    let point = from.lerp(to, t);
    let close = (flat(point) - centre).length() < foes::FOE_RADIUS + 0.1;
    let height = point.y >= foe.position.y && point.y <= foe.position.y + foe.height();
    (close && height).then_some(t)
}

pub(crate) fn step(arrows: &mut Vec<Arrow>, foes: &mut [Foe], world: &World, events: &mut Events, dt: f32) {
    for arrow in arrows.iter_mut() {
        arrow.previous_position = arrow.position;
        arrow.age += dt;
        if let Some(time) = &mut arrow.stuck {
            *time += dt;
            continue;
        }
        let from = arrow.position;
        let to = from + arrow.velocity * dt;
        arrow.velocity.y -= ARROW_GRAVITY * dt;
        let wall = collide::first_solid(world, from, to);
        let target = foes
            .iter()
            .enumerate()
            .filter(|(_, foe)| foe.alive())
            .filter_map(|(index, foe)| through(foe, from, to).map(|t| (index, t)))
            .min_by(|a, b| a.1.total_cmp(&b.1));
        match (target, wall) {
            (Some((index, t)), wall) if wall.is_none_or(|w| t <= w) => {
                if let Some(foe) = foes.get_mut(index) {
                    let push = Vec3::new(arrow.velocity.x, 0.0, arrow.velocity.z).normalize_or_zero() * ARROW_PUSH;
                    foes::hit(foe, ARROW_DAMAGE, push, events);
                }
                // Spent: it falls away (gone at once).
                arrow.age = FLIGHT;
                arrow.position = from.lerp(to, t);
            }
            (_, Some(w)) => {
                arrow.position = from.lerp(to, w);
                arrow.stuck = Some(0.0);
            }
            _ => {
                arrow.position = to;
                if to.y <= world.ground_height(to.x, to.z) {
                    arrow.position.y = world.ground_height(to.x, to.z);
                    arrow.stuck = Some(0.0);
                }
            }
        }
    }
    arrows.retain(|arrow| arrow.position.is_finite() && arrow.stuck.map_or(arrow.age < FLIGHT, |time| time < STUCK));
}

pub(crate) fn pose(arrow: &Arrow, alpha: f32) -> ArrowPose {
    let alpha = if alpha.is_finite() { alpha.clamp(0.0, 1.0) } else { 1.0 };
    ArrowPose { position: arrow.previous_position.lerp(arrow.position, alpha), direction: arrow.velocity.normalize_or_zero() }
}
