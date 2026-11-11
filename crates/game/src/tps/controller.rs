//! How the hero moves: 360° relative to the camera, with acceleration, smoothed facing and a jump.
//!
//! PLACEHOLDER (G11.1): walk, jog, run, crouch speed and a plain jump. G11.4 owns this file and
//! `camera.rs` (coyote time, buffering, rolls, ladders, stamina, collision with props).
use super::hero::{Hero, wrap_pi};
use super::world::World;
use crate::state::Input;
use glam::Vec3;

/// Metres a second at full stick.
const JOG_SPEED: f32 = 5.0;
const RUN_SPEED: f32 = 8.5;
const CROUCH_SPEED: f32 = 2.4;
/// How fast the velocity follows the wanted one (m/s²), on the ground and in the air.
const GROUND_ACCEL: f32 = 30.0;
const AIR_ACCEL: f32 = 8.0;
const GRAVITY: f32 = 20.0;
const JUMP_SPEED: f32 = 7.0;
/// How quickly the hero turns to face where it goes (1/s; about 0.1 s to close most of a turn).
const FACING_RATE: f32 = 10.0;

fn axis(value: f32) -> f32 {
    if value.is_finite() { value.clamp(-1.0, 1.0) } else { 0.0 }
}

/// One fixed step. `camera_yaw` says which way "forward" is; `jump` is a press since the last step.
pub(super) fn step(hero: &mut Hero, input: &Input, camera_yaw: f32, world: &World, jump: bool, dt: f32) {
    if !hero.position.is_finite() || !hero.velocity.is_finite() || !hero.facing.is_finite() {
        *hero = Hero::at(world.spawn());
    }
    hero.previous_position = hero.position;
    hero.previous_facing = hero.facing;

    let (forward, strafe) = (axis(input.forward), axis(input.strafe));
    let (sin, cos) = if camera_yaw.is_finite() { camera_yaw.sin_cos() } else { (0.0, 1.0) };
    // Same convention as the first-person game: yaw 0 looks along -Z, positive yaw turns right.
    let wish = Vec3::new(sin * forward + cos * strafe, 0.0, -cos * forward + sin * strafe).clamp_length_max(1.0);
    let top = if input.crouch {
        CROUCH_SPEED
    } else if input.run {
        RUN_SPEED
    } else {
        JOG_SPEED
    };
    let accel = if hero.grounded { GROUND_ACCEL } else { AIR_ACCEL };
    let current = Vec3::new(hero.velocity.x, 0.0, hero.velocity.z);
    let horizontal = current + (wish * top - current).clamp_length_max(accel * dt);

    let mut vertical = hero.velocity.y;
    if jump && hero.grounded {
        vertical = JUMP_SPEED;
        hero.grounded = false;
    }
    vertical -= GRAVITY * dt;
    hero.velocity = Vec3::new(horizontal.x, vertical, horizontal.z);
    hero.position = world.keep_on_map(hero.position + hero.velocity * dt);

    let ground = world.ground_height(hero.position.x, hero.position.z);
    if hero.position.y <= ground && hero.velocity.y <= 0.0 {
        hero.position.y = ground;
        hero.velocity.y = 0.0;
        hero.grounded = true;
    } else {
        hero.grounded = false;
    }

    // Face where the player wants to go, turning smoothly.
    if wish.length_squared() > 0.01 {
        let wanted = wish.x.atan2(-wish.z);
        hero.facing = wrap_pi(hero.facing + wrap_pi(wanted - hero.facing) * (1.0 - (-FACING_RATE * dt).exp()));
    }
}
