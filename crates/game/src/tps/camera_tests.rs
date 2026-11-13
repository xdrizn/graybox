//! G11.6: how the camera feels. It eases back behind the hero after a while without camera input
//! (never while the player steers), pulls in at once when something comes between it and the hero
//! but eases back out, and follows a jump softly instead of bobbing with it.
use super::*;
use crate::state::Input;
use hero::wrap_pi;

fn step(tps: &mut Tps, input: Input) {
    tps.input = input;
    tps.advance(STEP_SECS);
}

fn run(tps: &mut Tps, input: Input, seconds: f32) {
    for _ in 0..(seconds * 60.0).round() as usize {
        step(tps, input);
    }
}

/// A still hero facing `facing`, the camera looking along yaw 0.
fn standing(facing: f32) -> Tps {
    let mut tps = Tps::new();
    tps.hero.facing = facing;
    tps.hero.previous_facing = facing;
    tps.camera.yaw = 0.0;
    tps
}

#[test]
fn after_a_while_without_camera_input_the_camera_eases_back_behind_the_hero() {
    let mut tps = standing(1.2);
    run(&mut tps, Input::default(), camera::RECENTER_AFTER - 0.2);
    assert_eq!(tps.camera.yaw, 0.0, "not before the wait is over");
    tps.camera.pitch = 0.4;
    let mut yaw = tps.camera.yaw;
    let mut biggest = 0.0_f32;
    let mut first = None;
    for _ in 0..(6.0 * 60.0) as usize {
        step(&mut tps, Input::default());
        let moved = wrap_pi(tps.camera.yaw - yaw).abs();
        if moved > 0.0 && first.is_none() {
            first = Some(moved);
        }
        biggest = biggest.max(moved);
        yaw = tps.camera.yaw;
    }
    assert!(wrap_pi(tps.camera.yaw - 1.2).abs() < 0.01, "behind the hero: yaw {}", tps.camera.yaw);
    assert!((tps.camera.pitch - OrbitCamera::default().pitch).abs() < 0.01, "and back to the usual height: {}", tps.camera.pitch);
    assert!(first.is_some_and(|first| first < 0.002), "it starts gently: {first:?}");
    assert!(biggest < 0.03, "and never jumps: {biggest} rad in one step");
    assert_eq!(tps.hero.facing, 1.2, "the hero does not turn");
}

#[test]
fn the_short_way_round_and_the_picture_moves_between_steps_too() {
    // Facing just left of straight behind the camera's back: it turns left, not 350° right.
    let mut tps = standing(-0.4);
    tps.camera.yaw = 6.0;
    run(&mut tps, Input::default(), camera::RECENTER_AFTER + 0.5);
    let before = tps.camera.yaw;
    assert!(wrap_pi(before - 6.0) < 0.0 && wrap_pi(before - 6.0) > -0.2, "turning left: {before}");
    // Halfway to the next step, the camera is drawn halfway between the two yaws.
    let eye = |tps: &Tps| tps.frame().eye;
    let start = eye(&tps);
    tps.input = Input::default();
    tps.advance(STEP_SECS * 1.5);
    let mut whole = tps.clone();
    whole.advance(STEP_SECS * 0.5);
    let (now, next) = (eye(&tps), eye(&whole));
    assert!(now != start && now != next, "drawn between the steps: {start} {now} {next}");
}

#[test]
fn any_camera_input_restarts_the_wait() {
    let mut tps = standing(2.0);
    for _ in 0..6 {
        run(&mut tps, Input::default(), camera::RECENTER_AFTER - 0.5);
        step(&mut tps, Input { look: [0.001, 0.0], ..Input::default() });
    }
    assert!((tps.camera.yaw - 0.006).abs() < 1e-4, "only the player turned it: {}", tps.camera.yaw);
}

