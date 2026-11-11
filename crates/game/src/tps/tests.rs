use super::*;
use crate::STEP_SECS;
use std::f32::consts::{FRAC_PI_2, PI};

/// Run `seconds` of game in whole steps with `input` held.
fn run(tps: &mut Tps, input: Input, seconds: f64) {
    let steps = (seconds / STEP_SECS).round() as u32;
    for _ in 0..steps {
        tps.input = Input { look: [0.0; 2], jump: false, ..input };
        tps.advance(STEP_SECS);
    }
}

fn forward() -> Input {
    Input { forward: 1.0, ..Input::default() }
}

#[test]
fn the_hero_starts_still_on_the_ground_and_stays_there() {
    let mut tps = Tps::new();
    run(&mut tps, Input::default(), 1.0);
    assert_eq!(tps.hero.position, tps.world().spawn());
    assert!(tps.hero.grounded);
    assert_eq!(tps.hero.speed(), 0.0);
}

#[test]
fn forward_walks_away_from_the_camera_and_speeds_up_instead_of_jumping_to_speed() {
    let mut tps = Tps::new();
    run(&mut tps, forward(), 1.0 / 60.0);
    assert!(tps.hero.speed() > 0.0 && tps.hero.speed() < 1.0, "one step: {}", tps.hero.speed());
    let mut tps = Tps::new();
    run(&mut tps, forward(), 1.0);
    // Camera yaw 0 looks along -Z. A jog is 5 m/s; getting up to it costs a fraction of a metre.
    let moved = tps.hero.position - tps.world().spawn();
    assert!(moved.x.abs() < 0.001);
    assert!((-5.0..-4.3).contains(&moved.z), "moved z = {}", moved.z);
    assert!((tps.hero.speed() - 5.0).abs() < 0.01);
}

#[test]
fn moving_is_relative_to_where_the_camera_looks_in_every_direction() {
    // (camera yaw, forward, strafe) → the way the hero must go (x, z).
    for (yaw, forward, strafe, want) in [
        (0.0, 1.0, 0.0, (0.0, -1.0)),
        (0.0, -1.0, 0.0, (0.0, 1.0)),
        (0.0, 0.0, 1.0, (1.0, 0.0)),
        (0.0, 0.0, -1.0, (-1.0, 0.0)),
        (FRAC_PI_2, 1.0, 0.0, (1.0, 0.0)),
        (PI, 1.0, 0.0, (0.0, 1.0)),
        (PI, 0.0, 1.0, (-1.0, 0.0)),
    ] {
        let mut tps = Tps::new();
        tps.camera.yaw = yaw;
        run(&mut tps, Input { forward, strafe, ..Input::default() }, 0.5);
        let at = tps.hero.position - tps.world().spawn();
        let length = (at.x * at.x + at.z * at.z).sqrt();
        assert!(length > 1.0, "moved {length}");
        assert!((at.x / length - want.0).abs() < 0.01 && (at.z / length - want.1).abs() < 0.01, "yaw {yaw}: went {at:?}, wanted {want:?}");
    }
}

#[test]
fn a_diagonal_is_no_faster_than_straight_and_run_is_faster_than_jog() {
    let mut tps = Tps::new();
    run(&mut tps, Input { forward: 1.0, strafe: 1.0, ..Input::default() }, 1.0);
    assert!(tps.hero.speed() <= 5.001, "{}", tps.hero.speed());
    let mut tps = Tps::new();
    run(&mut tps, Input { forward: 1.0, run: true, ..Input::default() }, 1.5);
    assert!((tps.hero.speed() - 8.5).abs() < 0.01);
    let mut tps = Tps::new();
    run(&mut tps, Input { forward: 1.0, crouch: true, ..Input::default() }, 1.5);
    assert!((tps.hero.speed() - 2.4).abs() < 0.01);
}

#[test]
fn a_light_stick_walks_slower_than_a_full_one() {
    let mut tps = Tps::new();
    run(&mut tps, Input { forward: 0.4, ..Input::default() }, 1.0);
    assert!((tps.hero.speed() - 2.0).abs() < 0.01, "{}", tps.hero.speed());
}

