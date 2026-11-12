//! How the hero moves (G11.4): 360° relative to the camera, with acceleration and smoothed facing;
//! it stops at the world's solid boxes and stands on their tops; it jumps with coyote time and a
//! buffered press; it rolls (a dodge on the ground, or on landing to take a fall); it climbs ladders,
//! mantles onto ledges and climbs the rock wall; running, rolling and the wall cost stamina; a hard
//! fall without a roll knocks it down. What the weapons do is in `combat.rs`.
use super::animator::smooth;
use super::clip::ClipId;
use super::clips::clip;
use super::collide::{self, STEP};
use super::combat;
use super::hero::{Hero, RADIUS, wrap_pi};
use super::props::{ClimbKind, Climbable};
use super::world::World;
use crate::state::Input;
use glam::{Vec2, Vec3};

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
/// A jump still works this long after walking off an edge, and a press this long before landing counts.
const COYOTE: f32 = 0.12;
const JUMP_BUFFER: f32 = 0.12;
/// A roll pressed this long before landing turns the landing into a roll.
const ROLL_BUFFER: f32 = 0.3;
/// Walking down a slope or off a step lower than this keeps the feet on the ground.
const SNAP: f32 = 0.45;

const ROLL_TIME: f32 = 0.7;
const ROLL_SPEED: f32 = 7.5;
/// Falling more than this (metres) without rolling knocks the hero down.
const HARD_FALL: f32 = 7.0;
const KNOCKDOWN_TIME: f32 = 0.9;

pub const STAMINA_MAX: f32 = 100.0;
const RUN_COST: f32 = 8.0;
const ROLL_COST: f32 = 15.0;
const WALL_COST: f32 = 10.0;
const WALL_HOLD_COST: f32 = 3.0;
const STAMINA_REGEN: f32 = 25.0;
/// Stamina comes back this long after it was last spent; once spent out, running waits for this much.
const REGEN_DELAY: f32 = 0.8;
const TIRED_UNTIL: f32 = 25.0;

const LADDER_SPEED: f32 = 1.8;
const WALL_SPEED: f32 = 1.3;
/// A ledge this much higher than the feet is mantled onto by moving into it.
const MANTLE_MIN: f32 = 0.45;
const MANTLE_MAX: f32 = 2.0;

/// What the hero is doing besides walking about.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Action {
    /// On the ground or in the air, moving freely (attacks happen here too).
    #[default]
    Free,
    Roll {
        time: f32,
        direction: Vec3,
    },
    /// On the ladder `index` of [`World::climbables`].
    Ladder {
        index: usize,
    },
    /// On the climb wall `index` of [`World::climbables`].
    Wall {
        index: usize,
    },
    /// Pulling up onto a ledge (or off the top of a ladder or wall).
    Mantle {
        time: f32,
        duration: f32,
        from: Vec3,
        to: Vec3,
    },
    /// Knocked down by a hard fall; it gets up by itself.
    Down {
        time: f32,
        getting_up: bool,
    },
}

impl Action {
    pub fn name(&self) -> &'static str {
        match self {
            Action::Free => "free",
            Action::Roll { .. } => "roll",
            Action::Ladder { .. } => "ladder",
            Action::Wall { .. } => "wall",
            Action::Mantle { .. } => "mantle",
            Action::Down { .. } => "down",
        }
    }
}

/// The presses of one step (each counts once).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Presses {
    pub jump: bool,
    pub roll: bool,
    pub attack: bool,
    /// 1 sword, 2 spear, 3 club, 4 bow; 0 is no press.
    pub weapon: u8,
}

/// The movement state kept between steps.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Moves {
    pub action: Action,
    pub stamina: f32,
    /// Spent out: running waits until stamina is back to `TIRED_UNTIL`.
    tired: bool,
    /// Seconds since stamina was last spent.
    rest: f32,
    since_ground: f32,
    jump_buffer: f32,
    roll_buffer: f32,
    /// The highest the feet have been since the hero left the ground.
    highest: f32,
}

