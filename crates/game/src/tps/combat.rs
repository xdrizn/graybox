//! What the weapons do (first part of G11.5): 1–4 pick the sword, spear, club or bow (drawn from
//! the back; the same key again puts it away; also in the air, and a key pressed mid-strike counts
//! when the strike ends); an attack press strikes with the weapon in hand (it
//! is drawn at once if it was on the back): the sword chains three strikes when pressed again in
//! time, the spear thrusts, the club swings; the bow draws while the attack is held and shoots on
//! release. A strike slows the hero, pushes it a little forward and keeps its facing.
//! Strikes hit the foes in reach in front of the hero once each, in the middle of the swing (each
//! weapon its reach, width, damage and push); a bow drawn long enough shoots an arrow where the
//! camera looks.
use super::animator::Weapon;
use super::arrows::{self, Arrow};
use super::clip::ClipId;
use super::clips::clip;
use super::controller::{Action, Presses};
use super::damage::Events;
use super::foes::{self, Foe};
use super::hero::Hero;
use glam::Vec3;

/// A strike being played.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Strike {
    clip: ClipId,
    time: f32,
    /// The next sword strike was pressed for in time.
    queued: bool,
    /// The foes this strike already hit (bit per foe index).
    hits: u32,
}

/// The weapon state kept between steps.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Combat {
    strike: Option<Strike>,
    aiming: bool,
    /// Seconds the bow has been drawn.
    drawn: f32,
    /// A weapon key pressed during a strike, picked when it ends (0 is none).
    queued_weapon: u8,
}

impl Combat {
    /// The strike being played, if any.
    pub fn striking(&self) -> Option<ClipId> {
        self.strike.map(|strike| strike.clip)
    }

    /// The bow is drawn.
    pub fn aiming(&self) -> bool {
        self.aiming
    }
}

/// How weapons change the movement this step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Pace {
    /// A cap on the speed (m/s).
    pub limit: Option<f32>,
    /// A push forward (m/s) during the strike's swing.
    pub lunge: f32,
    /// How fast the facing follows the stick (1 is normal).
    pub turn: f32,
    /// Face where the camera looks (aiming the bow).
    pub face_camera: bool,
    /// A strike is playing: no jump, roll or climb until it ends.
    pub busy: bool,
}

const FREE: Pace = Pace { limit: None, lunge: 0.0, turn: 1.0, face_camera: false, busy: false };
/// Walking pace while the bow is drawn.
const AIM_SPEED: f32 = 1.8;
/// The sword's next strike counts if pressed after this part of the current one.
const COMBO_FROM: f32 = 0.35;
/// The bow shoots only once drawn this long (a tap does nothing).
const BOW_READY: f32 = 0.35;

/// What a strike does: the part of the clip it hits in, how far and how wide (cosine of the half
/// angle in front), the damage and the push (m/s).
struct Blow {
    from: f32,
    to: f32,
    reach: f32,
    width: f32,
    damage: f32,
    push: f32,
}

fn blow(clip: ClipId) -> Blow {
    match clip {
        ClipId::Sword3 => Blow { from: 0.3, to: 0.6, reach: 2.2, width: -0.1, damage: 28.0, push: 4.0 },
        ClipId::SpearThrust => Blow { from: 0.3, to: 0.55, reach: 2.9, width: 0.75, damage: 26.0, push: 3.0 },
        ClipId::ClubSwing => Blow { from: 0.35, to: 0.6, reach: 2.2, width: 0.2, damage: 36.0, push: 7.0 },
        _ => Blow { from: 0.25, to: 0.55, reach: 2.0, width: 0.3, damage: 18.0, push: 2.0 },
    }
}

/// What the weapons reach this step: the foes, the arrows in the air, where the camera looks.
pub(crate) struct Field<'a> {
    pub foes: &'a mut [Foe],
    pub arrows: &'a mut Vec<Arrow>,
    pub events: &'a mut Events,
    /// Where the camera looks (the bow shoots that way).
    pub aim: Vec3,
}

pub(crate) fn pace(combat: &Combat) -> Pace {
    if let Some(strike) = combat.strike {
        let t = strike.time / clip(strike.clip).duration.max(0.05);
        let push = match strike.clip {
            ClipId::SpearThrust => 3.5,
            ClipId::ClubSwing => 1.5,
            _ => 2.5,
        };
        let lunge = if (0.15..0.45).contains(&t) { push } else { 0.0 };
        return Pace { limit: Some(0.6), lunge, turn: 0.15, face_camera: false, busy: true };
    }
    if combat.aiming {
        return Pace { limit: Some(AIM_SPEED), face_camera: true, ..FREE };
    }
    FREE
}

fn first_strike(weapon: Weapon) -> Option<ClipId> {
    match weapon {
        Weapon::Sword => Some(ClipId::Sword1),
        Weapon::Spear => Some(ClipId::SpearThrust),
        Weapon::Club => Some(ClipId::ClubSwing),
        Weapon::Bow | Weapon::None => None,
    }
}

