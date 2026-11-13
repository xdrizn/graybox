//! G11.5: health and falls, death and respawn, dummies and husks, strikes landing, arrows, events.
use super::*;
use crate::state::Input;

fn run(tps: &mut Tps, input: Input, seconds: f32) {
    for _ in 0..(seconds * 60.0).round() as usize {
        tps.input = input;
        tps.advance(STEP_SECS);
    }
}

fn click(tps: &mut Tps) {
    run(tps, Input { shoot: true, ..Input::default() }, 1.0 / 60.0);
    run(tps, Input::default(), 1.0 / 60.0);
}

/// A game with only the foes `keep` says, the hero turned to look at the first one.
fn facing_first(keep: impl Fn(&Foe) -> bool) -> Tps {
    let mut tps = Tps::new();
    tps.foes.retain(|foe| keep(foe));
    if let Some(foe) = tps.foes.first() {
        let to = foe.position - tps.hero.position;
        tps.hero.facing = to.x.atan2(-to.z);
        tps.camera.yaw = tps.hero.facing;
    }
    tps
}

/// Stand the hero `distance` metres in front of the first foe, looking at it.
fn before_first(tps: &mut Tps, distance: f32) {
    let Some(foe) = tps.foes.first().copied() else { panic!("no foe") };
    let at = foe.position + Vec3::new(0.0, 0.0, distance);
    let y = tps.world().ground_height(at.x, at.z);
    tps.hero = Hero::at(Vec3::new(at.x, y, at.z));
    tps.hero.facing = 0.0;
    tps.camera.yaw = 0.0;
}

#[test]
fn the_valley_has_dummies_ahead_of_the_start_and_husks_further_out() {
    let tps = Tps::new();
    let start = tps.hero.position;
    let dummies: Vec<&Foe> = tps.foes.iter().filter(|foe| foe.kind == FoeKind::Dummy).collect();
    let husks: Vec<&Foe> = tps.foes.iter().filter(|foe| foe.kind == FoeKind::Husk).collect();
    assert_eq!(dummies.len(), 3);
    assert!(husks.len() >= 3, "{} husks", husks.len());
    assert!(tps.foes.len() <= MAX_FOES);
    for dummy in dummies {
        assert!((dummy.position - start).length() < 12.0, "a dummy near the start: {:?}", dummy.position);
    }
    for husk in husks {
        assert!((husk.position - start).length() > 20.0, "husks are further out: {:?}", husk.position);
    }
    for foe in &tps.foes {
        let ground = tps.world().ground_height(foe.position.x, foe.position.z);
        assert!((foe.position.y - ground).abs() < 0.05, "{foe:?} stands on the ground");
    }
}

#[test]
fn a_fall_over_four_metres_hurts_unless_rolled_and_says_so() {
    let mut tps = Tps::new();
    tps.foes.clear();
    tps.hero.position.y += 8.0;
    tps.hero.grounded = false;
    run(&mut tps, Input::default(), 2.0);
    let health = tps.hero.vitals.health;
    assert!(health < HEALTH_MAX && health > 0.0, "health {health}");
    assert_eq!(tps.hero.vitals.attacker, "Fall");
    let events = tps.drain_events();
    assert!(events.contains(&GameEvent::TookDamage) && events.contains(&GameEvent::HealthChanged), "{events:?}");

    let mut tps = Tps::new();
    tps.foes.clear();
    tps.hero.position.y += 3.5;
    tps.hero.grounded = false;
    run(&mut tps, Input::default(), 1.5);
    assert_eq!(tps.hero.vitals.health, HEALTH_MAX, "a short fall is free");

    let mut tps = Tps::new();
    tps.foes.clear();
    tps.hero.position.y += 8.0;
    tps.hero.grounded = false;
    for _ in 0..200 {
        let low = tps.hero.position.y - tps.world().ground_height(tps.hero.position.x, tps.hero.position.z) < 2.5;
        run(&mut tps, Input { roll: low, ..Input::default() }, 1.0 / 60.0);
        if tps.hero.grounded {
            break;
        }
    }
    assert_eq!(tps.hero.vitals.health, HEALTH_MAX, "the roll takes the fall");
}

