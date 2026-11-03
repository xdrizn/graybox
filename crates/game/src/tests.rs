use super::{FixedClock, Game, MAX_STEPS, STEP_SECS};

#[test]
fn a_second_of_frames_is_sixty_steps_at_any_frame_rate() {
    for fps in [30.0, 60.0, 144.0, 240.0] {
        let mut g = Game::new();
        let frames = fps as usize;
        let steps: u32 = (0..frames).map(|_| g.advance(1.0 / fps)).sum();
        // Rounding may leave the last step a hair short of due.
        assert!((59..=60).contains(&steps), "{fps} fps: {steps} steps");
        assert_eq!(g.ticks(), u64::from(steps));
        assert!((g.time() - f64::from(steps) * STEP_SECS).abs() < 1e-9);
    }
}

#[test]
fn time_shorter_than_a_step_waits_for_the_next() {
    let mut c = FixedClock::default();
    assert_eq!(c.advance(STEP_SECS * 0.4), 0);
    assert!((c.alpha() - 0.4).abs() < 1e-9);
    assert_eq!(c.advance(STEP_SECS * 0.7), 1);
    assert!((c.alpha() - 0.1).abs() < 1e-9);
}

#[test]
fn a_late_frame_runs_a_few_steps_and_drops_the_backlog() {
    let mut g = Game::new();
    assert_eq!(g.advance(10.0), MAX_STEPS);
    assert_eq!(g.alpha(), 0.0, "the backlog is gone");
    // The next ordinary frame is one ordinary step, not a catch-up.
    assert_eq!(g.advance(STEP_SECS), 1);
}

#[test]
fn junk_frame_times_change_nothing() {
    for dt in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut g = Game::new();
        assert_eq!(g.advance(dt), 0, "{dt}");
        assert_eq!(g, Game::new(), "{dt}");
    }
    // Huge but finite: bounded, and the clock stays usable.
    let mut g = Game::new();
    assert_eq!(g.advance(f64::MAX), MAX_STEPS);
    assert_eq!(g.alpha(), 0.0);
    assert_eq!(g.advance(STEP_SECS * 0.5), 0);
    assert_eq!(g.advance(f64::MAX), MAX_STEPS, "a second huge frame doesn't overflow");
}

use super::{GameEvent, MAGAZINE_SIZE, MAX_DEBRIS, MAX_PROJECTILES, Projectile, Sentinel, arena};
use glam::Vec3;

fn quiet_game(position: Vec3) -> Game {
    let mut game = Game::new();
    game.enemies.clear();
    game.player.position = position;
    game
}

fn frames(game: &mut Game, count: usize) {
    for _ in 0..count {
        game.advance(STEP_SECS);
    }
}

#[test]
fn movement_is_fixed_step_deterministic_and_diagonals_are_normalized() {
    let mut sixty = quiet_game(Vec3::ZERO);
    sixty.input.forward = 1.0;
    sixty.input.strafe = 1.0;
    let mut thirty = sixty.clone();
    frames(&mut sixty, 60);
    for _ in 0..30 {
        thirty.advance(STEP_SECS * 2.0);
    }
    assert_eq!(sixty, thirty);
    assert!((sixty.player.position.length() - 4.6).abs() < 0.01);
}

#[test]
fn controller_cannot_walk_through_outer_walls_or_crates() {
    let mut game = quiet_game(Vec3::new(0.0, 0.0, 14.0));
    game.input.strafe = 1.0;
    game.input.run = true;
    frames(&mut game, 240);
    assert!(game.player.position.x <= 19.701);
    assert!(game.player.position.x > 19.4);
    let mut game = quiet_game(Vec3::new(5.0, 0.0, -6.0));
    game.input.forward = 1.0;
    frames(&mut game, 120);
    assert!(game.player.position.z >= -8.701);
    assert_eq!(game.player.position.y, 0.0);
}

#[test]
fn stairs_and_ramp_both_reach_their_landings_and_snap_down() {
    for (x, height) in [(15.0, 4.0), (-15.0, 3.0)] {
        let mut game = quiet_game(Vec3::new(x, 0.0, 4.0));
        game.input.forward = 1.0;
        frames(&mut game, 240);
        assert!((game.player.position.y - height).abs() < 0.02, "{x}: {:?}", game.player.position);
        assert!(game.player.position.z < -12.0, "{x}: {:?}", game.player.position);
        game.input.forward = -1.0;
        frames(&mut game, 240);
        assert!(game.player.position.y.abs() < 0.02);
    }
}