fn next_strike(clip: ClipId) -> Option<ClipId> {
    match clip {
        ClipId::Sword1 => Some(ClipId::Sword2),
        ClipId::Sword2 => Some(ClipId::Sword3),
        _ => None,
    }
}

fn start(hero: &mut Hero, clip: ClipId) {
    // A draw or sheathe still showing would cover the strike (and move the weapon mid-swing).
    hero.animator.release();
    hero.animator.play(clip);
    hero.combat.strike = Some(Strike { clip, time: 0.0, queued: false, hits: 0 });
}

/// The strike's blow lands on every foe in reach in front of the hero it has not hit yet.
fn land(hero: &Hero, strike: &mut Strike, field: &mut Field<'_>) {
    let blow = blow(strike.clip);
    let t = strike.time / clip(strike.clip).duration.max(0.05);
    if !(blow.from..=blow.to).contains(&t) {
        return;
    }
    let forward = Vec3::new(hero.facing.sin(), 0.0, -hero.facing.cos());
    for (index, foe) in field.foes.iter_mut().enumerate().take(32) {
        let bit = 1_u32 << index;
        if strike.hits & bit != 0 || !foe.alive() {
            continue;
        }
        let to = Vec3::new(foe.position.x - hero.position.x, 0.0, foe.position.z - hero.position.z);
        let distance = to.length();
        let level = (foe.position.y - hero.position.y).abs() < 1.5;
        let ahead = if distance > 1e-3 { forward.dot(to / distance) } else { 1.0 };
        if level && distance < blow.reach + foes::FOE_RADIUS && (ahead >= blow.width || distance < 0.6) {
            strike.hits |= bit;
            let push = if distance > 1e-3 { to / distance } else { forward } * blow.push;
            foes::hit(foe, blow.damage, push, field.events);
        }
    }
}

/// One fixed step, before the movement. `held` is the attack button held now.
pub(super) fn step(hero: &mut Hero, presses: Presses, held: bool, field: &mut Field<'_>, dt: f32) {
    let free = hero.moves.action == Action::Free;
    if !free || !hero.grounded {
        // Climbing, rolling, falling or knocked down: strikes and the bow stop.
        if hero.combat.strike.take().is_some() && free {
            // Off an edge mid-swing: back to moving (the fall pose), not the rest of the swing.
            hero.animator.stop();
        }
        if hero.combat.aiming {
            hero.combat.aiming = false;
            hero.combat.drawn = 0.0;
            hero.animator.release();
        }
    }
    if !free {
        // The hands are busy (climbing, rolling, getting up): weapon keys do nothing.
        hero.combat.queued_weapon = 0;
        return;
    }
    if presses.weapon != 0 {
        hero.combat.queued_weapon = presses.weapon;
    }

    // A strike runs to its end, or into the next one.
    if let Some(mut strike) = hero.combat.strike {
        strike.time += dt;
        let length = clip(strike.clip);
        if presses.attack && strike.time >= length.duration * COMBO_FROM {
            strike.queued = true;
        }
        land(hero, &mut strike, field);
        hero.combat.strike = Some(strike);
        if strike.time >= length.duration - length.fade_out {
            hero.combat.strike = None;
            if let Some(next) = next_strike(strike.clip).filter(|_| strike.queued) {
                start(hero, next);
            }
        }
        return;
    }

    // A weapon key (now, or pressed during the strike that just ended): also in the air.
    let weapon = hero.animator.weapon();
    let pick = std::mem::take(&mut hero.combat.queued_weapon);
    if let Some(picked) = [Weapon::Sword, Weapon::Spear, Weapon::Club, Weapon::Bow].get(usize::from(pick).wrapping_sub(1)).copied() {
        if hero.combat.aiming {
            hero.combat.aiming = false;
            hero.combat.drawn = 0.0;
            hero.animator.release();
        }
        if picked == weapon && hero.animator.in_hand() {
            hero.animator.play(ClipId::WeaponSheathe);
        } else {
            hero.animator.set_weapon(picked, false);
            hero.animator.play(ClipId::WeaponDraw);
        }
        return;
    }
    if !hero.grounded {
        return;
    }

    if weapon == Weapon::Bow {
        if held && !hero.combat.aiming {
            hero.animator.set_weapon(Weapon::Bow, true);
            hero.animator.play(ClipId::BowDraw);
            hero.combat.aiming = true;
            hero.combat.drawn = 0.0;
        } else if held && hero.combat.aiming {
            hero.combat.drawn += dt;
        } else if !held && hero.combat.aiming {
            hero.combat.aiming = false;
            if hero.combat.drawn >= BOW_READY {
                hero.animator.play(ClipId::BowShoot);
                let forward = Vec3::new(hero.facing.sin(), 0.0, -hero.facing.cos());
                let from = hero.position + Vec3::Y * 1.45 + forward * 0.4;
                arrows::shoot(field.arrows, from, field.aim, field.events);
            } else {
                hero.animator.release();
            }
            hero.combat.drawn = 0.0;
        }
        return;
    }
    if presses.attack
        && let Some(first) = first_strike(weapon)
    {
        hero.animator.set_weapon(weapon, true);
        start(hero, first);
    }
}