#[test]
fn the_hero_turns_to_face_where_it_goes_smoothly() {
    let mut tps = Tps::new();
    // Right of the camera is east: facing PI/2.
    run(&mut tps, Input { strafe: 1.0, ..Input::default() }, 1.0 / 60.0);
    assert!(tps.hero.facing > 0.0 && tps.hero.facing < FRAC_PI_2 * 0.5, "first step: {}", tps.hero.facing);
    run(&mut tps, Input { strafe: 1.0, ..Input::default() }, 1.0);
    assert!((tps.hero.facing - FRAC_PI_2).abs() < 0.02, "{}", tps.hero.facing);
    // Stopping keeps the facing.
    run(&mut tps, Input::default(), 1.0);
    assert!((tps.hero.facing - FRAC_PI_2).abs() < 0.02);
}

#[test]
fn turning_round_takes_the_short_way() {
    let mut tps = Tps::new();
    // Facing -3.0 and walking towards the camera (wanted: PI) is a turn of 0.14 rad, not 6.14.
    tps.hero.facing = -3.0;
    run(&mut tps, Input { forward: -1.0, ..Input::default() }, 1.0 / 60.0);
    assert!(tps.hero.facing.abs() > 2.9, "went the long way round: {}", tps.hero.facing);
    run(&mut tps, Input { forward: -1.0, ..Input::default() }, 1.0);
    assert!((tps.hero.facing.abs() - PI).abs() < 0.02, "{}", tps.hero.facing);
}

#[test]
fn a_jump_leaves_the_ground_peaks_about_a_metre_up_and_lands() {
    let mut tps = Tps::new();
    let ground = tps.world().spawn().y;
    tps.input.jump = true;
    let mut peak = 0.0_f32;
    let mut airborne = false;
    for _ in 0..120 {
        tps.advance(STEP_SECS);
        peak = peak.max(tps.hero.position.y - ground);
        airborne |= !tps.hero.grounded;
    }
    assert!(airborne);
    assert!((1.0..1.5).contains(&peak), "peak {peak}");
    assert!(tps.hero.grounded && tps.hero.position.y == ground);
}

#[test]
fn pressing_jump_in_the_air_does_not_jump_again() {
    let mut tps = Tps::new();
    tps.input.jump = true;
    tps.advance(STEP_SECS);
    let ground = tps.world().spawn().y;
    let mut peak = 0.0_f32;
    for _ in 0..30 {
        tps.input.jump = true;
        tps.advance(STEP_SECS);
        peak = peak.max(tps.hero.position.y - ground);
    }
    run(&mut tps, Input::default(), 1.0);
    assert!(peak < 1.5, "{peak}");
}

#[test]
fn a_press_shorter_than_a_frame_still_jumps() {
    let mut tps = Tps::new();
    tps.input.jump = true;
    // A very fast display frame: no step is due yet, the press waits for the next one.
    let ground = tps.world().spawn().y;
    assert_eq!(tps.advance(0.001), 0);
    assert_eq!(tps.hero.position.y, ground);
    tps.advance(STEP_SECS);
    assert!(tps.hero.position.y > ground);
}

#[test]
fn the_camera_turns_with_the_look_input_and_never_flips() {
    let mut tps = Tps::new();
    tps.input.look = [FRAC_PI_2, 0.0];
    tps.advance(STEP_SECS);
    assert!((tps.camera.yaw - FRAC_PI_2).abs() < 1e-5);
    assert_eq!(tps.input.look, [0.0; 2], "the look is used once");
    tps.input.look = [0.0, 50.0];
    tps.advance(STEP_SECS);
    assert_eq!(tps.camera.pitch, PITCH_MAX);
    tps.input.look = [0.0, -50.0];
    tps.advance(STEP_SECS);
    assert_eq!(tps.camera.pitch, PITCH_MIN);
    tps.input.look = [-10.0, 0.0];
    tps.advance(STEP_SECS);
    assert!((0.0..std::f32::consts::TAU).contains(&tps.camera.yaw));
}

#[test]
fn the_camera_turns_on_its_own_while_the_hero_stands_still() {
    let mut tps = Tps::new();
    tps.input.look = [1.0, 0.0];
    tps.advance(STEP_SECS);
    assert_eq!(tps.hero.position, tps.world().spawn());
    assert_eq!(tps.hero.facing, 0.0);
    assert!((tps.camera.yaw - 1.0).abs() < 1e-5);
}

