//! Deterministic, GPU-independent Greybox FPS simulation. The exported arena is also the
//! renderer's geometry source; all gameplay advances through the bounded fixed-step clock.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod arena;
mod clock;
mod controller;
pub mod menu;
mod state;
pub mod tps;
pub use clock::{FixedClock, MAX_STEPS, STEP_SECS};
use glam::Vec3;
pub use state::{Debris, GameEvent, Input, MAGAZINE_SIZE, MAX_DEBRIS, MAX_ENEMIES, MAX_PROJECTILES, Player, Projectile, START_RESERVE, Sentinel};

#[cfg(test)]
mod menu_tests;
#[cfg(test)]
mod tests;

const EVENT_CAPACITY: usize = 256;
const DT: f32 = STEP_SECS as f32;

#[derive(Debug, PartialEq)]
pub struct Game {
    clock: FixedClock,
    ticks: u64,
    pub input: Input,
    pub player: Player,
    pub enemies: Vec<Sentinel>,
    pub projectiles: Vec<Projectile>,
    pub debris: Vec<Debris>,
    pub paused: bool,
    objects: Vec<arena::SceneObject>,
    events: Vec<GameEvent>,
    pending: Input,
    fire_damage_remaining: f32,
    seed: u32,
}

impl Clone for Game {
    fn clone(&self) -> Self {
        // A cloned simulation keeps the same preallocation guarantees as a fresh one.
        let mut player = self.player.clone();
        player.attacker.reserve(96_usize.saturating_sub(player.attacker.len()));
        Self {
            clock: self.clock,
            ticks: self.ticks,
            input: self.input,
            player,
            enemies: clone_reserved(&self.enemies, MAX_ENEMIES),
            projectiles: clone_reserved(&self.projectiles, MAX_PROJECTILES),
            debris: clone_reserved(&self.debris, MAX_DEBRIS),
            paused: self.paused,
            objects: self.objects.clone(),
            events: clone_reserved(&self.events, EVENT_CAPACITY),
            pending: self.pending,
            fire_damage_remaining: self.fire_damage_remaining,
            seed: self.seed,
        }
    }
}

fn clone_reserved<T: Clone>(source: &[T], capacity: usize) -> Vec<T> {
    let mut output = Vec::with_capacity(capacity.max(source.len()));
    output.extend_from_slice(source);
    output
}

impl Default for Game {
    fn default() -> Self {
        let mut enemies = Vec::with_capacity(MAX_ENEMIES);
        for position in [Vec3::new(-4.0, 0.0, -9.0), Vec3::new(4.0, 0.0, -20.0), Vec3::new(0.0, 0.0, -33.0)] {
            enemies.push(Sentinel::new(position));
        }
        Self {
            clock: FixedClock::default(),
            ticks: 0,
            input: Input::default(),
            player: Player::default(),
            enemies,
            projectiles: Vec::with_capacity(MAX_PROJECTILES),
            debris: Vec::with_capacity(MAX_DEBRIS),
            paused: false,
            objects: arena::objects(),
            events: Vec::with_capacity(EVENT_CAPACITY),
            pending: Input::default(),
            fire_damage_remaining: 1.0,
            seed: 17,
        }
    }
}

impl Game {
    pub fn new() -> Self {
        Self::default()
    }

