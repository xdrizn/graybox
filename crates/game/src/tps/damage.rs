//! Health, damage and the game events (G11.5): what hurts the hero (falls, husks), what it does
//! (knocked back, a flinch, death), and the events the designed UI listens to (the same names as
//! the first-person game: `vida-mudou`, `tomou-dano`, `vida-zero`, `renasceu`, `atirou`,
//! `inimigo-morreu`).
use super::clip::ClipId;
use super::controller::Action;
use super::hero::Hero;
use crate::state::GameEvent;
use glam::Vec3;

pub const HEALTH_MAX: f32 = 100.0;
/// A fall longer than this (metres) without a roll hurts…
pub const FALL_SAFE: f32 = 4.0;
/// …this much for every metre more (a 12 m fall takes about half the health).
const FALL_DAMAGE: f32 = 6.0;
/// The most events kept between two frames of the UI.
const EVENT_CAPACITY: usize = 64;

/// The hero's health.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vitals {
    pub health: f32,
    pub max: f32,
    /// The last hit taken (for the UI).
    pub last_damage: f32,
    /// What dealt it ("Husk", "Fall").
    pub attacker: &'static str,
}

impl Default for Vitals {
    fn default() -> Self {
        Self { health: HEALTH_MAX, max: HEALTH_MAX, last_damage: 0.0, attacker: "" }
    }
}

/// Game events since the UI last took them (oldest first; the oldest are dropped past the cap).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Events(Vec<GameEvent>);

impl Events {
    pub fn push(&mut self, event: GameEvent) {
        if self.0.len() >= EVENT_CAPACITY {
            self.0.remove(0);
        }
        self.0.push(event);
    }

    pub fn drain(&mut self) -> Vec<GameEvent> {
        std::mem::take(&mut self.0)
    }
}

/// Whether the hero is down for good (until it respawns).
pub fn dead(hero: &Hero) -> bool {
    matches!(hero.moves.action, Action::Dead { .. })
}

/// Hurt the hero by `amount`, pushed by `push` (m/s, sideways; a little lift is added). A roll
/// dodges it, a dead hero takes nothing. Returns the damage dealt.
pub(crate) fn hurt(hero: &mut Hero, amount: f32, push: Vec3, attacker: &'static str, events: &mut Events) -> f32 {
    let amount = if amount.is_finite() { amount.clamp(0.0, 1000.0) } else { 0.0 };
    if amount <= 0.0 || dead(hero) || matches!(hero.moves.action, Action::Roll { .. }) {
        return 0.0;
    }
    let vitals = &mut hero.vitals;
    let dealt = amount.min(vitals.health);
    vitals.health = (vitals.health - amount).max(0.0);
    vitals.last_damage = amount;
    vitals.attacker = attacker;
    events.push(GameEvent::HealthChanged);
    events.push(GameEvent::TookDamage);
    if vitals.health <= 0.0 {
        die(hero, events);
        return dealt;
    }
    let push = if push.is_finite() { Vec3::new(push.x, 0.0, push.z).clamp_length_max(12.0) } else { Vec3::ZERO };
    if push.length_squared() > 0.01 && matches!(hero.moves.action, Action::Free) {
        hero.velocity = push + Vec3::Y * 2.5;
        hero.grounded = false;
    }
    // The flinch plays over the body, unless the hands are busy with the bow or a weapon draw
    // (it would cut them off: the weapon would stay on the back, the bow drawn with no pose).
    if hero.animator.overlay_clip().is_none_or(|clip| clip == ClipId::HitReact || clip == ClipId::Land) && !hero.combat.aiming() {
        hero.animator.play(ClipId::HitReact);
    }
    dealt
}

/// A fall of `drop` metres that was not rolled.
pub(crate) fn fall(hero: &mut Hero, drop: f32, events: &mut Events) {
    if drop.is_finite() && drop > FALL_SAFE {
        hurt(hero, (drop - FALL_SAFE) * FALL_DAMAGE, Vec3::ZERO, "Fall", events);
    }
}

fn die(hero: &mut Hero, events: &mut Events) {
    hero.moves.action = Action::Dead { time: 0.0 };
    hero.velocity = Vec3::new(0.0, hero.velocity.y.min(0.0), 0.0);
    hero.animator.play(ClipId::Die);
    events.push(GameEvent::Died);
}

/// Back at `spawn` with full health.
pub(crate) fn respawn(hero: &mut Hero, spawn: Vec3, events: &mut Events) {
    let vitals = Vitals { max: hero.vitals.max, health: hero.vitals.max, ..Vitals::default() };
    let weapon = (hero.animator.weapon(), hero.animator.in_hand());
    *hero = Hero::at(spawn);
    hero.vitals = vitals;
    hero.animator.set_weapon(weapon.0, weapon.1);
    events.push(GameEvent::Respawned);
    events.push(GameEvent::HealthChanged);
}