#[test]
fn out_of_health_the_hero_dies_lies_still_and_respawns_at_the_start() {
    let mut tps = Tps::new();
    tps.foes.clear();
    let start = tps.hero.position;
    tps.hero.position.y += 40.0;
    tps.hero.grounded = false;
    run(&mut tps, Input::default(), 3.5);
    assert!(tps.dead(), "a 40 m fall kills: {:?}", tps.hero.moves.action);
    assert_eq!(tps.hero.vitals.health, 0.0);
    assert!(tps.drain_events().contains(&GameEvent::Died));
    let lying = tps.hero.position;
    run(&mut tps, Input { forward: 1.0, jump: true, shoot: true, ..Input::default() }, 1.0);
    assert!((tps.hero.position - lying).length() < 0.05, "a dead hero does not move");
    assert!(tps.dead());
    assert_eq!(tps.hero.animator.current(), ClipId::Die, "it lies there");
    run(&mut tps, Input::default(), 5.0);
    assert_eq!(tps.hero.animator.current(), ClipId::Die, "still lying there");

    tps.respawn();
    assert!(!tps.dead());
    assert_eq!(tps.hero.vitals.health, HEALTH_MAX);
    assert!((tps.hero.position - start).length() < 0.01);
    assert!(tps.drain_events().contains(&GameEvent::Respawned));
    run(&mut tps, Input::default(), 0.5);
    assert_eq!(tps.hero.moves.action, Action::Free);
}

#[test]
fn sword_strikes_land_once_each_and_break_a_dummy_which_comes_back() {
    let mut tps = facing_first(|foe| foe.kind == FoeKind::Dummy);
    before_first(&mut tps, 1.4);
    let full = tps.foes[0].health;
    click(&mut tps);
    run(&mut tps, Input::default(), 1.0);
    assert_eq!(tps.hero.combat.striking(), None);
    let after_one = tps.foes[0].health;
    assert!(after_one < full, "the strike landed");
    assert!(after_one > full - 25.0, "once, not every step: {after_one}");
    for _ in 0..6 {
        click(&mut tps);
        run(&mut tps, Input::default(), 1.0);
        if !tps.foes[0].alive() {
            break;
        }
    }
    assert!(!tps.foes[0].alive(), "broken apart: {:?}", tps.foes[0]);
    assert!(tps.drain_events().contains(&GameEvent::EnemyKilled));
    run(&mut tps, Input::default(), 5.0);
    assert!(tps.foes[0].alive(), "it comes back");
    assert_eq!(tps.foes[0].health, full);
}

#[test]
fn a_strike_misses_what_is_behind_or_out_of_reach() {
    let mut tps = facing_first(|foe| foe.kind == FoeKind::Dummy);
    before_first(&mut tps, 4.0);
    let full = tps.foes[0].health;
    click(&mut tps);
    run(&mut tps, Input::default(), 1.0);
    assert_eq!(tps.foes[0].health, full, "out of reach");
    before_first(&mut tps, 1.4);
    tps.hero.facing = std::f32::consts::PI;
    tps.camera.yaw = std::f32::consts::PI;
    click(&mut tps);
    run(&mut tps, Input::default(), 1.0);
    assert_eq!(tps.foes[0].health, full, "behind the hero");
}

#[test]
fn the_club_hits_harder_than_the_sword_and_the_spear_reaches_further() {
    let mut tps = facing_first(|foe| foe.kind == FoeKind::Dummy);
    before_first(&mut tps, 1.4);
    run(&mut tps, Input { weapon: 3, ..Input::default() }, 1.0 / 60.0);
    run(&mut tps, Input::default(), 0.8);
    let full = tps.foes[0].health;
    click(&mut tps);
    run(&mut tps, Input::default(), 1.4);
    assert!(full - tps.foes[0].health >= 30.0, "club: {}", full - tps.foes[0].health);

    let mut tps = facing_first(|foe| foe.kind == FoeKind::Dummy);
    before_first(&mut tps, 2.8);
    run(&mut tps, Input { weapon: 2, ..Input::default() }, 1.0 / 60.0);
    run(&mut tps, Input::default(), 0.8);
    let full = tps.foes[0].health;
    click(&mut tps);
    run(&mut tps, Input::default(), 1.2);
    assert!(tps.foes[0].health < full, "the spear reaches 2.8 m");
}