#[test]
fn it_never_turns_while_the_player_steers_and_waits_again_after() {
    let mut tps = standing(0.0);
    // Sideways for a long time: the hero runs to the right of the screen, the camera stays put
    // (turning it would turn the steering with it, and the hero would run in circles).
    let right = Input { strafe: 1.0, ..Input::default() };
    run(&mut tps, right, camera::RECENTER_AFTER * 3.0);
    assert_eq!(tps.camera.yaw, 0.0);
    assert!((tps.hero.facing - std::f32::consts::FRAC_PI_2).abs() < 0.05, "the hero faces right: {}", tps.hero.facing);
    // Let go: it waits again, then goes behind the hero.
    run(&mut tps, Input::default(), camera::RECENTER_AFTER - 0.2);
    assert_eq!(tps.camera.yaw, 0.0);
    run(&mut tps, Input::default(), 5.0);
    assert!(wrap_pi(tps.camera.yaw - tps.hero.facing).abs() < 0.01);
}

/// The hero just south of the tower base, which is right behind a camera looking south.
fn by_the_tower() -> (Tps, Solid) {
    let mut tps = Tps::new();
    let at = Vec3::new(-10.0, 0.0, 6.0);
    let base = tps
        .world()
        .solids()
        .iter()
        .copied()
        .filter(|s| s.max.x - s.min.x > 4.0 && s.max.y - s.min.y > 1.4)
        .min_by(|a, b| (((a.min + a.max) * 0.5) - at).length().total_cmp(&(((b.min + b.max) * 0.5) - at).length()));
    let Some(base) = base else { panic!("no tower base") };
    let x = (base.min.x + base.max.x) * 0.5;
    let z = base.max.z + 1.0;
    tps.hero = Hero::at(Vec3::new(x, tps.world().ground_height(x, z), z));
    tps.camera.yaw = std::f32::consts::PI;
    tps.camera.pitch = -0.1;
    (tps, base)
}

#[test]
fn something_in_the_way_pulls_the_camera_in_at_once_and_it_eases_back_out() {
    let (mut tps, base) = by_the_tower();
    step(&mut tps, Input::default());
    let frame = tps.frame();
    assert!(frame.eye.z > base.max.z && (frame.eye - frame.target).length() < 4.4, "pulled in at once");
    let near = (frame.eye - frame.target).length();
    // Turn round (as the mouse would): nothing in the way now.
    tps.camera.turn([std::f32::consts::PI, 0.0]);
    let mut last = near;
    for index in 0..(1.5 * 60.0) as usize {
        step(&mut tps, Input::default());
        let frame = tps.frame();
        let distance = (frame.eye - frame.target).length();
        assert!(distance + 1e-4 >= last, "it never comes back in: {distance} after {last}");
        if index == 1 {
            // (The step before is drawn as it was: the picture runs between steps.)
            assert!(distance < near + 0.5, "not all at once: {near} → {distance}");
        }
        last = distance;
    }
    assert!((last - 4.5).abs() < 0.02, "all the way out: {last}");
}

#[test]
fn a_jump_moves_the_hero_up_the_screen_and_the_camera_catches_up_after() {
    let mut tps = Tps::new();
    run(&mut tps, Input::default(), 0.2);
    let gap = |tps: &Tps| {
        let frame = tps.frame();
        frame.target.y - (frame.hero.position.y + 1.0)
    };
    assert_eq!(gap(&tps), 0.0);
    step(&mut tps, Input { jump: true, ..Input::default() });
    let mut lowest = 0.0_f32;
    for _ in 0..(1.5 * 60.0) as usize {
        step(&mut tps, Input::default());
        lowest = lowest.min(gap(&tps));
        assert!(gap(&tps).abs() <= 1.0 + 1e-4);
    }
    assert!(lowest < -0.15, "the camera rises less than the hero: {lowest}");
    assert!(tps.hero.grounded);
    assert!(gap(&tps).abs() < 0.01, "and has caught up: {}", gap(&tps));
}

#[test]
fn a_long_fall_keeps_the_hero_near_the_middle_of_the_screen() {
    let mut tps = Tps::new();
    let spawn = tps.hero.position;
    tps.hero = Hero::at(spawn + Vec3::Y * 25.0);
    tps.hero.grounded = false;
    for _ in 0..(1.5 * 60.0) as usize {
        step(&mut tps, Input::default());
        let frame = tps.frame();
        let gap = frame.target.y - (frame.hero.position.y + 1.0);
        assert!(gap.abs() <= 1.0 + 1e-4, "at most a metre off: {gap}");
    }
}