    /// Look deltas are applied once, edge actions survive sub-step display frames, and held
    /// shooting/movement is sampled for every step. Invalid frame times change nothing.
    pub fn advance(&mut self, dt: f64) -> u32 {
        if !dt.is_finite() || dt <= 0.0 {
            return 0;
        }
        if self.paused {
            self.input.look = [0.0; 2];
            self.input.jump = false;
            self.input.reload = false;
            self.input.heal = false;
            self.input.respawn = false;
            self.pending = Input::default();
            return 0;
        }
        if !self.player.yaw.is_finite() {
            self.player.yaw = 0.0;
        }
        if !self.player.pitch.is_finite() {
            self.player.pitch = 0.0;
        }
        if !self.player.dead {
            if self.input.look[0].is_finite() {
                self.player.yaw = (self.player.yaw + self.input.look[0].rem_euclid(std::f32::consts::TAU)).rem_euclid(std::f32::consts::TAU);
            }
            if self.input.look[1].is_finite() {
                self.player.pitch = (self.player.pitch + self.input.look[1].clamp(-std::f32::consts::PI, std::f32::consts::PI)).clamp(-1.54, 1.54);
            }
        }
        self.input.look = [0.0; 2];
        self.pending.jump |= std::mem::take(&mut self.input.jump);
        self.pending.shoot |= self.input.shoot;
        self.pending.reload |= std::mem::take(&mut self.input.reload);
        self.pending.heal |= std::mem::take(&mut self.input.heal);
        self.pending.respawn |= std::mem::take(&mut self.input.respawn);
        let steps = self.clock.advance(dt);
        for _ in 0..steps {
            self.step();
        }
        steps
    }

    fn step(&mut self) {
        self.ticks = self.ticks.saturating_add(1);
        self.enemies.truncate(MAX_ENEMIES);
        self.projectiles.truncate(MAX_PROJECTILES);
        self.debris.truncate(MAX_DEBRIS);
        let edges = std::mem::take(&mut self.pending);
        if edges.respawn && self.player.dead {
            self.respawn();
        }
        if edges.heal {
            self.heal(25.0);
        }
        if !self.player.dead {
            let movement = Input { jump: edges.jump, ..self.input };
            controller::advance(&mut self.player, movement, &self.objects, DT);
            if self.player.position.y < -12.0 {
                self.damage(self.player.max_health, "queda");
            }
            self.advance_weapon(edges.reload, edges.shoot);
            if self.player.on_fire {
                self.fire_damage_remaining -= DT;
                if self.fire_damage_remaining <= 0.0 {
                    self.fire_damage_remaining += 1.0;
                    self.damage(8.0, "fogo");
                }
            }
        }
        self.advance_enemies();
        self.advance_projectiles();
        self.advance_debris();
    }

    fn advance_weapon(&mut self, reload: bool, shot: bool) {
        if self.player.dead {
            return;
        }
        self.player.shot_age = (self.player.shot_age + DT).min(100.0);
        if self.player.reload_remaining > 0.0 {
            self.player.reload_remaining = (self.player.reload_remaining - DT).max(0.0);
            if self.player.reload_remaining <= 0.00001 {
                self.player.reload_remaining = 0.0;
                let transfer = MAGAZINE_SIZE.saturating_sub(self.player.ammo).min(self.player.reserve);
                self.player.ammo += transfer;
                self.player.reserve -= transfer;
                self.emit(GameEvent::Reloaded);
            }
        }
        if reload && self.player.reload_remaining == 0.0 && self.player.ammo < MAGAZINE_SIZE && self.player.reserve > 0 {
            self.player.reload_remaining = 0.8;
        }
        if (self.input.shoot || shot) && self.player.reload_remaining == 0.0 && self.player.shot_age >= 0.22 {
            self.shoot();
        }
    }

    fn emit(&mut self, event: GameEvent) {
        // Consumers normally drain every display frame. An unattended game must stay bounded.
        if self.events.len() < EVENT_CAPACITY {
            self.events.push(event);
        }
    }

    /// Draining is an explicit integration boundary, not part of the fixed-step hot path.
    pub fn drain_events(&mut self) -> Vec<GameEvent> {
        let mut drained = Vec::with_capacity(self.events.len());
        drained.append(&mut self.events);
        drained
    }

    pub fn damage(&mut self, amount: f32, attacker: &str) {
        if self.player.dead || !amount.is_finite() || amount <= 0.0 {
            return;
        }
        let amount = amount.min(self.player.health.max(0.0));
        if amount <= 0.0 {
            return;
        }
        self.player.health = (self.player.health - amount).max(0.0);
        self.player.last_damage = amount;
        self.player.attacker.clear();
        for character in attacker.chars() {
            if self.player.attacker.len() + character.len_utf8() > 96 {
                break;
            }
            self.player.attacker.push(character);
        }
        self.emit(GameEvent::HealthChanged);
        self.emit(GameEvent::TookDamage);
        if self.player.health == 0.0 {
            self.player.dead = true;
            self.player.reload_remaining = 0.0;
            self.player.vertical_speed = 0.0;
            self.set_on_fire(false);
            self.emit(GameEvent::Died);
        }
    }