#[test]
fn a_husk_walks_to_the_hero_winds_up_and_hurts_it() {
    let mut tps = facing_first(|foe| foe.kind == FoeKind::Husk);
    before_first(&mut tps, 8.0);
    let husk_start = tps.foes[0].position;
    let mut hurt_at = None;
    for step in 0..(12 * 60) {
        run(&mut tps, Input::default(), 1.0 / 60.0);
        if tps.hero.vitals.health < HEALTH_MAX {
            hurt_at = Some(step);
            break;
        }
    }
    assert!(hurt_at.is_some(), "the husk never hit: {:?}", tps.foes[0]);
    assert!((tps.foes[0].position - husk_start).length() > 5.0, "it walked over");
    assert_eq!(tps.hero.vitals.health, HEALTH_MAX - HUSK_DAMAGE);
    assert_eq!(tps.hero.vitals.attacker, "Husk");
    assert!(tps.hero.velocity.length() > 2.0, "knocked back");
}

#[test]
fn a_roll_through_the_swing_dodges_it_and_a_strike_staggers_the_wind_up() {
    let mut tps = facing_first(|foe| foe.kind == FoeKind::Husk);
    before_first(&mut tps, 1.2);
    // Wait for the wind-up, then roll as it ends.
    for _ in 0..120 {
        run(&mut tps, Input::default(), 1.0 / 60.0);
        if let FoeState::WindUp { time } = tps.foes[0].state
            && time > 0.55
        {
            break;
        }
    }
    assert!(matches!(tps.foes[0].state, FoeState::WindUp { .. }), "{:?}", tps.foes[0].state);
    run(&mut tps, Input { roll: true, strafe: 1.0, ..Input::default() }, 1.0 / 60.0);
    run(&mut tps, Input::default(), 0.3);
    assert_eq!(tps.hero.vitals.health, HEALTH_MAX, "rolled through it");

    let mut tps = facing_first(|foe| foe.kind == FoeKind::Husk);
    before_first(&mut tps, 1.2);
    for _ in 0..120 {
        run(&mut tps, Input::default(), 1.0 / 60.0);
        if matches!(tps.foes[0].state, FoeState::WindUp { .. }) {
            break;
        }
    }
    click(&mut tps);
    run(&mut tps, Input::default(), 0.2);
    assert!(matches!(tps.foes[0].state, FoeState::Stagger { .. }), "staggered: {:?}", tps.foes[0].state);
    run(&mut tps, Input::default(), 0.4);
    assert_eq!(tps.hero.vitals.health, HEALTH_MAX, "the stagger lost its blow");
}

#[test]
fn husks_give_up_on_a_dead_hero_and_walk_home() {
    let mut tps = facing_first(|foe| foe.kind == FoeKind::Husk);
    before_first(&mut tps, 6.0);
    run(&mut tps, Input::default(), 2.0);
    let home = tps.foes[0].home;
    assert!((tps.foes[0].position - home).length() > 1.0, "it came for the hero");
    tps.hero.vitals.health = 1.0;
    tps.hero.position.y += 30.0;
    tps.hero.grounded = false;
    run(&mut tps, Input::default(), 3.0);
    assert!(tps.dead());
    run(&mut tps, Input::default(), 10.0);
    assert!((tps.foes[0].position - home).length() < 1.0, "back home: {:?} vs {home:?}", tps.foes[0].position);
}

#[test]
fn a_drawn_bow_shoots_an_arrow_that_hits_a_dummy_and_a_tap_shoots_nothing() {
    let mut tps = facing_first(|foe| foe.kind == FoeKind::Dummy);
    before_first(&mut tps, 10.0);
    // Aim at the dummy's middle (the hero may stand higher or lower on the hill).
    let rise = tps.foes[0].position.y + 1.0 - (tps.hero.position.y + 1.45);
    tps.camera.pitch = rise.atan2(10.0);
    run(&mut tps, Input { weapon: 4, ..Input::default() }, 1.0 / 60.0);
    run(&mut tps, Input::default(), 0.8);
    run(&mut tps, Input { shoot: true, ..Input::default() }, 2.0 / 60.0);
    run(&mut tps, Input::default(), 0.5);
    assert!(tps.arrows.is_empty(), "a tap shoots nothing");
    let full = tps.foes[0].health;
    tps.drain_events();
    run(&mut tps, Input { shoot: true, ..Input::default() }, 0.6);
    run(&mut tps, Input::default(), 1.0 / 60.0);
    assert_eq!(tps.arrows.len(), 1);
    assert!(tps.drain_events().contains(&GameEvent::Shot));
    run(&mut tps, Input::default(), 1.0);
    assert_eq!(tps.foes[0].health, full - ARROW_DAMAGE, "the arrow hit");
    assert!(tps.arrows.is_empty(), "spent on the hit");
}

