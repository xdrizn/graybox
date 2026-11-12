//! The hero's animation (G11.3): clips, blends, crossfades, the preview.
use super::rig::{BONES, Bone, MAX_BONES, Pose, lowest_sole};
use super::*;
use crate::STEP_SECS;

const DT: f32 = STEP_SECS as f32;

fn drive(speed: f32, grounded: bool) -> Drive {
    Drive { velocity: Vec3::new(0.0, 0.0, -speed), facing: 0.0, grounded, crouch: false }
}

/// Step `animator` for `seconds` with `drive` held, `dt` a step; returns the largest bone turn and
/// pelvis move between two steps.
fn run(animator: &mut Animator, drive: Drive, seconds: f32, dt: f32) -> (f32, f32) {
    let mut worst = (0.0_f32, 0.0_f32);
    let steps = (seconds / dt).round() as u32;
    let mut last = animator.look(1.0).pose;
    for _ in 0..steps {
        animator.step(&drive, dt);
        let now = animator.look(1.0).pose;
        let (angle, root) = now.distance(&last);
        worst = (worst.0.max(angle), worst.1.max(root));
        last = now;
    }
    worst
}

#[test]
fn the_rig_and_every_clip_stay_within_limits() {
    const { assert!(BONES <= MAX_BONES, "too many bones") };
    assert_eq!(ClipId::ALL.len(), 26);
    for id in ClipId::ALL {
        let clip = clip(id);
        assert_eq!(clip.id, id, "the library holds {id:?} in its place");
        assert!(clip.duration > 0.05 && clip.duration < 5.0, "{id:?} lasts {}", clip.duration);
        assert!((0.02..=0.4).contains(&clip.fade_in) && (0.02..=0.4).contains(&clip.fade_out), "{id:?} fades");
        assert!(clip.keys.len() >= 2, "{id:?} has keys");
        assert!(clip.keys.first().is_some_and(|key| key.time == 0.0), "{id:?} starts at 0");
        assert!(clip.keys.windows(2).all(|pair| pair[0].time < pair[1].time), "{id:?} keys in order");
        let last = clip.keys.last().map_or(0.0, |key| key.time);
        match clip.mode {
            Mode::Loop => assert!(last < clip.duration, "{id:?}: a loop's last key runs into the first"),
            Mode::Once | Mode::Hold => assert!((last - clip.duration).abs() < 1e-4, "{id:?} ends on its last key"),
        }
        for key in &clip.keys {
            assert!(key.root.is_finite() && key.root.length() < 1.2, "{id:?} root {:?}", key.root);
            assert!(key.angles.iter().all(|a| a.is_finite() && a.abs().max_element() <= 400.0), "{id:?} angles");
        }
        if let Some(at) = clip.swap_at {
            assert!(at > 0.0 && at < clip.duration);
        }
        assert_eq!(ClipId::from_name(id.name()), Some(id));
    }
}

#[test]
fn each_clip_loops_or_ends_as_declared() {
    for id in ClipId::ALL {
        let clip = clip(id);
        match clip.mode {
            Mode::Loop => {
                // No seam where the end runs into the start.
                let (angle, root) = clip.sample(clip.duration - 1e-4).distance(&clip.sample(0.0));
                assert!(angle < 0.01 && root < 0.001, "{id:?} seam: {angle} rad, {root} m");
                let (angle, _) = clip.sample(clip.duration + 0.3).distance(&clip.sample(0.3));
                assert!(angle < 1e-3, "{id:?} repeats");
            }
            Mode::Once | Mode::Hold => {
                // It stays on its last pose.
                let (angle, _) = clip.sample(clip.duration + 1.0).distance(&clip.sample(clip.duration));
                assert!(angle < 1e-4, "{id:?} holds its end");
            }
        }
        // Played on the animator: a `Once` clip gives way to moving again, `Hold` and `Loop` stay.
        let mut animator = Animator::new();
        run(&mut animator, drive(0.0, true), 0.5, DT);
        animator.play(id);
        run(&mut animator, drive(0.0, true), 0.1, DT);
        let shows = animator.current();
        assert!(shows == id || clip.layer == Layer::Over, "{id:?} shows at once (showing {shows:?})");
        run(&mut animator, drive(0.0, true), clip.duration * 2.0 + 0.6, DT);
        match clip.mode {
            Mode::Once => assert_eq!(animator.current(), ClipId::Idle, "{id:?} ended"),
            Mode::Hold | Mode::Loop => assert_eq!(animator.current(), id, "{id:?} stays"),
        }
    }
}

