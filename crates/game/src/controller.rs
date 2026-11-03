use crate::arena::SceneObject;
use crate::state::{Input, PLAYER_RADIUS, Player, STANDING_HEIGHT};
use glam::Vec3;

const STEP_HEIGHT: f32 = 0.45;
const SNAP: f32 = 0.4;
const EPSILON: f32 = 0.0001;

fn overlaps_xz(position: Vec3, object: &SceneObject) -> bool {
    let half = object.size * 0.5;
    (position.x - object.position.x).abs() < half.x + PLAYER_RADIUS - EPSILON
        && (position.z - object.position.z).abs() < half.z + PLAYER_RADIUS - EPSILON
}

fn ramp_height(position: Vec3, object: &SceneObject) -> Option<f32> {
    let angle = object.rotation.x;
    let relative_z = position.z - object.position.z;
    // Plane of the upper face of the same rotated box used by rendering and raycasts.
    let y = object.position.y + object.size.y * 0.5 / angle.cos() - relative_z * angle.tan();
    let local_z = relative_z / angle.cos() - object.size.y * 0.5 * angle.tan();
    if (position.x - object.position.x).abs() <= object.size.x * 0.5 + PLAYER_RADIUS - EPSILON && local_z.abs() <= object.size.z * 0.5 + PLAYER_RADIUS
    {
        Some(y.max(0.0))
    } else {
        None
    }
}

fn top_at(position: Vec3, object: &SceneObject) -> Option<f32> {
    if !object.collision {
        return None;
    }
    if object.is_ramp() {
        ramp_height(position, object)
    } else if overlaps_xz(position, object) {
        Some(object.position.y + object.size.y * 0.5)
    } else {
        None
    }
}

fn support(objects: &[SceneObject], position: Vec3, ceiling: f32) -> Option<f32> {
    objects.iter().filter_map(|object| top_at(position, object)).filter(|height| *height <= ceiling + EPSILON).max_by(f32::total_cmp)
}

fn clear(objects: &[SceneObject], position: Vec3, height: f32) -> bool {
    objects.iter().all(|object| {
        if !object.collision {
            return true;
        }
        if object.is_ramp() {
            return ramp_height(position, object).is_none_or(|top| {
                let bottom = top - object.size.y / object.rotation.x.cos();
                position.y >= top - EPSILON || position.y + height <= bottom + EPSILON
            });
        }
        let min_y = object.position.y - object.size.y * 0.5;
        let max_y = object.position.y + object.size.y * 0.5;
        !overlaps_xz(position, object) || position.y >= max_y - EPSILON || position.y + height <= min_y + EPSILON
    })
}

/// Axis-separated horizontal sweep with grounded step-up, then a swept vertical landing/ceiling
/// pass. At maximum running speed each substep is < radius, so thin walls cannot be skipped.
pub(crate) fn advance(player: &mut Player, input: Input, objects: &[SceneObject], dt: f32) {
    if !player.position.is_finite() {
        player.position = crate::state::SPAWN;
        player.vertical_speed = 0.0;
        player.grounded = false;
    }
    if input.crouch {
        player.crouching = true;
    } else if clear(objects, player.position, STANDING_HEIGHT) {
        player.crouching = false;
    }

    player.grounded = player.vertical_speed <= 0.0
        && support(objects, player.position, player.position.y + 0.02).is_some_and(|y| (player.position.y - y).abs() <= 0.025);
    if input.jump && player.grounded {
        player.vertical_speed = 7.0;
        player.grounded = false;
    }
    move_horizontal(player, input, objects, dt);
    move_vertical(player, objects, dt);
}

fn axis_input(value: f32) -> f32 {
    if value.is_finite() { value.clamp(-1.0, 1.0) } else { 0.0 }
}

fn move_horizontal(player: &mut Player, input: Input, objects: &[SceneObject], dt: f32) {
    let forward = axis_input(input.forward);
    let strafe = axis_input(input.strafe);
    let wish = Vec3::new(player.yaw.sin() * forward + player.yaw.cos() * strafe, 0.0, -player.yaw.cos() * forward + player.yaw.sin() * strafe)
        .clamp_length_max(1.0);
    let speed = if player.crouching {
        2.3
    } else if input.run {
        8.0
    } else {
        4.6
    };
    let delta = wish * speed * dt;
    let before = player.position;
    for axis in [0, 2] {
        let mut candidate = player.position;
        candidate[axis] += delta[axis];
        if player.grounded
            && let Some(y) = support(objects, candidate, player.position.y + STEP_HEIGHT).filter(|y| *y > candidate.y)
        {
            candidate.y = y;
        }
        if clear(objects, candidate, player.height()) {
            player.position = candidate;
        }
    }
    if (player.position - before).length_squared() > 0.000001 {
        player.bob = (player.bob + dt * if player.crouching { 7.0 } else { 11.0 }) % std::f32::consts::TAU;
    }
}

fn move_vertical(player: &mut Player, objects: &[SceneObject], dt: f32) {
    let previous_y = player.position.y;
    player.vertical_speed -= 18.0 * dt;
    let proposed_y = previous_y + player.vertical_speed * dt;
    if player.vertical_speed <= 0.0 {
        let floor = support(objects, player.position, previous_y + EPSILON);
        if let Some(y) = floor.filter(|y| proposed_y <= *y || (player.grounded && previous_y - *y <= SNAP)) {
            player.position.y = y;
            player.vertical_speed = 0.0;
            player.grounded = true;
            return;
        }
    } else if let Some(ceiling) = ceiling_between(player, objects, proposed_y) {
        player.position.y = ceiling - player.height();
        player.vertical_speed = 0.0;
        return;
    }
    player.position.y = proposed_y;
    player.grounded = false;
}

fn ceiling_between(player: &Player, objects: &[SceneObject], proposed_y: f32) -> Option<f32> {
    let mut ceiling = f32::INFINITY;
    for object in objects.iter().filter(|object| object.collision) {
        let bottom = if object.is_ramp() {
            let Some(top) = ramp_height(player.position, object) else {
                continue;
            };
            top - object.size.y / object.rotation.x.cos()
        } else {
            if !overlaps_xz(player.position, object) {
                continue;
            }
            object.position.y - object.size.y * 0.5
        };
        if player.position.y + player.height() <= bottom + EPSILON && proposed_y + player.height() >= bottom {
            ceiling = ceiling.min(bottom);
        }
    }
    ceiling.is_finite().then_some(ceiling)
}