impl Default for Moves {
    fn default() -> Self {
        Self {
            action: Action::Free,
            stamina: STAMINA_MAX,
            tired: false,
            rest: REGEN_DELAY,
            since_ground: 0.0,
            jump_buffer: 0.0,
            roll_buffer: 0.0,
            highest: f32::MIN,
        }
    }
}

impl Moves {
    fn spend(&mut self, amount: f32) {
        self.stamina = (self.stamina - amount.max(0.0)).max(0.0);
        self.rest = 0.0;
        if self.stamina <= 0.0 {
            self.tired = true;
        }
    }
}

fn axis(value: f32) -> f32 {
    if value.is_finite() { value.clamp(-1.0, 1.0) } else { 0.0 }
}

/// The facing (yaw) that looks along `direction` (same convention as the camera: 0 looks along -Z).
fn yaw_of(direction: Vec3) -> f32 {
    direction.x.atan2(-direction.z)
}

/// The unit vector a facing looks along.
fn forward_of(facing: f32) -> Vec3 {
    Vec3::new(facing.sin(), 0.0, -facing.cos())
}

/// Where the player wants to go, relative to the camera (length up to 1).
fn wish(input: &Input, camera_yaw: f32) -> Vec3 {
    let (forward, strafe) = (axis(input.forward), axis(input.strafe));
    let (sin, cos) = if camera_yaw.is_finite() { camera_yaw.sin_cos() } else { (0.0, 1.0) };
    // Same convention as the first-person game: yaw 0 looks along -Z, positive yaw turns right.
    Vec3::new(sin * forward + cos * strafe, 0.0, -cos * forward + sin * strafe).clamp_length_max(1.0)
}

/// One fixed step. `camera_yaw` says which way "forward" is.
pub(super) fn step(hero: &mut Hero, input: &Input, camera_yaw: f32, world: &World, presses: Presses, dt: f32) {
    if !hero.position.is_finite() || !hero.velocity.is_finite() || !hero.facing.is_finite() {
        *hero = Hero::at(world.spawn());
    }
    hero.previous_position = hero.position;
    hero.previous_facing = hero.facing;
    let wish = wish(input, camera_yaw);
    let moves = &mut hero.moves;
    moves.jump_buffer = if presses.jump { JUMP_BUFFER } else { (moves.jump_buffer - dt).max(0.0) };
    moves.roll_buffer = if presses.roll { ROLL_BUFFER } else { (moves.roll_buffer - dt).max(0.0) };
    moves.rest += dt;

    match hero.moves.action {
        Action::Free => free(hero, input, wish, camera_yaw, world, dt),
        Action::Roll { time, direction } => roll(hero, world, time, direction, dt),
        Action::Ladder { index } => ladder(hero, input, world, index, presses, dt),
        Action::Wall { index } => wall(hero, input, world, index, presses, dt),
        Action::Mantle { time, duration, from, to } => mantle(hero, time, duration, from, to, dt),
        Action::Down { time, getting_up } => down(hero, time, getting_up, dt),
    }

    let moves = &mut hero.moves;
    if moves.rest >= REGEN_DELAY {
        moves.stamina = (moves.stamina + STAMINA_REGEN * dt).min(STAMINA_MAX);
    }
    if moves.tired && moves.stamina >= TIRED_UNTIL {
        moves.tired = false;
    }
}

/// Move by the velocity, stop at solids and settle on the ground. Returns whether it just landed.
fn travel(hero: &mut Hero, world: &World, dt: f32) -> bool {
    let was_grounded = hero.grounded;
    let before = hero.position;
    let mut position = world.keep_on_map(hero.position + hero.velocity * dt);
    let mut velocity = hero.velocity;
    collide::push_out(world, &mut position, &mut velocity, before.y);
    let floor = collide::ground(world, position, position.y.max(before.y) + STEP);
    let mut landed = false;
    if position.y <= floor && velocity.y <= 0.0 {
        position.y = floor;
        velocity.y = 0.0;
        landed = !was_grounded;
        hero.grounded = true;
    } else if was_grounded && velocity.y <= 0.0 && position.y - floor < SNAP {
        // Walking down a slope or off a low step: stay on the ground.
        position.y = floor;
        velocity.y = 0.0;
        hero.grounded = true;
    } else {
        hero.grounded = false;
    }
    hero.position = position;
    hero.velocity = velocity;
    landed
}