#[test]
fn walking_up_and_down_hills_keeps_the_hero_dead_centre() {
    let mut tps = Tps::new();
    for yaw in [0.0, 1.5, 3.0, 4.5] {
        tps.camera.yaw = yaw;
        for _ in 0..90 {
            step(&mut tps, Input { forward: 1.0, run: true, ..Input::default() });
            if tps.hero.grounded && tps.hero.moves.action == Action::Free {
                let frame = tps.frame();
                assert!((frame.target - (frame.hero.position + Vec3::Y)).length() < 1e-4);
            }
        }
    }
}

/// A tree standing at `foot` (scale 1).
fn tree(kind: PropKind, foot: Vec3) -> Prop {
    Prop { kind, position: foot, yaw: 0.4, scale: 1.0 }
}

#[test]
fn a_crown_between_the_camera_and_the_hero_fades_and_one_beside_does_not() {
    let target = Vec3::new(0.0, 1.0, 0.0);
    let eye = Vec3::new(0.0, 2.5, 4.5);
    for kind in [PropKind::Pine, PropKind::Oak] {
        let Some(crown) = kind.crown() else { panic!("{kind:?} has a crown") };
        // Right on the line, its crown at the line's height: gone (as far as it goes).
        let middle = (crown.low + crown.high) * 0.5;
        let on_line = tree(kind, Vec3::new(0.0, 1.8 - middle, 2.25));
        assert_eq!(on_line.crown_in_the_way(eye, target), 1.0, "{kind:?}");
        // Well to the side, behind the hero, or with the camera above the crown looking past it: kept.
        for foot in [Vec3::new(4.0, 0.0, 2.25), Vec3::new(0.0, 0.0, -4.0), Vec3::new(-3.5, -1.0, 1.0)] {
            assert_eq!(tree(kind, foot).crown_in_the_way(eye, target), 0.0, "{kind:?} at {foot}");
        }
        // The camera inside the crown: gone.
        let around_eye = tree(kind, eye - Vec3::Y * middle);
        assert_eq!(around_eye.crown_in_the_way(eye, target), 1.0, "{kind:?}");
        // Sliding a tree across the line fades it in and out smoothly, never jumping.
        let mut last = 0.0_f32;
        for index in 0..=400 {
            let x = -4.0 + index as f32 * 0.02;
            let fade = tree(kind, Vec3::new(x, 1.8 - middle, 2.25)).crown_in_the_way(eye, target);
            assert!((fade - last).abs() < 0.08, "{kind:?} jumps at x {x}: {last} → {fade}");
            last = fade;
        }
    }
    // Only trees have a crown.
    assert!(PropKind::ALL.iter().filter(|kind| kind.crown().is_some()).eq([PropKind::Pine, PropKind::Oak].iter()));
    let rock = Prop { kind: PropKind::Rock, position: Vec3::new(0.0, 0.0, 2.25), yaw: 0.0, scale: 3.0 };
    assert_eq!(rock.crown_in_the_way(eye, target), 0.0);
}

#[test]
fn junk_never_fades_a_crown() {
    let pine = tree(PropKind::Pine, Vec3::ZERO);
    let nan = Vec3::splat(f32::NAN);
    assert_eq!(pine.crown_in_the_way(nan, Vec3::Y), 0.0);
    assert_eq!(pine.crown_in_the_way(Vec3::Y, Vec3::INFINITY), 0.0);
    let broken = Prop { scale: f32::NAN, ..pine };
    assert_eq!(broken.crown_in_the_way(Vec3::new(0.0, 2.0, 3.0), Vec3::Y), 0.0);
    let flat = Prop { scale: 0.0, ..pine };
    assert_eq!(flat.crown_in_the_way(Vec3::new(0.0, 2.0, 3.0), Vec3::Y), 0.0);
    // The same point for eye and target: the crown around it fades.
    assert_eq!(pine.crown_in_the_way(Vec3::Y * 2.5, Vec3::Y * 2.5), 1.0);
}
