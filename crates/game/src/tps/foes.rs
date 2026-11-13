//! What the hero fights (G11.5): training dummies near the start (they wobble when hit and break
//! apart) and a few slow husks around the valley (original, faceted, orange slit eyes) that walk to
//! the hero, wind up, swing, stagger when hit and break apart. Both come back after a while.
use super::collide;
use super::controller::Action;
use super::damage::{self, Events};
use super::hero::{Hero, wrap_pi};
use super::terrain::{RIVER_HALF_WIDTH, river_z};
use super::world::World;
use crate::state::GameEvent;
use glam::{Vec2, Vec3};

/// The most foes the valley holds (and the renderer draws).
pub const MAX_FOES: usize = 12;
/// A foe is drawn and collides as a standing cylinder this wide…
pub const FOE_RADIUS: f32 = 0.4;
/// …and this tall (a husk; a dummy is a little shorter).
pub const HUSK_HEIGHT: f32 = 2.0;
pub const DUMMY_HEIGHT: f32 = 1.8;
/// Seconds the pieces of a broken foe fly and lie before they vanish.
pub const PIECES_TIME: f32 = 1.6;

const HUSK_HEALTH: f32 = 60.0;
const DUMMY_HEALTH: f32 = 45.0;
const HUSK_SPEED: f32 = 1.5;
/// A husk sees the hero this far away, and gives up this far from home.
const SIGHT: f32 = 16.0;
const LEASH: f32 = 26.0;
/// It starts its swing this close, and the swing reaches this far.
const ATTACK_RANGE: f32 = 1.7;
const REACH: f32 = 2.1;
const WIND_UP: f32 = 0.7;
const SWING: f32 = 0.35;
const RECOVER: f32 = 0.9;
const STAGGER: f32 = 0.45;
pub const HUSK_DAMAGE: f32 = 15.0;
const HUSK_PUSH: f32 = 6.0;
const HUSK_RESPAWN: f32 = 14.0;
const DUMMY_RESPAWN: f32 = 4.0;
const HIT_FLASH: f32 = 0.15;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FoeKind {
    Dummy,
    Husk,
}

