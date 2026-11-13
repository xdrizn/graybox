//! What the weapons do (first part of G11.5): 1–4 pick the sword, spear, club or bow (drawn from
//! the back; the same key again puts it away; also in the air, and a key pressed mid-strike counts
//! when the strike ends); an attack press strikes with the weapon in hand (it
//! is drawn at once if it was on the back): the sword chains three strikes when pressed again in
//! time, the spear thrusts, the club swings. A strike slows the hero, pushes it a little forward
//! and keeps its facing. Strikes hit the foes in reach in front of the hero once each, in the middle
//! of the swing (each weapon its reach, width, damage and push).
//!
//! The bow (G11.6): an attack press alone is a quick shot from the hip, the hero turning to where
//! the camera looks. Holding aim is aim mode (the camera over the shoulder, the hero walking and
//! facing the camera, the bow up with an arrow nocked); an attack press there draws the string and
//! shoots. Either way the arrow flies to the point under the middle of the screen.
use super::animator::Weapon;
use super::arrows::{self, Arrow};
use super::clip::ClipId;
use super::clips::{BOW_NOCKED, clip};
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
    /// Aim mode: the aim button held with the bow.
    aiming: bool,
    /// The bowstring being drawn for a shot.
    draw: Option<Draw>,
    /// Seconds left of a shot's follow-through (no new shot until it ends).
    recover: f32,
    /// An attack pressed during the follow-through, shot once it ends.
    queued_shot: bool,
    /// A weapon key pressed during a strike, picked when it ends (0 is none).
    queued_weapon: u8,
}

impl Combat {
    /// The strike being played, if any.
    pub fn striking(&self) -> Option<ClipId> {
        self.strike.map(|strike| strike.clip)
    }

    /// Aim mode (the `mirando` variable): the camera over the shoulder, the crosshair shown.
    pub fn aiming(&self) -> bool {
        self.aiming
    }

    /// A bow shot is being drawn or followed through.
    pub fn shooting(&self) -> bool {
        self.draw.is_some() || self.recover > 0.0
    }
}

/// A bow shot on its way: seconds drawn, and how long until the arrow goes.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Draw {
    time: f32,
    ready: f32,
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
/// Walking pace in aim mode.
const AIM_SPEED: f32 = 1.8;
/// Aim mode draws from the nocked pose for this long; a hip shot is quicker, from further in.
const AIM_DRAW: f32 = 0.35;
const HIP_DRAW_FROM: f32 = 0.42;
const HIP_DRAW: f32 = 0.18;
/// The follow-through after a shot.
const RECOVER: f32 = 0.3;
/// The hand the arrow leaves from: above the feet, in front.
const HAND_HEIGHT: f32 = 1.45;
const HAND_FORWARD: f32 = 0.4;
/// The sword's next strike counts if pressed after this part of the current one.
const COMBO_FROM: f32 = 0.35;

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
    /// The point under the middle of the screen (arrows fly there).
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
    if combat.shooting() {
        // A hip shot: the hero keeps facing the shot.
        return Pace { face_camera: true, turn: 3.0, ..FREE };
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

/// Out of aim mode, any shot dropped, the bow's pose let go.
fn lower_bow(hero: &mut Hero) {
    let combat = &mut hero.combat;
    let was = combat.aiming || combat.shooting();
    combat.aiming = false;
    combat.draw = None;
    combat.recover = 0.0;
    combat.queued_shot = false;
    if was && hero.animator.overlay_clip().is_some_and(|clip| matches!(clip, ClipId::BowAim | ClipId::BowDraw | ClipId::BowShoot)) {
        hero.animator.release();
    }
}

/// One fixed step, before the movement. `aim` is the aim button held now.
pub(super) fn step(hero: &mut Hero, presses: Presses, aim: bool, field: &mut Field<'_>, dt: f32) {
    let free = hero.moves.action == Action::Free;
    if !free || !hero.grounded {
        // Climbing, rolling, falling or knocked down: strikes and the bow stop.
        if hero.combat.strike.take().is_some() && free {
            // Off an edge mid-swing: back to moving (the fall pose), not the rest of the swing.
            hero.animator.stop();
        }
        lower_bow(hero);
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
        lower_bow(hero);
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
        bow(hero, presses.attack, aim, field, dt);
        return;
    }
    if presses.attack
        && let Some(first) = first_strike(weapon)
    {
        hero.animator.set_weapon(weapon, true);
        start(hero, first);
    }
}

/// The bow this step: into and out of aim mode, a shot drawn, loosed and followed through.
fn bow(hero: &mut Hero, attack: bool, aim: bool, field: &mut Field<'_>, dt: f32) {
    if aim != hero.combat.aiming {
        hero.combat.aiming = aim;
        if aim {
            hero.animator.set_weapon(Weapon::Bow, true);
            // A shot already on its way goes first; the bow comes up nocked after it.
            if !hero.combat.shooting() {
                hero.animator.play(ClipId::BowAim);
            }
        } else if hero.combat.draw.take().is_some() || hero.combat.recover <= 0.0 {
            // Letting go mid-draw drops the shot; otherwise the bow goes down now.
            hero.combat.recover = 0.0;
            hero.combat.queued_shot = false;
            hero.animator.release();
        }
    }
    if attack {
        if hero.combat.recover > 0.0 {
            hero.combat.queued_shot = true;
        } else if hero.combat.draw.is_none() {
            start_draw(hero, field.aim);
        }
    }
    if let Some(mut draw) = hero.combat.draw {
        draw.time += dt;
        hero.combat.draw = Some(draw);
        if draw.time >= draw.ready {
            hero.combat.draw = None;
            hero.combat.recover = RECOVER;
            hero.animator.play(ClipId::BowShoot);
            let forward = Vec3::new(hero.facing.sin(), 0.0, -hero.facing.cos());
            let from = hero.position + Vec3::Y * HAND_HEIGHT + forward * HAND_FORWARD;
            arrows::shoot(field.arrows, from, arrows::toward(from, field.aim, forward), field.events);
        }
    } else if hero.combat.recover > 0.0 {
        hero.combat.recover = (hero.combat.recover - dt).max(0.0);
        if hero.combat.recover <= 0.0 {
            if std::mem::take(&mut hero.combat.queued_shot) {
                start_draw(hero, field.aim);
            } else if hero.combat.aiming {
                hero.animator.play(ClipId::BowAim);
            }
        }
    }
}

/// Start drawing a shot: from the nocked pose in aim mode, or quickly from the hip (turning to it).
fn start_draw(hero: &mut Hero, at: Vec3) {
    hero.animator.set_weapon(Weapon::Bow, true);
    if hero.combat.aiming {
        hero.animator.play_from(ClipId::BowDraw, BOW_NOCKED);
        hero.combat.draw = Some(Draw { time: 0.0, ready: AIM_DRAW });
    } else {
        let to = at - hero.position;
        if to.x.is_finite() && to.z.is_finite() && to.x * to.x + to.z * to.z > 0.01 {
            hero.facing = to.x.atan2(-to.z);
        }
        hero.animator.play_from(ClipId::BowDraw, HIP_DRAW_FROM);
        hero.combat.draw = Some(Draw { time: 0.0, ready: HIP_DRAW });
    }
}