#[test]
fn jump_is_an_edge_and_crouch_run_change_speed_and_eye_height() {
    let mut game = quiet_game(Vec3::ZERO);
    frames(&mut game, 1);
    game.input.jump = true;
    frames(&mut game, 1);
    assert!(game.player.position.y > 0.0);
    assert!(!game.input.jump);
    frames(&mut game, 90);
    assert_eq!(game.player.position.y, 0.0);
    let mut walk = quiet_game(Vec3::new(0.0, 0.0, 14.0));
    walk.input.forward = 1.0;
    let mut run = walk.clone();
    run.input.run = true;
    let mut crouch = run.clone();
    crouch.input.crouch = true;
    frames(&mut walk, 30);
    frames(&mut run, 30);
    frames(&mut crouch, 30);
    assert!(run.player.position.z < walk.player.position.z);
    assert!(crouch.player.position.z > walk.player.position.z);
    assert!(crouch.player.crouching);
    assert!((crouch.player.eye().y - 0.95).abs() < 0.001);
}

#[test]
fn crouch_cannot_stand_inside_a_low_ceiling() {
    let mut game = quiet_game(Vec3::ZERO);
    game.player.crouching = true;
    let mut ceiling = game.objects[0];
    ceiling.position = Vec3::new(0.0, 1.5, 0.0);
    ceiling.size = Vec3::new(2.0, 0.2, 2.0);
    game.objects.push(ceiling);
    frames(&mut game, 1);
    assert!(game.player.crouching);
    game.player.position.x = 3.0;
    frames(&mut game, 1);
    assert!(!game.player.crouching);
}

#[test]
fn look_is_applied_once_per_display_frame_and_rejects_invalid_values() {
    let mut game = quiet_game(Vec3::ZERO);
    game.input.look = [0.5, 0.2];
    game.advance(STEP_SECS * 3.0);
    assert!((game.player.yaw - 0.5).abs() < 0.0001);
    assert!((game.player.pitch - 0.2).abs() < 0.0001);
    assert!(game.player.direction().x > 0.0);
    assert!(game.player.direction().y > 0.0);
    game.input.look = [f32::NAN, f32::INFINITY];
    game.input.forward = f32::NAN;
    game.input.strafe = f32::INFINITY;
    frames(&mut game, 1);
    assert!(game.player.position.is_finite());
    assert!((game.player.yaw - 0.5).abs() < 0.0001);
    game.input.look = [0.0, 10000.0];
    frames(&mut game, 1);
    assert!(game.player.pitch < std::f32::consts::FRAC_PI_2);
}

#[test]
fn edge_actions_survive_a_display_frame_shorter_than_a_step() {
    let mut game = quiet_game(Vec3::ZERO);
    frames(&mut game, 1);
    game.input.jump = true;
    assert_eq!(game.advance(STEP_SECS * 0.4), 0);
    assert!(!game.input.jump);
    game.advance(STEP_SECS * 0.7);
    assert!(game.player.position.y > 0.0);
}

#[test]
fn reload_conserves_ammunition_and_blocks_shooting_until_finished() {
    let mut game = quiet_game(Vec3::ZERO);
    game.player.ammo = 3;
    game.player.reserve = 5;
    game.input.reload = true;
    game.input.shoot = true;
    frames(&mut game, 1);
    assert!(game.player.reload_remaining > 0.0);
    assert_eq!(game.player.ammo + game.player.reserve, 8);
    game.input.shoot = false;
    frames(&mut game, 49);
    assert_eq!(game.player.ammo, 8);
    assert_eq!(game.player.reserve, 0);
    assert_eq!(game.player.reload_remaining, 0.0);
    assert!(game.drain_events().contains(&GameEvent::Reloaded));
    game.input.reload = true;
    frames(&mut game, 1);
    assert_eq!(game.player.reload_remaining, 0.0);
    game.player.ammo = MAGAZINE_SIZE;
    game.player.reserve = 20;
    game.input.reload = true;
    frames(&mut game, 1);
    assert_eq!(game.player.ammo + game.player.reserve, 32);
    assert_eq!(game.player.reload_remaining, 0.0);
}