impl FoeKind {
    pub fn name(self) -> &'static str {
        match self {
            FoeKind::Dummy => "Dummy",
            FoeKind::Husk => "Husk",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FoeState {
    Idle,
    Walk,
    /// Raising its arms; the swing lands when this ends.
    WindUp {
        time: f32,
    },
    Swing {
        time: f32,
    },
    Recover {
        time: f32,
    },
    /// Hit: it reels and its wind-up is lost.
    Stagger {
        time: f32,
    },
    /// Out of health: in pieces, then gone until it comes back.
    Broken {
        time: f32,
    },
}

impl FoeState {
    pub fn name(&self) -> &'static str {
        match self {
            FoeState::Idle => "idle",
            FoeState::Walk => "walk",
            FoeState::WindUp { .. } => "windUp",
            FoeState::Swing { .. } => "swing",
            FoeState::Recover { .. } => "recover",
            FoeState::Stagger { .. } => "stagger",
            FoeState::Broken { .. } => "broken",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Foe {
    pub kind: FoeKind,
    /// Where it stands at first and comes back to.
    pub home: Vec3,
    pub position: Vec3,
    pub(crate) previous_position: Vec3,
    pub facing: f32,
    /// Knockback being spent (m/s, sideways).
    pub push: Vec3,
    pub health: f32,
    pub state: FoeState,
    /// Seconds left of the white flash after a hit.
    pub flash: f32,
    /// Which way the last hit came from (a dummy leans away from it).
    pub hit_from: Vec3,
}

impl Foe {
    fn new(kind: FoeKind, home: Vec3, facing: f32) -> Self {
        Self {
            kind,
            home,
            position: home,
            previous_position: home,
            facing,
            push: Vec3::ZERO,
            health: max_health(kind),
            state: FoeState::Idle,
            flash: 0.0,
            hit_from: Vec3::ZERO,
        }
    }

    /// Standing and hittable (not in pieces).
    pub fn alive(&self) -> bool {
        !matches!(self.state, FoeState::Broken { .. })
    }

    pub fn height(&self) -> f32 {
        match self.kind {
            FoeKind::Dummy => DUMMY_HEIGHT,
            FoeKind::Husk => HUSK_HEIGHT,
        }
    }
}

fn max_health(kind: FoeKind) -> f32 {
    match kind {
        FoeKind::Dummy => DUMMY_HEALTH,
        FoeKind::Husk => HUSK_HEALTH,
    }
}

/// Dry land, not in a solid, not too steep: a foe can stand there.
fn standable(world: &World, x: f32, z: f32) -> bool {
    let feet = Vec3::new(x, world.ground_height(x, z), z);
    let limit = world.walk_limit() - 2.0;
    x.abs() < limit
        && z.abs() < limit
        && (z - river_z(x)).abs() > RIVER_HALF_WIDTH + 1.5
        && world.slope(x, z) < 0.5
        && !collide::blocked(world, feet)
        && world.solids().iter().all(|solid| collide::footprint_distance(solid, feet) > FOE_RADIUS + 0.4)
}

/// The nearest place to stand to `x, z` (looking in rings up to 6 m out).
fn place(world: &World, x: f32, z: f32) -> Option<Vec3> {
    for ring in 0..=12 {
        let radius = ring as f32 * 0.5;
        let steps = if ring == 0 { 1 } else { 8 + ring * 2 };
        for step in 0..steps {
            let angle = step as f32 * std::f32::consts::TAU / steps as f32;
            let (px, pz) = (x + angle.cos() * radius, z + angle.sin() * radius);
            if standable(world, px, pz) {
                return Some(Vec3::new(px, world.ground_height(px, pz), pz));
            }
        }
    }
    None
}

/// The valley's foes: three dummies a few steps in front of the start, husks further out.
pub fn spawn_all(world: &World) -> Vec<Foe> {
    let start = world.spawn();
    let mut foes = Vec::with_capacity(MAX_FOES);
    // The hero starts looking north (-Z): the dummies stand in a row ahead.
    for x in [-3.0, 0.0, 3.0] {
        if let Some(at) = place(world, start.x + x, start.z - 8.0) {
            foes.push(Foe::new(FoeKind::Dummy, at, 0.0));
        }
    }
    for [x, z] in [[-14.0, -30.0], [16.0, -38.0], [-30.0, -12.0], [30.0, 6.0], [4.0, -52.0]] {
        if foes.len() >= MAX_FOES {
            break;
        }
        if let Some(at) = place(world, start.x + x, start.z + z) {
            foes.push(Foe::new(FoeKind::Husk, at, 0.0));
        }
    }
    foes
}

/// Hit `foe` for `amount`, pushed by `push` (m/s). Returns whether it broke apart.
pub(crate) fn hit(foe: &mut Foe, amount: f32, push: Vec3, events: &mut Events) -> bool {
    let amount = if amount.is_finite() { amount.clamp(0.0, 1000.0) } else { 0.0 };
    if !foe.alive() || amount <= 0.0 {
        return false;
    }
    foe.health = (foe.health - amount).max(0.0);
    foe.flash = HIT_FLASH;
    let push = if push.is_finite() { Vec3::new(push.x, 0.0, push.z).clamp_length_max(10.0) } else { Vec3::ZERO };
    foe.hit_from = push.normalize_or_zero();
    if foe.kind == FoeKind::Husk {
        foe.push += push;
    }
    if foe.health <= 0.0 {
        foe.state = FoeState::Broken { time: 0.0 };
        foe.push = Vec3::ZERO;
        events.push(GameEvent::EnemyKilled);
        return true;
    }
    foe.state = FoeState::Stagger { time: 0.0 };
    false
}

/// Turn `facing` toward `direction` at `rate` (1/s).
fn turn(facing: f32, direction: Vec3, rate: f32, dt: f32) -> f32 {
    if direction.length_squared() < 1e-6 {
        return facing;
    }
    let wanted = direction.x.atan2(-direction.z);
    wrap_pi(facing + wrap_pi(wanted - facing) * (1.0 - (-rate * dt).exp()))
}

fn forward(facing: f32) -> Vec3 {
    Vec3::new(facing.sin(), 0.0, -facing.cos())
}

/// One fixed step of every foe: thinking, walking, swinging at the hero, and coming back.
pub(crate) fn step(foes: &mut [Foe], hero: &mut Hero, world: &World, events: &mut Events, dt: f32) {
    let hero_alive = !damage::dead(hero);
    for index in 0..foes.len() {
        let Some(foe) = foes.get_mut(index) else { continue };
        foe.previous_position = foe.position;
        foe.flash = (foe.flash - dt).max(0.0);
        if let FoeState::Broken { time } = foe.state {
            let time = time + dt;
            let back = if foe.kind == FoeKind::Husk { HUSK_RESPAWN } else { DUMMY_RESPAWN };
            *foe = if time >= back { Foe::new(foe.kind, foe.home, foe.facing) } else { Foe { state: FoeState::Broken { time }, ..*foe } };
            continue;
        }
        if foe.kind == FoeKind::Dummy {
            foe.state = match foe.state {
                FoeState::Stagger { time } if time + dt < STAGGER * 2.0 => FoeState::Stagger { time: time + dt },
                _ => FoeState::Idle,
            };
            continue;
        }
        husk(foe, hero, hero_alive, world, events, dt);
    }
    separate(foes, hero, world);
}

fn husk(foe: &mut Foe, hero: &mut Hero, hero_alive: bool, world: &World, events: &mut Events, dt: f32) {
    let to_hero = Vec3::new(hero.position.x - foe.position.x, 0.0, hero.position.z - foe.position.z);
    let distance = to_hero.length();
    let from_home = Vec2::new(foe.position.x - foe.home.x, foe.position.z - foe.home.z).length();
    let chasing = hero_alive && distance < SIGHT && from_home < LEASH;
    let mut walk = Vec3::ZERO;
    foe.state = match foe.state {
        FoeState::Idle | FoeState::Walk => {
            if chasing && distance < ATTACK_RANGE {
                FoeState::WindUp { time: 0.0 }
            } else if chasing {
                walk = to_hero / distance.max(1e-3);
                FoeState::Walk
            } else if from_home > 0.6 {
                walk = Vec3::new(foe.home.x - foe.position.x, 0.0, foe.home.z - foe.position.z).normalize_or_zero();
                FoeState::Walk
            } else {
                FoeState::Idle
            }
        }
        FoeState::WindUp { time } => {
            foe.facing = turn(foe.facing, to_hero, 3.0, dt);
            if time + dt >= WIND_UP {
                // The blow lands now, on a hero still in reach in front of it.
                let ahead = forward(foe.facing).dot(to_hero / distance.max(1e-3));
                if hero_alive && distance < REACH && (ahead > 0.3 || distance < 0.8) {
                    let push = to_hero / distance.max(1e-3) * HUSK_PUSH;
                    damage::hurt(hero, HUSK_DAMAGE, push, FoeKind::Husk.name(), events);
                }
                FoeState::Swing { time: 0.0 }
            } else {
                FoeState::WindUp { time: time + dt }
            }
        }
        FoeState::Swing { time } => {
            if time + dt >= SWING {
                FoeState::Recover { time: 0.0 }
            } else {
                FoeState::Swing { time: time + dt }
            }
        }
        FoeState::Recover { time } => {
            if time + dt >= RECOVER {
                FoeState::Idle
            } else {
                FoeState::Recover { time: time + dt }
            }
        }
        FoeState::Stagger { time } => {
            if time + dt >= STAGGER {
                FoeState::Idle
            } else {
                FoeState::Stagger { time: time + dt }
            }
        }
        FoeState::Broken { time } => FoeState::Broken { time },
    };
    if walk.length_squared() > 0.0 {
        foe.facing = turn(foe.facing, walk, 4.0, dt);
    }
    let velocity = walk * HUSK_SPEED + foe.push;
    foe.push *= (-8.0 * dt).exp();
    let mut position = world.keep_on_map(foe.position + velocity * dt);
    let mut ignored = velocity;
    collide::push_out(world, &mut position, &mut ignored, foe.position.y);
    position.y = collide::ground(world, position, foe.position.y + collide::STEP);
    // Husks keep out of the river (they cannot swim).
    if (position.z - river_z(position.x)).abs() < RIVER_HALF_WIDTH + 0.5 {
        position = foe.position;
    }
    foe.position = position;
}

/// Foes do not stand inside each other, and the hero does not walk through them: it is pushed
/// out (on foot or rolling; climbing or lying dead it is left alone).
fn separate(foes: &mut [Foe], hero: &mut Hero, world: &World) {
    let flat = |v: Vec3| Vec2::new(v.x, v.z);
    if matches!(hero.moves.action, Action::Free | Action::Roll { .. } | Action::Down { .. }) {
        let mut shift = Vec2::ZERO;
        for foe in foes.iter().filter(|foe| foe.alive()) {
            let between = flat(hero.position) - flat(foe.position);
            let length = between.length();
            let gap = FOE_RADIUS + super::hero::RADIUS;
            let level = hero.position.y < foe.position.y + foe.height() && foe.position.y < hero.position.y + super::hero::HEIGHT;
            if length < gap && level {
                shift += if length > 1e-4 { between / length } else { Vec2::Y } * (gap - length);
            }
        }
        let moved = world.keep_on_map(Vec3::new(hero.position.x + shift.x, hero.position.y, hero.position.z + shift.y));
        if shift.length_squared() > 0.0 && moved.is_finite() && !collide::blocked(world, moved) {
            hero.position = moved;
        }
    }
    for index in 0..foes.len() {
        let Some(foe) = foes.get(index).copied() else { continue };
        if !foe.alive() || foe.kind == FoeKind::Dummy {
            continue;
        }
        let mut shift = Vec2::ZERO;
        let mut away = |other: Vec3, gap: f32| {
            let between = flat(foe.position) - flat(other);
            let length = between.length();
            if length < gap {
                shift += if length > 1e-4 { between / length } else { Vec2::X } * (gap - length);
            }
        };
        for (other_index, other) in foes.iter().enumerate() {
            if other_index != index && other.alive() {
                away(other.position, FOE_RADIUS * 2.0);
            }
        }
        if shift.length_squared() > 0.0
            && let Some(foe) = foes.get_mut(index)
        {
            let moved = Vec3::new(foe.position.x + shift.x, foe.position.y, foe.position.z + shift.y);
            if !collide::blocked(world, moved) {
                foe.position = moved;
            }
        }
    }
}

/// How a foe looks this display frame (between two steps).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FoePose {
    pub kind: FoeKind,
    pub position: Vec3,
    pub facing: f32,
    /// Arms raised (0 down, 1 high): up through the wind-up, down fast in the swing.
    pub raise: f32,
    /// The white flash of a hit (0..1).
    pub flash: f32,
    /// How far it leans away from the last hit (0..1).
    pub lean: f32,
    /// Which way it leans.
    pub lean_toward: Vec3,
    /// Seconds since it broke apart (drawn as flying pieces), if it did.
    pub broken: Option<f32>,
}

impl Default for FoePose {
    fn default() -> Self {
        Self { kind: FoeKind::Dummy, position: Vec3::ZERO, facing: 0.0, raise: 0.0, flash: 0.0, lean: 0.0, lean_toward: Vec3::Z, broken: None }
    }
}

pub(crate) fn pose(foe: &Foe, alpha: f32) -> FoePose {
    let alpha = if alpha.is_finite() { alpha.clamp(0.0, 1.0) } else { 1.0 };
    let (raise, lean, broken) = match foe.state {
        FoeState::WindUp { time } => ((time / WIND_UP).clamp(0.0, 1.0), 0.0, None),
        FoeState::Swing { time } => (1.0 - (time / SWING).clamp(0.0, 1.0), 0.0, None),
        FoeState::Stagger { time } => {
            let span = if foe.kind == FoeKind::Dummy { STAGGER * 2.0 } else { STAGGER };
            let t = (time / span).clamp(0.0, 1.0);
            // A dummy rocks back and forth and settles; a husk reels once.
            let rock = if foe.kind == FoeKind::Dummy { (t * 9.0).sin() * (1.0 - t) } else { (t * std::f32::consts::PI).sin() };
            (0.0, rock, None)
        }
        FoeState::Broken { time } => (0.0, 0.0, Some(time)),
        _ => (0.0, 0.0, None),
    };
    FoePose {
        kind: foe.kind,
        position: foe.previous_position.lerp(foe.position, alpha),
        facing: foe.facing,
        raise,
        flash: (foe.flash / HIT_FLASH).clamp(0.0, 1.0),
        lean,
        lean_toward: foe.hit_from,
        broken,
    }
}