fn free(hero: &mut Hero, input: &Input, wish: Vec3, camera_yaw: f32, world: &World, dt: f32) {
    let pace = combat::pace(&hero.combat);
    let running = input.run && !hero.moves.tired && wish.length_squared() > 0.01;
    let mut top = if input.crouch {
        CROUCH_SPEED
    } else if running {
        RUN_SPEED
    } else {
        JOG_SPEED
    };
    if let Some(limit) = pace.limit {
        top = top.min(limit);
    }
    if running && hero.grounded && hero.speed() > JOG_SPEED + 0.5 {
        hero.moves.spend(RUN_COST * dt);
    }

    // Roll on the ground (a dodge): where the stick points, or straight ahead.
    if hero.moves.roll_buffer > 0.0 && hero.grounded && !pace.busy && hero.moves.stamina >= ROLL_COST {
        let direction = if wish.length_squared() > 0.01 { wish.normalize() } else { forward_of(hero.facing) };
        start_roll(hero, direction);
        return;
    }

    let accel = if hero.grounded { GROUND_ACCEL } else { AIR_ACCEL };
    let current = Vec3::new(hero.velocity.x, 0.0, hero.velocity.z);
    let mut horizontal = current + (wish * top - current).clamp_length_max(accel * dt);
    if pace.lunge > 0.0 && hero.grounded {
        horizontal = horizontal.lerp(forward_of(hero.facing) * pace.lunge, 0.5);
    }

    let moves = &mut hero.moves;
    moves.since_ground = if hero.grounded { 0.0 } else { moves.since_ground + dt };
    let mut vertical = hero.velocity.y;
    if moves.jump_buffer > 0.0 && moves.since_ground <= COYOTE && !pace.busy {
        vertical = JUMP_SPEED;
        moves.jump_buffer = 0.0;
        moves.since_ground = COYOTE + 1.0;
        hero.grounded = false;
    }
    vertical -= GRAVITY * dt;
    hero.velocity = Vec3::new(horizontal.x, vertical, horizontal.z);

    if travel(hero, world, dt) {
        let drop = hero.moves.highest - hero.position.y;
        if hero.moves.roll_buffer > 0.0 {
            // A roll pressed just before landing takes the fall.
            let direction = if horizontal.length_squared() > 0.25 { horizontal.normalize() } else { forward_of(hero.facing) };
            start_roll(hero, direction);
            return;
        }
        if drop > HARD_FALL {
            hero.moves.action = Action::Down { time: 0.0, getting_up: false };
            hero.velocity = Vec3::ZERO;
            hero.animator.play(ClipId::Knockdown);
            return;
        }
    }
    if hero.grounded {
        hero.moves.highest = hero.position.y;
    } else {
        hero.moves.highest = hero.moves.highest.max(hero.position.y);
    }

    if !pace.busy && start_climb(hero, world, wish) {
        return;
    }

    // Face where the player wants to go (or where the camera looks while aiming), turning smoothly.
    let wanted = if pace.face_camera { Some(camera_yaw) } else { (wish.length_squared() > 0.01).then(|| yaw_of(wish)) };
    if let Some(wanted) = wanted.filter(|w| w.is_finite()) {
        let rate = FACING_RATE * pace.turn;
        hero.facing = wrap_pi(hero.facing + wrap_pi(wanted - hero.facing) * (1.0 - (-rate * dt).exp()));
    }
}

fn start_roll(hero: &mut Hero, direction: Vec3) {
    hero.moves.spend(ROLL_COST);
    hero.moves.roll_buffer = 0.0;
    hero.moves.action = Action::Roll { time: 0.0, direction };
    hero.facing = yaw_of(direction);
    hero.animator.play(ClipId::LandRoll);
}