#[test]
fn pistol_is_stopped_by_the_same_static_geometry_as_line_of_sight() {
    let mut game = quiet_game(Vec3::new(9.0, 0.0, 10.0));
    game.enemies.push(Sentinel::new(Vec3::new(9.0, 0.0, 2.0)));
    game.input.shoot = true;
    frames(&mut game, 1);
    assert_eq!(game.player.ammo, MAGAZINE_SIZE - 1);
    assert_eq!(game.enemies[0].health, 3);
    assert!(arena::raycast(&game.objects, game.player.eye(), -Vec3::Z, 10.0).is_some());
    game.input.shoot = false;
    game.enemies[0].fire_remaining = 0.0;
    frames(&mut game, 1);
    assert!(game.projectiles.is_empty());
}

#[test]
fn three_pistol_hits_kill_sentinel_emit_debris_and_respawn_after_five_seconds() {
    let mut game = quiet_game(Vec3::ZERO);
    let mut enemy = Sentinel::new(Vec3::new(0.0, 0.0, -5.0));
    enemy.fire_remaining = 1000.0;
    game.enemies.push(enemy);
    for expected in [2, 1, 0] {
        game.input.shoot = true;
        frames(&mut game, 1);
        game.input.shoot = false;
        assert_eq!(game.enemies[0].health, expected);
        assert!(game.enemies[0].hit_remaining > 0.0);
        if expected != 0 {
            frames(&mut game, 14);
        }
    }
    assert_eq!(game.debris.len(), 9);
    assert!(game.drain_events().contains(&GameEvent::EnemyKilled));
    frames(&mut game, 298);
    assert_eq!(game.enemies[0].health, 0);
    frames(&mut game, 3);
    assert_eq!(game.enemies[0].health, 3);
    assert!(game.debris.is_empty());
}

#[test]
fn projectile_sweeps_damage_player_and_expire_without_unbounded_growth() {
    let mut game = quiet_game(Vec3::ZERO);
    game.projectiles.push(Projectile { position: Vec3::new(0.0, 1.5, 1.0), velocity: -Vec3::Z * 12.0, remaining: 4.0 });
    frames(&mut game, 8);
    assert_eq!(game.player.health, 85.0);
    assert!(game.projectiles.is_empty());
    game.projectiles.push(Projectile { position: Vec3::new(0.0, 5.0, 0.0), velocity: Vec3::X * 0.01, remaining: 4.0 });
    frames(&mut game, 241);
    assert!(game.projectiles.is_empty());
    assert!(game.debris.len() <= MAX_DEBRIS);
    assert!(game.projectiles.len() <= MAX_PROJECTILES);
}

#[test]
fn sentinel_with_line_of_sight_fires_at_twelve_metres_per_second() {
    let mut game = quiet_game(Vec3::ZERO);
    let mut enemy = Sentinel::new(Vec3::new(0.0, 0.0, -5.0));
    enemy.fire_remaining = 0.0;
    game.enemies.push(enemy);
    frames(&mut game, 1);
    assert_eq!(game.projectiles.len(), 1);
    assert!((game.projectiles[0].velocity.length() - 12.0).abs() < 0.001);
    assert!((2.8..=4.2).contains(&game.enemies[0].fire_remaining));
    frames(&mut game, 30);
    assert!(game.player.health < 100.0);
}

#[test]
fn health_status_death_and_respawn_have_distinct_idempotent_transitions() {
    let mut game = quiet_game(Vec3::ZERO);
    game.damage(30.0, "teste");
    assert_eq!(game.player.health, 70.0);
    assert_eq!(game.player.attacker, "teste");
    assert_eq!(game.player.last_damage, 30.0);
    assert_eq!(game.drain_events(), [GameEvent::HealthChanged, GameEvent::TookDamage]);
    game.heal(20.0);
    assert_eq!(game.player.health, 90.0);
    game.set_on_fire(true);
    game.set_on_fire(true);
    assert_eq!(game.drain_events(), [GameEvent::HealthChanged, GameEvent::Healed, GameEvent::StatusStarted]);
    frames(&mut game, 61);
    assert_eq!(game.player.health, 82.0);
    game.damage(1000.0, "fatal");
    assert!(game.player.dead);
    assert!(!game.player.on_fire);
    assert_eq!(game.player.health, 0.0);
    let position = game.player.position;
    game.input.forward = 1.0;
    game.input.shoot = true;
    game.heal(100.0);
    frames(&mut game, 30);
    assert_eq!(game.player.position, position);
    assert_eq!(game.player.health, 0.0);
    let events = game.drain_events();
    assert_eq!(events.iter().filter(|event| **event == GameEvent::Died).count(), 1);
    assert!(events.contains(&GameEvent::StatusEnded));
    game.input.respawn = true;
    game.input.shoot = false;
    game.input.forward = 0.0;
    frames(&mut game, 1);
    assert!(!game.player.dead);
    assert_eq!(game.player.health, game.player.max_health);
    assert_eq!(game.player.ammo, MAGAZINE_SIZE);
    assert!(game.drain_events().contains(&GameEvent::Respawned));
}