#[test]
fn the_hero_is_centred_and_the_camera_orbits_at_a_fixed_distance() {
    let mut tps = Tps::new();
    run(&mut tps, forward(), 0.5);
    for yaw in [0.0, 1.0, 2.5, 4.0] {
        for pitch in [-0.5, -0.25, 0.0, 0.1] {
            tps.camera.yaw = yaw;
            tps.camera.pitch = pitch;
            let frame = tps.frame();
            // The camera looks at the hero's middle…
            assert!((frame.target - (frame.hero.position + Vec3::Y)).length() < 1e-5);
            // …from 4.5 m away, straight along its view direction (the hero is dead centre).
            let to_target = (frame.target - frame.eye).normalize();
            assert!((to_target - tps.camera.direction()).length() < 1e-4);
            assert!(((frame.target - frame.eye).length() - 4.5).abs() < 1e-3);
        }
    }
}

#[test]
fn the_camera_stays_above_the_ground() {
    let mut tps = Tps::new();
    tps.camera.pitch = PITCH_MAX;
    let eye = tps.frame().eye;
    assert!(eye.y >= tps.world().ground_height(eye.x, eye.z) + 0.3 - 1e-6);
}

#[test]
fn the_picture_is_drawn_between_two_steps() {
    let mut tps = Tps::new();
    tps.input = forward();
    // One step and half of the next.
    assert_eq!(tps.advance(STEP_SECS * 1.5), 1);
    let now = tps.hero.position.z;
    let before = tps.hero.pose(0.0).position.z;
    let shown = tps.frame().hero.position.z;
    assert!(now < before, "one step moved it");
    assert!(shown < before && shown > now - 1e-6 && (shown - (before + now) / 2.0).abs() < 1e-4, "shown {shown} between {before} and {now}");
}

#[test]
fn the_same_input_gives_the_same_game_whatever_the_frame_rate() {
    let play = |frame: f64| {
        let mut tps = Tps::new();
        for _ in 0..(120.0 * STEP_SECS / frame).round() as u32 {
            tps.input = Input { forward: 1.0, strafe: 0.3, run: true, ..Input::default() };
            tps.advance(frame);
        }
        tps
    };
    let (slow, fast) = (play(STEP_SECS * 2.0), play(STEP_SECS));
    assert_eq!(slow.ticks(), fast.ticks());
    assert_eq!(slow.hero, fast.hero);
}

#[test]
fn junk_input_changes_nothing_and_never_poisons_the_game() {
    let mut tps = Tps::new();
    for dt in [f64::NAN, f64::INFINITY, -1.0, 0.0] {
        assert_eq!(tps.advance(dt), 0);
    }
    let nan = f32::NAN;
    tps.input = Input { forward: nan, strafe: f32::INFINITY, look: [nan, f32::NEG_INFINITY], ..Input::default() };
    tps.advance(STEP_SECS);
    assert!(tps.hero.position.is_finite() && tps.camera.yaw.is_finite() && tps.camera.pitch.is_finite());
    assert!(tps.frame().eye.is_finite());
    // A hero that was somehow broken is put back at the start.
    tps.hero.position = Vec3::new(nan, 0.0, 0.0);
    tps.input = forward();
    tps.advance(STEP_SECS);
    assert!(tps.hero.position.is_finite());
}

#[test]
fn the_hero_cannot_leave_the_map() {
    let mut tps = Tps::new();
    let limit = tps.world().walk_limit();
    tps.hero.position = Vec3::new(limit - 0.1, 0.0, 0.0);
    tps.camera.yaw = FRAC_PI_2;
    run(&mut tps, Input { forward: 1.0, run: true, ..Input::default() }, 2.0);
    assert_eq!(tps.hero.position.x, limit);
}

#[test]
fn paused_means_nothing_moves_and_nothing_is_kept() {
    let mut tps = Tps::new();
    tps.paused = true;
    tps.input = Input { forward: 1.0, look: [1.0, 1.0], jump: true, ..Input::default() };
    assert_eq!(tps.advance(1.0), 0);
    assert_eq!(tps.hero.position, tps.world().spawn());
    assert_eq!(tps.camera, OrbitCamera::default());
    tps.paused = false;
    tps.input = Input::default();
    tps.advance(STEP_SECS);
    assert_eq!(tps.hero.position.y, tps.world().spawn().y, "the jump pressed while paused was dropped");
}