fn roll(hero: &mut Hero, world: &World, time: f32, direction: Vec3, dt: f32) {
    let time = time + dt;
    let speed = ROLL_SPEED * (1.0 - 0.45 * (time / ROLL_TIME).min(1.0));
    hero.velocity = Vec3::new(direction.x * speed, hero.velocity.y - GRAVITY * dt, direction.z * speed);
    travel(hero, world, dt);
    hero.moves.highest = hero.position.y;
    // Rolling off an edge counts as leaving the ground (coyote time runs from there).
    hero.moves.since_ground = if hero.grounded { 0.0 } else { hero.moves.since_ground + dt };
    hero.moves.action = if time >= ROLL_TIME { Action::Free } else { Action::Roll { time, direction } };
}

fn down(hero: &mut Hero, time: f32, getting_up: bool, dt: f32) {
    let time = time + dt;
    hero.velocity = Vec3::ZERO;
    let get_up = clip(ClipId::GetUp);
    hero.moves.action = if !getting_up && time >= KNOCKDOWN_TIME {
        hero.animator.play(ClipId::GetUp);
        Action::Down { time: 0.0, getting_up: true }
    } else if getting_up && time >= get_up.duration - get_up.fade_out {
        Action::Free
    } else {
        Action::Down { time, getting_up }
    };
}

// ---- Climbing ---------------------------------------------------------------------------------

/// Half the depth of a climbable along its normal.
fn depth(climbable: &Climbable) -> f32 {
    ((climbable.max - climbable.min) * 0.5).dot(climbable.normal.abs())
}

/// The middle of a climbable's footprint, at its bottom.
fn middle(climbable: &Climbable) -> Vec3 {
    let middle = (climbable.min + climbable.max) * 0.5;
    Vec3::new(middle.x, climbable.min.y, middle.z)
}

/// Where the hero hangs on a ladder (XZ), facing it.
fn ladder_spot(ladder: &Climbable) -> Vec3 {
    middle(ladder) + ladder.normal * (depth(ladder) + RADIUS)
}

/// Sideways along a wall, to the right of a hero facing it.
fn wall_side(wall: &Climbable) -> Vec3 {
    Vec3::new(wall.normal.z, 0.0, -wall.normal.x)
}

/// Grab a ladder, the wall or a ledge the hero walks into. Returns whether it did.
fn start_climb(hero: &mut Hero, world: &World, wish: Vec3) -> bool {
    if wish.length_squared() < 0.25 {
        return false;
    }
    let feet = hero.position;
    let flat = |v: Vec3| Vec2::new(v.x, v.z);
    for (index, climbable) in world.climbables().iter().enumerate() {
        if !climbable.min.is_finite() || !climbable.max.is_finite() || !climbable.normal.is_finite() {
            continue;
        }
        match climbable.kind {
            ClimbKind::Ladder => {
                let spot = ladder_spot(climbable);
                // In front of the ladder: up to 0.8 m to either side, from touching it to 0.6 m back.
                let offset = feet - spot;
                let close = offset.dot(wall_side(climbable)).abs() < 0.8 && (-0.4..0.6).contains(&offset.dot(climbable.normal));
                let height = feet.y >= climbable.min.y - 0.6 && feet.y <= climbable.max.y - 1.3;
                if close && height && wish.dot(-climbable.normal) > 0.5 {
                    hero.moves.action = Action::Ladder { index };
                    hero.velocity = Vec3::ZERO;
                    hero.position = Vec3::new(spot.x, feet.y.max(climbable.min.y), spot.z);
                    hero.facing = yaw_of(-climbable.normal);
                    hero.animator.play(ClipId::ClimbLadder);
                    hero.animator.set_rate(0.0);
                    return true;
                }
            }
            ClimbKind::Wall => {
                let side = wall_side(climbable);
                let centre = middle(climbable);
                let along = (feet - centre).dot(side);
                let half = ((climbable.max - climbable.min) * 0.5).dot(side.abs());
                let out = (feet - centre).dot(climbable.normal) - depth(climbable);
                let height = feet.y >= climbable.min.y - 0.5 && feet.y <= climbable.max.y - 1.5;
                if along.abs() < half - 0.3
                    && (-0.2..RADIUS + 0.35).contains(&out)
                    && height
                    && wish.dot(-climbable.normal) > 0.5
                    && hero.moves.stamina > 10.0
                {
                    hero.moves.action = Action::Wall { index };
                    hero.velocity = Vec3::ZERO;
                    let spot = centre + side * along + climbable.normal * (depth(climbable) + 0.05);
                    hero.position = Vec3::new(spot.x, feet.y.max(climbable.min.y), spot.z);
                    hero.facing = yaw_of(-climbable.normal);
                    hero.animator.play(ClipId::ClimbWall);
                    hero.animator.set_rate(0.0);
                    return true;
                }
            }
            ClimbKind::Mantle => {
                let rise = climbable.max.y - feet.y;
                if !(MANTLE_MIN..=MANTLE_MAX).contains(&rise) {
                    continue;
                }
                let near = Vec2::new(feet.x.clamp(climbable.min.x, climbable.max.x), feet.z.clamp(climbable.min.z, climbable.max.z));
                let gap = near - flat(feet);
                if gap.length() > RADIUS + 0.2 {
                    continue;
                }
                let toward = if gap.length() > 1e-3 { gap.normalize() } else { flat(wish).normalize_or_zero() };
                if flat(wish).dot(toward) < 0.5 {
                    continue;
                }
                let inside = near + toward * 0.45;
                let x = inside.x.clamp(climbable.min.x + 0.2, (climbable.max.x - 0.2).max(climbable.min.x + 0.2));
                let z = inside.y.clamp(climbable.min.z + 0.2, (climbable.max.z - 0.2).max(climbable.min.z + 0.2));
                let to = Vec3::new(x, climbable.max.y, z);
                if collide::blocked(world, to) {
                    continue;
                }
                start_mantle(hero, to, rise);
                return true;
            }
        }
    }
    false
}