    pub fn heal(&mut self, amount: f32) {
        if self.player.dead || !amount.is_finite() || amount <= 0.0 {
            return;
        }
        let new = (self.player.health + amount.min(self.player.max_health)).min(self.player.max_health);
        if new > self.player.health {
            self.player.health = new;
            self.emit(GameEvent::HealthChanged);
            self.emit(GameEvent::Healed);
        }
    }

    pub fn set_on_fire(&mut self, enabled: bool) {
        let enabled = enabled && !self.player.dead;
        if self.player.on_fire != enabled {
            self.player.on_fire = enabled;
            self.fire_damage_remaining = 1.0;
            self.emit(if enabled { GameEvent::StatusStarted } else { GameEvent::StatusEnded });
        }
    }

    pub fn respawn(&mut self) {
        self.set_on_fire(false);
        let p = &mut self.player;
        p.position = state::SPAWN;
        p.yaw = 0.0;
        p.pitch = 0.0;
        p.health = p.max_health;
        p.ammo = MAGAZINE_SIZE;
        p.reserve = START_RESERVE;
        p.crouching = false;
        p.dead = false;
        p.last_damage = 0.0;
        p.attacker.clear();
        p.shot_age = 100.0;
        p.reload_remaining = 0.0;
        p.bob = 0.0;
        p.vertical_speed = 0.0;
        p.grounded = false;
        self.projectiles.clear();
        self.emit(GameEvent::HealthChanged);
        self.emit(GameEvent::Respawned);
    }

    fn shoot(&mut self) {
        self.player.shot_age = 0.0;
        if self.player.ammo == 0 {
            self.emit(GameEvent::AmmoEmpty);
            return;
        }
        self.player.ammo -= 1;
        self.emit(GameEvent::Shot);
        if self.player.ammo == 0 {
            self.emit(GameEvent::AmmoEmpty);
        }
        let origin = self.player.eye();
        let direction = self.player.direction();
        let wall = arena::raycast(&self.objects, origin, direction, 120.0).unwrap_or(120.0);
        let mut target = None;
        let mut closest = wall;
        for (index, enemy) in self.enemies.iter().enumerate() {
            if enemy.health == 0 {
                continue;
            }
            let min = enemy.position + Vec3::new(-0.575, 0.0, -0.35);
            let max = enemy.position + Vec3::new(0.575, 2.65, 0.35);
            if let Some(distance) = arena::ray_box(origin, direction, min, max, closest).filter(|distance| *distance < closest) {
                closest = distance;
                target = Some(index);
            }
        }
        let mut death_position = None;
        if let Some(enemy) = target.and_then(|index| self.enemies.get_mut(index)) {
            enemy.health = enemy.health.saturating_sub(1);
            enemy.hit_remaining = 0.15;
            if enemy.health == 0 {
                enemy.respawn_remaining = 5.0;
                death_position = Some(enemy.position);
            }
        }
        if let Some(position) = death_position {
            self.emit(GameEvent::EnemyKilled);
            self.spawn_debris(position);
        }
    }

    fn spawn_debris(&mut self, position: Vec3) {
        for _ in 0..9 {
            if self.debris.len() >= MAX_DEBRIS {
                break;
            }
            let x = arena::random(&mut self.seed) * 2.0 - 1.0;
            let z = arena::random(&mut self.seed) * 2.0 - 1.0;
            let y = arena::random(&mut self.seed);
            self.debris.push(Debris {
                position: position + Vec3::new(x * 0.5, 0.5 + y * 1.7, z * 0.4),
                rotation: Vec3::new(x, y, z) * 3.0,
                velocity: Vec3::new(x * 4.0, 3.0 + y * 4.0, z * 4.0),
                angular_velocity: Vec3::new(z, x, y) * 5.0,
                remaining: 5.0,
            });
        }
    }

