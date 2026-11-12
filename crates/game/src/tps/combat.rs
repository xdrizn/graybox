//! What the weapons do (first part of G11.5): 1–4 pick the sword, spear, club or bow (drawn from
//! the back; the same key again puts it away; also in the air, and a key pressed mid-strike counts
//! when the strike ends); an attack press strikes with the weapon in hand (it
//! is drawn at once if it was on the back): the sword chains three strikes when pressed again in
//! time, the spear thrusts, the club swings; the bow draws while the attack is held and shoots on
//! release. A strike slows the hero, pushes it a little forward and keeps its facing.
//! Nothing is hit yet: targets, damage and arrows come with the rest of G11.5.
use super::animator::Weapon;
use super::clip::ClipId;
use super::clips::clip;
use super::controller::{Action, Presses};
use super::hero::Hero;

/// A strike being played.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Strike {
    clip: ClipId,
    time: f32,
    /// The next sword strike was pressed for in time.
    queued: bool,
}

/// The weapon state kept between steps.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Combat {
    strike: Option<Strike>,
    aiming: bool,
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
    hero.combat.strike = Some(Strike { clip, time: 0.0, queued: false });
}

/// One fixed step, before the movement. `held` is the attack button held now.
pub(super) fn step(hero: &mut Hero, presses: Presses, held: bool, dt: f32) {
    let free = hero.moves.action == Action::Free;
    if !free || !hero.grounded {
        // Climbing, rolling, falling or knocked down: strikes and the bow stop.
        if hero.combat.strike.take().is_some() && free {
            // Off an edge mid-swing: back to moving (the fall pose), not the rest of the swing.
            hero.animator.stop();
        }
        if hero.combat.aiming {
            hero.combat.aiming = false;
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
        } else if !held && hero.combat.aiming {
            hero.animator.play(ClipId::BowShoot);
            hero.combat.aiming = false;
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