fn start_mantle(hero: &mut Hero, to: Vec3, rise: f32) {
    // A low ledge is a quicker hop.
    let rate = if rise < 1.0 { 1.5 } else { 1.0 };
    let duration = clip(ClipId::Mantle).duration / rate;
    hero.moves.action = Action::Mantle { time: 0.0, duration, from: hero.position, to };
    hero.velocity = Vec3::ZERO;
    let across = to - hero.position;
    if Vec2::new(across.x, across.z).length() > 0.05 {
        hero.facing = yaw_of(Vec3::new(across.x, 0.0, across.z));
    }
    hero.animator.play(ClipId::Mantle);
    hero.animator.set_rate(rate);
}

fn mantle(hero: &mut Hero, time: f32, duration: f32, from: Vec3, to: Vec3, dt: f32) {
    let time = time + dt;
    let t = (time / duration.max(0.05)).min(1.0);
    // Up first, then over the edge.
    let up = smooth(t / 0.6);
    let across = smooth((t - 0.35) / 0.65);
    let position = Vec3::new(from.x + (to.x - from.x) * across, from.y + (to.y - from.y) * up, from.z + (to.z - from.z) * across);
    hero.velocity = (position - hero.position) / dt.max(1e-4);
    hero.position = position;
    if t >= 1.0 {
        hero.moves.action = Action::Free;
        hero.velocity = Vec3::ZERO;
        hero.grounded = true;
        hero.moves.highest = to.y;
        hero.moves.since_ground = 0.0;
    } else {
        hero.moves.action = Action::Mantle { time, duration, from, to };
        hero.grounded = true;
    }
}

/// Let go of a ladder or the wall: back to moving freely, pushed `away` (m/s).
fn let_go(hero: &mut Hero, away: Vec3) {
    hero.moves.action = Action::Free;
    hero.velocity = away;
    hero.grounded = false;
    hero.moves.highest = hero.position.y;
    // The press that let go is not also a jump (no coyote time off a ladder or the wall).
    hero.moves.since_ground = COYOTE + 1.0;
    hero.moves.jump_buffer = 0.0;
    hero.animator.stop();
}