    fn advance_enemies(&mut self) {
        for enemy in &mut self.enemies {
            enemy.hit_remaining = (enemy.hit_remaining - DT).max(0.0);
            if enemy.health == 0 {
                enemy.respawn_remaining = (enemy.respawn_remaining - DT).max(0.0);
                if enemy.respawn_remaining <= 0.00001 {
                    enemy.health = 3;
                    enemy.position = enemy.spawn;
                    enemy.hit_remaining = 0.0;
                    enemy.fire_remaining = 2.5;
                }
                continue;
            }
            let origin = enemy.position + Vec3::Y * 1.9;
            let towards = self.player.eye() - origin;
            enemy.yaw = towards.x.atan2(-towards.z);
            enemy.fire_remaining = (enemy.fire_remaining - DT).max(0.0);
            let distance = towards.length();
            if self.player.dead || enemy.fire_remaining > 0.0 || !(0.01..=24.0).contains(&distance) {
                continue;
            }
            let direction = towards / distance;
            if arena::raycast(&self.objects, origin, direction, distance).is_some_and(|hit| hit < distance - 0.01) {
                continue;
            }
            if self.projectiles.len() < MAX_PROJECTILES {
                self.projectiles.push(Projectile { position: origin, velocity: direction * 12.0, remaining: 4.0 });
            }
            enemy.fire_remaining = 3.5 * (0.8 + arena::random(&mut self.seed) * 0.4);
        }
    }

    fn advance_projectiles(&mut self) {
        let mut hits = 0_u32;
        let player_min = self.player.position - Vec3::new(state::PLAYER_RADIUS + 0.16, 0.16, state::PLAYER_RADIUS + 0.16);
        let player_max = self.player.position + Vec3::new(state::PLAYER_RADIUS + 0.16, self.player.height() + 0.16, state::PLAYER_RADIUS + 0.16);
        self.projectiles.retain_mut(|projectile| {
            if !projectile.position.is_finite()
                || !projectile.velocity.is_finite()
                || !projectile.remaining.is_finite()
                || projectile.remaining <= 0.0
            {
                return false;
            }
            let delta = projectile.velocity * DT;
            let length = delta.length();
            if !length.is_finite() || length < 0.000001 {
                return false;
            }
            let direction = delta / length;
            let wall = arena::raycast(&self.objects, projectile.position, direction, length + 0.16);
            let player = if self.player.dead { None } else { arena::ray_box(projectile.position, direction, player_min, player_max, length) };
            if player.is_some_and(|distance| wall.is_none_or(|wall| distance < wall)) {
                hits += 1;
                return false;
            }
            if wall.is_some() {
                return false;
            }
            projectile.position += delta;
            projectile.remaining -= DT;
            projectile.remaining > 0.00001
        });
        for _ in 0..hits {
            self.damage(15.0, "Sentinela");
        }
    }

    fn advance_debris(&mut self) {
        self.debris.retain_mut(|piece| {
            if !piece.position.is_finite()
                || !piece.velocity.is_finite()
                || !piece.rotation.is_finite()
                || !piece.angular_velocity.is_finite()
                || !piece.remaining.is_finite()
            {
                return false;
            }
            piece.velocity.y -= 18.0 * DT;
            piece.position += piece.velocity * DT;
            piece.rotation += piece.angular_velocity * DT;
            if piece.position.y < 0.17 {
                piece.position.y = 0.17;
                piece.velocity.y = piece.velocity.y.abs() * 0.4;
                piece.velocity.x *= 0.8;
                piece.velocity.z *= 0.8;
            }
            piece.remaining -= DT;
            piece.remaining > 0.00001 && piece.position.is_finite() && piece.rotation.is_finite()
        });
    }

    pub fn ticks(&self) -> u64 {
        self.ticks
    }
    pub fn time(&self) -> f64 {
        self.ticks as f64 * STEP_SECS
    }
    pub fn alpha(&self) -> f64 {
        self.clock.alpha()
    }
}