#[test]
fn an_arrow_into_the_ground_sticks_and_is_gone_later() {
    let mut tps = Tps::new();
    tps.foes.clear();
    tps.camera.pitch = -0.6;
    run(&mut tps, Input { weapon: 4, ..Input::default() }, 1.0 / 60.0);
    run(&mut tps, Input::default(), 0.8);
    run(&mut tps, Input { shoot: true, ..Input::default() }, 0.6);
    run(&mut tps, Input::default(), 1.0);
    assert_eq!(tps.arrows.len(), 1);
    assert!(tps.arrows[0].stuck.is_some(), "stuck: {:?}", tps.arrows[0]);
    let stuck_at = tps.arrows[0].position;
    run(&mut tps, Input::default(), 1.0);
    assert_eq!(tps.arrows.first().map(|arrow| arrow.position), Some(stuck_at));
    run(&mut tps, Input::default(), 7.0);
    assert!(tps.arrows.is_empty());
}

#[test]
fn a_long_run_with_foes_stays_finite_and_the_frame_lists_them() {
    let mut tps = Tps::new();
    let input = Input { forward: 1.0, run: true, shoot: true, ..Input::default() };
    for second in 0..60 {
        tps.camera.yaw = second as f32 * 0.7;
        run(&mut tps, input, 1.0);
        if tps.dead() {
            tps.respawn();
        }
    }
    assert!(tps.foes.iter().all(|foe| foe.position.is_finite() && foe.health.is_finite()));
    assert!(tps.hero.position.is_finite());
    let frame = tps.frame();
    assert_eq!(frame.foe_count, tps.foes.len());
    assert_eq!(frame.arrow_count, tps.arrows.len());
}

#[test]
fn killed_by_a_husk_the_hero_lies_there_until_it_respawns() {
    let mut tps = facing_first(|foe| foe.kind == FoeKind::Husk);
    before_first(&mut tps, 1.2);
    tps.hero.vitals.health = 5.0;
    for _ in 0..(6 * 60) {
        run(&mut tps, Input::default(), 1.0 / 60.0);
        if tps.dead() {
            break;
        }
    }
    assert!(tps.dead(), "the husk killed it");
    for second in 0..6 {
        run(&mut tps, Input::default(), 1.0);
        assert_eq!(tps.hero.animator.current(), ClipId::Die, "lying there after {second} s: {:?}", tps.hero.moves.action);
    }
}

#[test]
fn the_hero_does_not_walk_through_a_dummy() {
    let mut tps = facing_first(|foe| foe.kind == FoeKind::Dummy);
    before_first(&mut tps, 3.0);
    run(&mut tps, Input { forward: 1.0, ..Input::default() }, 3.0);
    let dummy = tps.foes[0].position;
    let gap = glam::Vec2::new(tps.hero.position.x - dummy.x, tps.hero.position.z - dummy.z).length();
    assert!(gap >= FOE_RADIUS + hero::RADIUS - 0.05, "stopped at the dummy: {gap}");
    assert!(tps.hero.position.z > dummy.z, "still in front of it");
}

#[test]
fn a_hit_during_a_weapon_draw_does_not_leave_the_weapon_on_the_back() {
    let mut tps = facing_first(|foe| foe.kind == FoeKind::Husk);
    tps.foes.clear();
    run(&mut tps, Input { weapon: 4, ..Input::default() }, 1.0 / 60.0);
    run(&mut tps, Input::default(), 0.1);
    damage::hurt(&mut tps.hero, 10.0, Vec3::X, "Husk", &mut tps.events);
    run(&mut tps, Input::default(), 1.0);
    assert_eq!(tps.hero.animator.weapon(), Weapon::Bow);
    assert!(tps.hero.animator.in_hand(), "the bow reached the hand");
}

#[test]
fn a_paused_game_still_lets_the_dead_hero_fall_down() {
    let mut tps = facing_first(|foe| foe.kind == FoeKind::Husk);
    tps.foes.clear();
    run(&mut tps, Input::default(), 0.2);
    damage::hurt(&mut tps.hero, 1000.0, Vec3::ZERO, "Husk", &mut tps.events);
    assert!(tps.dead());
    // A death screen pauses the game the moment it opens.
    tps.paused = true;
    let standing = tps.hero.pose(1.0);
    run(&mut tps, Input { forward: 1.0, ..Input::default() }, 2.0);
    assert_ne!(tps.hero.pose(1.0), standing, "the fall plays");
    assert!(tps.dead());
    assert_eq!(tps.hero.vitals.health, 0.0);
}