/// Off the top of a ladder or the wall: pull up onto what is behind it. If something stands there,
/// the hero stays hanging at the top instead.
fn climb_over(hero: &mut Hero, world: &World, climbable: &Climbable) {
    let target = Vec3::new(hero.position.x, climbable.max.y, hero.position.z) - climbable.normal * (depth(climbable) * 2.0 + RADIUS + 0.5);
    let top = collide::ground(world, target, climbable.max.y + 0.7);
    let to = Vec3::new(target.x, top.max(climbable.max.y), target.z);
    if collide::blocked(world, to) {
        hero.position.y = hero.position.y.min(climbable.max.y - 1.1);
        hero.velocity = Vec3::ZERO;
        return;
    }
    let rise = to.y - hero.position.y;
    start_mantle(hero, to, rise);
}

fn ladder(hero: &mut Hero, input: &Input, world: &World, index: usize, presses: Presses, dt: f32) {
    let Some(ladder) = world.climbables().get(index).copied() else {
        hero.moves.action = Action::Free;
        return;
    };
    if presses.jump {
        let_go(hero, ladder.normal * 3.0 + Vec3::Y * 3.0);
        return;
    }
    let climb = axis(input.forward);
    let spot = ladder_spot(&ladder);
    let y = hero.position.y + climb * LADDER_SPEED * dt;
    hero.facing = yaw_of(-ladder.normal);
    hero.velocity = Vec3::Y * climb * LADDER_SPEED;
    hero.grounded = true;
    hero.animator.set_rate(climb * LADDER_SPEED / 0.8);
    let floor = collide::ground(world, Vec3::new(spot.x, y, spot.z), y + STEP);
    if climb < 0.0 && y <= floor {
        hero.position = Vec3::new(spot.x, floor, spot.z);
        let_go(hero, Vec3::ZERO);
        hero.grounded = true;
        return;
    }
    hero.position = Vec3::new(spot.x, y.max(floor), spot.z);
    if hero.position.y >= ladder.max.y - 1.1 {
        climb_over(hero, world, &ladder);
    }
}

fn wall(hero: &mut Hero, input: &Input, world: &World, index: usize, presses: Presses, dt: f32) {
    let Some(wall) = world.climbables().get(index).copied() else {
        hero.moves.action = Action::Free;
        return;
    };
    if presses.jump || hero.moves.stamina <= 0.0 {
        let_go(hero, wall.normal * 2.0);
        return;
    }
    let (up, across) = (axis(input.forward), axis(input.strafe));
    let moving = up.abs() + across.abs() > 0.1;
    hero.moves.spend(if moving { WALL_COST } else { WALL_HOLD_COST } * dt);
    let side = wall_side(&wall);
    let centre = middle(&wall);
    let half = ((wall.max - wall.min) * 0.5).dot(side.abs());
    let reach = (half - 0.4).max(0.0);
    let (was_along, was_y) = ((hero.position - centre).dot(side), hero.position.y);
    let along = (was_along + across * WALL_SPEED * dt).clamp(-reach, reach);
    let y = was_y + up * WALL_SPEED * dt;
    let place = |along: f32, y: f32| centre + side * along + wall.normal * (depth(&wall) + 0.05) + Vec3::Y * (y - centre.y);
    // Something standing in front of the wall (a pillar) stops the hero, up, down or sideways.
    let blocked = |at: Vec3| collide::blocked_below(world, at, wall.max.y);
    let (along, y) = if blocked(hero.position) || !blocked(place(along, y)) {
        (along, y)
    } else if !blocked(place(along, was_y)) {
        (along, was_y)
    } else if !blocked(place(was_along, y)) {
        (was_along, y)
    } else {
        (was_along, was_y)
    };
    let spot = place(along, y);
    hero.facing = yaw_of(-wall.normal);
    hero.velocity = Vec3::Y * up * WALL_SPEED + side * across * WALL_SPEED;
    hero.grounded = true;
    hero.animator.set_rate((up.abs() + across.abs()).min(1.0) * WALL_SPEED / 0.8);
    let floor = collide::ground(world, Vec3::new(spot.x, y, spot.z), y + STEP);
    if up < 0.0 && y <= floor {
        hero.position = Vec3::new(spot.x, floor, spot.z);
        let_go(hero, Vec3::ZERO);
        hero.grounded = true;
        return;
    }
    hero.position = Vec3::new(spot.x, y.max(floor), spot.z);
    if hero.position.y >= wall.max.y - 1.1 {
        climb_over(hero, world, &wall);
    }
}
