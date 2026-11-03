use glam::Vec3;

pub const MAGAZINE_SIZE: u32 = 12;
pub const MAX_ENEMIES: usize = 8;
pub const MAX_PROJECTILES: usize = 32;
pub const MAX_DEBRIS: usize = MAX_ENEMIES * 9;
pub(crate) const PLAYER_RADIUS: f32 = 0.3;
pub(crate) const STANDING_HEIGHT: f32 = 1.8;
pub(crate) const CROUCH_HEIGHT: f32 = 1.1;
pub(crate) const SPAWN: Vec3 = Vec3::new(0.0, 2.0, 12.0);

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Input {
    pub forward: f32,
    pub strafe: f32,
    /// Yaw and pitch increments in radians, consumed once per advance.
    pub look: [f32; 2],
    pub run: bool,
    pub crouch: bool,
    pub jump: bool,
    pub shoot: bool,
    pub reload: bool,
    pub heal: bool,
    pub respawn: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Player {
    /// Feet position, not eye position.
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub health: f32,
    pub max_health: f32,
    pub ammo: u32,
    pub reserve: u32,
    pub crouching: bool,
    pub on_fire: bool,
    pub dead: bool,
    pub last_damage: f32,
    pub attacker: String,
    pub shot_age: f32,
    pub reload_remaining: f32,
    pub bob: f32,
    pub(crate) vertical_speed: f32,
    pub(crate) grounded: bool,
}

impl Default for Player {
    fn default() -> Self {
        Self {
            position: SPAWN,
            yaw: 0.0,
            pitch: 0.0,
            health: 100.0,
            max_health: 100.0,
            ammo: MAGAZINE_SIZE,
            reserve: 48,
            crouching: false,
            on_fire: false,
            dead: false,
            last_damage: 0.0,
            attacker: String::with_capacity(96),
            shot_age: 100.0,
            reload_remaining: 0.0,
            bob: 0.0,
            vertical_speed: 0.0,
            grounded: false,
        }
    }
}

impl Player {
    pub fn eye(&self) -> Vec3 {
        self.position + Vec3::Y * if self.crouching { 0.95 } else { 1.65 }
    }

    /// Yaw zero faces -Z; positive yaw turns right, positive pitch looks up.
    pub fn direction(&self) -> Vec3 {
        Vec3::new(self.yaw.sin() * self.pitch.cos(), self.pitch.sin(), -self.yaw.cos() * self.pitch.cos())
    }

    pub(crate) fn height(&self) -> f32 {
        if self.crouching { CROUCH_HEIGHT } else { STANDING_HEIGHT }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Sentinel {
    pub position: Vec3,
    pub yaw: f32,
    pub health: u32,
    pub hit_remaining: f32,
    pub respawn_remaining: f32,
    pub(crate) fire_remaining: f32,
    pub(crate) spawn: Vec3,
}

impl Sentinel {
    pub fn new(position: Vec3) -> Self {
        let position = if position.is_finite() { position } else { Vec3::ZERO };
        Self { position, yaw: 0.0, health: 3, hit_remaining: 0.0, respawn_remaining: 0.0, fire_remaining: 2.5, spawn: position }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Projectile {
    pub position: Vec3,
    pub velocity: Vec3,
    pub remaining: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Debris {
    pub position: Vec3,
    pub rotation: Vec3,
    pub velocity: Vec3,
    pub angular_velocity: Vec3,
    pub remaining: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameEvent {
    HealthChanged,
    TookDamage,
    Died,
    Respawned,
    Shot,
    Reloaded,
    AmmoEmpty,
    Healed,
    StatusStarted,
    StatusEnded,
    EnemyKilled,
}

impl GameEvent {
    pub fn name(&self) -> &'static str {
        match self {
            Self::HealthChanged => "vida-mudou",
            Self::TookDamage => "tomou-dano",
            Self::Died => "vida-zero",
            Self::Respawned => "renasceu",
            Self::Shot => "atirou",
            Self::Reloaded => "recarregou",
            Self::AmmoEmpty => "municao-zero",
            Self::Healed => "curou",
            Self::StatusStarted => "status-iniciou",
            Self::StatusEnded => "status-terminou",
            Self::EnemyKilled => "inimigo-morreu",
        }
    }
}