#[test]
fn locomotion_weights_are_never_negative_and_sum_to_one() {
    for crouch in [0.0, 0.25, 0.5, 0.99, 1.0, -1.0, 7.0, f32::NAN] {
        for tenth in 0..120 {
            let speed = tenth as f32 * 0.1;
            let weights = locomotion_weights(speed, crouch);
            assert!(weights.iter().all(|w| *w >= 0.0 && w.is_finite()), "{speed} {crouch}: {weights:?}");
            let sum: f32 = weights.iter().sum();
            assert!((sum - 1.0).abs() < 1e-5, "{speed} m/s, crouch {crouch}: sum {sum}");
        }
    }
    assert_eq!(locomotion_weights(f32::NAN, 0.0)[0], 1.0, "junk speed stands still");
    // The clips each speed is made for weigh fully at it.
    assert_eq!(locomotion_weights(0.0, 0.0), [1.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
    assert_eq!(locomotion_weights(1.6, 0.0)[1], 1.0);
    assert_eq!(locomotion_weights(5.0, 0.0)[2], 1.0);
    assert_eq!(locomotion_weights(8.5, 0.0)[3], 1.0);
    assert_eq!(locomotion_weights(12.0, 1.0)[5], 1.0);
}

#[test]
fn feet_stay_on_the_ground_while_standing_and_walking() {
    // A foot touches the ground all through these clips (a jog and a run have a moment in the air).
    for (id, air, dip) in [
        (ClipId::Idle, 0.0, 0.035),
        (ClipId::Walk, 0.0, 0.035),
        (ClipId::CrouchIdle, 0.0, 0.035),
        (ClipId::CrouchWalk, 0.0, 0.035),
        (ClipId::Jog, 0.1, 0.045),
        (ClipId::Run, 0.14, 0.045),
    ] {
        let clip = clip(id);
        let mut lowest = f32::MAX;
        for step in 0..60 {
            let time = clip.duration * step as f32 / 60.0;
            let sole = lowest_sole(&clip.sample(time).model_matrices());
            assert!(sole > -dip && sole < 0.035 + air, "{id:?} at {time}: sole at {sole}");
            lowest = lowest.min(sole);
        }
        assert!(lowest.abs() < dip, "{id:?} touches the ground ({lowest})");
    }
}

/// Every change of clip is continuous: with a step 40 times shorter than the game's, no bone may
/// turn more than a few degrees and the pelvis may not move more than a centimetre in one step.
/// (A cut from one pose to another would show as one big step whatever the step length.)
#[test]
fn crossfades_are_continuous() {
    let fine = DT / 40.0;
    let ok = |(angle, root): (f32, f32), what: &str| {
        assert!(angle < 0.06, "{what}: a bone turned {angle} rad in one fine step");
        assert!(root < 0.012, "{what}: the pelvis moved {root} m in one fine step");
    };
    // Every clip played from standing, from jogging and in the middle of another crossfade.
    for id in ClipId::ALL {
        for speed in [0.0, 5.0] {
            let mut animator = Animator::new();
            run(&mut animator, drive(speed, true), 0.4, DT);
            let mut last = animator.look(1.0).pose;
            animator.play(id);
            animator.step(&drive(speed, true), fine);
            let (angle, root) = animator.look(1.0).pose.distance(&last);
            ok((angle, root), &format!("{id:?} from {speed} m/s, first step"));
            ok(run(&mut animator, drive(speed, true), 0.3, fine), &format!("{id:?} from {speed} m/s"));
            last = animator.look(1.0).pose;
            animator.play(ClipId::Sword2);
            animator.step(&drive(speed, true), fine);
            ok(animator.look(1.0).pose.distance(&last), &format!("{id:?} interrupted"));
            ok(run(&mut animator, drive(speed, true), clip(id).duration + 1.0, fine), &format!("{id:?} to its end"));
        }
    }
    // Jump, fall, land; crouch and stand; stop.
    let mut animator = Animator::new();
    ok(run(&mut animator, drive(5.0, true), 0.5, fine), "jog");
    let jump = Drive { velocity: Vec3::new(0.0, 7.0, -5.0), ..drive(5.0, false) };
    ok(run(&mut animator, jump, 0.2, fine), "take off");
    let falling = Drive { velocity: Vec3::new(0.0, -9.0, -5.0), ..drive(5.0, false) };
    ok(run(&mut animator, falling, 0.6, fine), "fall");
    ok(run(&mut animator, drive(5.0, true), 0.8, fine), "land");
    ok(run(&mut animator, Drive { crouch: true, ..drive(2.0, true) }, 0.6, fine), "crouch");
    ok(run(&mut animator, drive(0.0, true), 0.6, fine), "stand still");
    animator.play(ClipId::BowDraw);
    ok(run(&mut animator, drive(0.0, true), 0.8, fine), "aim");
    animator.stop();
    ok(run(&mut animator, drive(0.0, true), 0.5, fine), "stop aiming");
}

#[test]
fn a_crossfade_starts_from_the_pose_shown_not_from_the_new_clip() {
    // Without a crossfade a knockdown would throw the body back at once; with it the first steps
    // barely differ from standing.
    let mut animator = Animator::new();
    run(&mut animator, drive(0.0, true), 1.0, DT);
    let before = animator.look(1.0).pose;
    let jump = clip(ClipId::Knockdown).sample(0.05).distance(&before).0;
    animator.play(ClipId::Knockdown);
    animator.step(&drive(0.0, true), DT);
    let shown = animator.look(1.0).pose.distance(&before).0;
    assert!(shown < jump * 0.3, "first step moved {shown} rad (a cut would move {jump})");
}

#[test]
fn speed_blends_the_gait_and_turning_leans_the_body() {
    let mut animator = Animator::new();
    for (speed, want) in [(0.0, ClipId::Idle), (1.6, ClipId::Walk), (5.0, ClipId::Jog), (8.5, ClipId::Run)] {
        run(&mut animator, drive(speed, true), 1.0, DT);
        assert_eq!(animator.current(), want, "{speed} m/s");
    }
    run(&mut animator, Drive { crouch: true, ..drive(0.0, true) }, 1.0, DT);
    assert_eq!(animator.current(), ClipId::CrouchIdle);
    let crouched = animator.look(1.0).pose.root.y;
    assert!(crouched < -0.2, "crouching lowers the body: {crouched}");
    run(&mut animator, Drive { crouch: true, ..drive(1.9, true) }, 1.0, DT);
    assert_eq!(animator.current(), ClipId::CrouchWalk);
    // Running in a circle to the right leans to the right (the up axis tips towards +X).
    let mut animator = Animator::new();
    let mut facing = 0.0_f32;
    for _ in 0..90 {
        facing += 2.0 * DT;
        animator.step(&Drive { facing, ..drive(8.0, true) }, DT);
    }
    let up = animator.look(1.0).tilt * Vec3::Y;
    assert!(up.x > 0.1, "leans into the turn: {up:?}");
    // Straight on again, it comes back upright.
    for _ in 0..90 {
        animator.step(&Drive { facing, ..drive(8.0, true) }, DT);
    }
    assert!((animator.look(1.0).tilt * Vec3::Y).x.abs() < 0.02);
}

#[test]
fn the_air_and_landing_follow_the_vertical_speed() {
    let mut animator = Animator::new();
    run(&mut animator, drive(0.0, true), 0.3, DT);
    run(&mut animator, Drive { velocity: Vec3::new(0.0, 6.0, 0.0), ..drive(0.0, false) }, 0.1, DT);
    assert_eq!(animator.current(), ClipId::JumpUp);
    run(&mut animator, Drive { velocity: Vec3::new(0.0, -6.0, 0.0), ..drive(0.0, false) }, 0.3, DT);
    assert_eq!(animator.current(), ClipId::Fall);
    run(&mut animator, drive(0.0, true), 0.05, DT);
    assert_eq!(animator.current(), ClipId::Land, "a hard landing shows");
    run(&mut animator, drive(0.0, true), 1.0, DT);
    assert_eq!(animator.current(), ClipId::Idle);
    // Stepping off a small ledge (no jump) only falls after a moment.
    run(&mut animator, drive(0.0, false), 0.1, DT);
    assert_eq!(animator.current(), ClipId::Idle);
    run(&mut animator, drive(0.0, false), 0.2, DT);
    assert_eq!(animator.current(), ClipId::Fall);
}

#[test]
fn the_cloak_flares_with_speed_and_hangs_when_still() {
    let cloak_back = |animator: &Animator| {
        let m = animator.look(1.0).pose.model_matrices();
        // How far back the hem is, from the chest.
        m[Bone::CloakLower.index()].transform_point3(Vec3::new(0.0, -0.4, 0.0)).z - m[Bone::Chest.index()].transform_point3(Vec3::ZERO).z
    };
    let mut animator = Animator::new();
    run(&mut animator, drive(0.0, true), 2.0, DT);
    let still = cloak_back(&animator);
    run(&mut animator, drive(8.5, true), 2.0, DT);
    let running = cloak_back(&animator);
    assert!(running > still + 0.2, "the cloak flies back when running: {still} → {running}");
}

#[test]
fn drawing_and_sheathing_move_the_weapon_between_back_and_hand() {
    let mut animator = Animator::new();
    animator.set_weapon(Weapon::Spear, false);
    animator.play(ClipId::WeaponDraw);
    run(&mut animator, drive(0.0, true), 0.15, DT);
    assert!(!animator.in_hand(), "still reaching");
    run(&mut animator, drive(0.0, true), 0.2, DT);
    assert!(animator.in_hand(), "drawn");
    animator.play(ClipId::WeaponSheathe);
    run(&mut animator, drive(0.0, true), 0.6, DT);
    assert!(!animator.in_hand(), "back on the back");
    assert_eq!(animator.weapon(), Weapon::Spear);
    // The bow is drawn with the left hand: the same clip, mirrored.
    animator.set_weapon(Weapon::Bow, false);
    animator.play(ClipId::WeaponDraw);
    run(&mut animator, drive(0.0, true), 0.2, DT);
    let m = animator.look(1.0).pose.model_matrices();
    let left = m[Bone::HandL.index()].transform_point3(Vec3::ZERO).y;
    let right = m[Bone::HandR.index()].transform_point3(Vec3::ZERO).y;
    assert!(left > right + 0.3, "the left hand reaches over the shoulder: {left} vs {right}");
}

#[test]
fn the_pose_is_drawn_between_two_steps() {
    let mut animator = Animator::new();
    run(&mut animator, drive(8.5, true), 0.5, DT);
    let (a, b, half) = (animator.look(0.0).pose, animator.look(1.0).pose, animator.look(0.5).pose);
    let (whole, _) = a.distance(&b);
    assert!(whole > 0.01, "moving");
    let (to_a, _) = half.distance(&a);
    let (to_b, _) = half.distance(&b);
    assert!(to_a < whole && to_b < whole && (to_a + to_b - whole).abs() < whole * 0.2, "{to_a} + {to_b} ≈ {whole}");
    assert_eq!(animator.look(f32::NAN).pose, b);
}

#[test]
fn the_preview_shows_each_clip_on_a_still_hero() {
    let mut tps = Tps::new();
    let start = tps.hero.position;
    for id in ClipId::ALL {
        tps.preview_clip(Some(id));
        tps.input = Input { forward: 1.0, jump: true, ..Input::default() };
        for _ in 0..20 {
            tps.advance(STEP_SECS);
        }
        assert_eq!(tps.hero.animator.previewing(), Some(id));
        let shows = tps.frame().hero.clip;
        assert!(shows == id || matches!(id, ClipId::Land | ClipId::HitReact), "{id:?} shows ({shows:?})");
        assert!((tps.hero.position - start).length() < 1e-3, "{id:?}: the hero stood still");
    }
    // A clip that ends plays again.
    tps.preview_clip(Some(ClipId::Sword1));
    for _ in 0..((clip(ClipId::Sword1).duration + 1.0) / DT) as u32 {
        tps.advance(STEP_SECS);
    }
    assert_eq!(tps.frame().hero.clip, ClipId::Sword1);
    assert!(tps.frame().hero.in_hand && tps.frame().hero.weapon == Weapon::Sword);
    tps.preview_clip(None);
    tps.input = Input { forward: 1.0, ..Input::default() };
    for _ in 0..30 {
        tps.advance(STEP_SECS);
    }
    assert!((tps.hero.position - start).length() > 1.0, "moves again after the preview");
}

#[test]
fn junk_never_breaks_the_animator() {
    let mut animator = Animator::new();
    for junk in [f32::NAN, f32::INFINITY, -f32::INFINITY, 1e30] {
        animator.step(&Drive { velocity: Vec3::splat(junk), facing: junk, grounded: false, crouch: true }, DT);
        animator.step(&drive(junk, true), junk);
        animator.set_rate(junk);
        let look = animator.look(junk);
        assert!(look.pose.bones.iter().all(|q| q.is_finite()) && look.pose.root.is_finite() && look.tilt.is_finite());
    }
    run(&mut animator, drive(3.0, true), 1.0, DT);
    let pose: Pose = animator.look(1.0).pose;
    assert!(pose.bones.iter().all(|q| q.is_finite() && (q.length() - 1.0).abs() < 1e-3));
}