#[test]
fn invalid_damage_healing_and_paused_frames_do_not_change_state() {
    let mut game = quiet_game(Vec3::ZERO);
    let before = game.clone();
    for amount in [f32::NAN, f32::INFINITY, -10.0, 0.0] {
        game.damage(amount, "invalid");
        game.heal(amount);
    }
    assert_eq!(game, before);
    game.paused = true;
    game.input.forward = 1.0;
    let ticks = game.ticks();
    frames(&mut game, 120);
    assert_eq!(game.ticks(), ticks);
    assert_eq!(game.player.position, Vec3::ZERO);
}

#[test]
fn arena_generation_is_repeatable_and_rotated_ramp_blocks_rays() {
    let objects = arena::objects();
    assert_eq!(objects, arena::objects());
    let ramp = objects.iter().find(|object| object.is_ramp());
    assert!(ramp.is_some());
    let hit = arena::raycast(&objects, Vec3::new(-15.0, 8.0, -2.5), -Vec3::Y, 10.0);
    assert!(hit.is_some_and(|distance| (distance - 6.5).abs() < 0.02));
    assert_eq!(arena::raycast(&objects, Vec3::ZERO, Vec3::splat(f32::NAN), 5.0), None);
}

#[test]
fn empty_magazine_emits_an_event_without_creating_a_shot() {
    let mut game = quiet_game(Vec3::ZERO);
    game.player.ammo = 0;
    game.input.shoot = true;
    frames(&mut game, 1);
    assert_eq!(game.player.ammo, 0);
    assert_eq!(game.drain_events(), [GameEvent::AmmoEmpty]);
}

#[test]
fn sentinels_do_not_fire_beyond_their_twenty_four_metre_range() {
    let mut game = quiet_game(Vec3::new(0.0, 0.0, 12.0));
    let mut enemy = Sentinel::new(Vec3::new(0.0, 0.0, -20.0));
    enemy.fire_remaining = 0.0;
    game.enemies.push(enemy);
    frames(&mut game, 120);
    assert!(game.projectiles.is_empty());
    assert_eq!(game.player.health, 100.0);
}

#[test]
fn entity_and_event_storage_is_bounded_and_clones_keep_hot_path_capacity() {
    let mut game = quiet_game(Vec3::ZERO).clone();
    assert!(game.projectiles.capacity() >= MAX_PROJECTILES);
    assert!(game.debris.capacity() >= MAX_DEBRIS);
    assert!(game.player.attacker.capacity() >= 96);
    for _ in 0..2000 {
        game.set_on_fire(!game.player.on_fire);
    }
    assert_eq!(game.events.len(), super::EVENT_CAPACITY);
    game.drain_events();
    assert!(game.events.capacity() >= super::EVENT_CAPACITY);
    for _ in 0..20 {
        game.enemies.push(Sentinel::new(Vec3::new(0.0, 0.0, -5.0)));
    }
    frames(&mut game, 1);
    assert_eq!(game.enemies.len(), super::MAX_ENEMIES);
    for _ in 0..20 {
        game.spawn_debris(Vec3::Y);
    }
    assert_eq!(game.debris.len(), MAX_DEBRIS);
}

#[test]
fn a_quick_shot_between_fixed_steps_is_not_lost_or_repeated() {
    let mut game = quiet_game(Vec3::ZERO);
    game.input.shoot = true;
    assert_eq!(game.advance(STEP_SECS * 0.25), 0);
    game.input.shoot = false;
    game.advance(STEP_SECS);
    assert_eq!(game.player.ammo, 11);
    assert_eq!(game.drain_events().iter().filter(|e| **e == GameEvent::Shot).count(), 1);
    frames(&mut game, 30);
    assert_eq!(game.player.ammo, 11);
}
