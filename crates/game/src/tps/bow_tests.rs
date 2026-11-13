//! G11.6: the bow. A click shoots from the hip; holding aim brings the camera over the shoulder with
//! the crosshair in the middle of the screen, and a click there draws and shoots where it points;
//! the shoulder button swaps sides. Arrows go where the crosshair is.
use super::*;
use crate::state::Input;

fn run(tps: &mut Tps, input: Input, seconds: f32) {
    for _ in 0..(seconds * 60.0).round() as usize {
        tps.input = input;
        tps.advance(STEP_SECS);
    }
}

const AIM: Input = Input {
    forward: 0.0,
    strafe: 0.0,
    look: [0.0; 2],
    run: false,
    crouch: false,
    jump: false,
    shoot: false,
    reload: false,
    heal: false,
    respawn: false,
    roll: false,
    weapon: 0,
    aim: true,
    shoulder: false,
};

/// A game with one dummy `distance` metres in front of the hero, the bow in hand.
fn with_bow(distance: f32) -> Tps {
    let mut tps = Tps::new();
    tps.foes.retain(|foe| foe.kind == FoeKind::Dummy);
    tps.foes.truncate(1);
    let foe = tps.foes[0].position;
    let at = foe + Vec3::new(0.0, 0.0, distance);
    tps.hero = Hero::at(Vec3::new(at.x, tps.world().ground_height(at.x, at.z), at.z));
    tps.hero.facing = 0.0;
    tps.camera.yaw = 0.0;
    // Level, a touch up: the crosshair looks over the grass, not into it.
    tps.camera.pitch = 0.05;
    run(&mut tps, Input { weapon: 4, ..Input::default() }, 1.0 / 60.0);
    run(&mut tps, Input::default(), 0.8);
    assert_eq!(tps.hero.animator.weapon(), Weapon::Bow);
    tps
}

/// Where the middle of the screen looks: from the eye through the camera's target.
fn crosshair(tps: &Tps) -> (Vec3, Vec3) {
    let frame = tps.frame();
    (frame.eye, (frame.target - frame.eye).normalize())
}

/// Stand the dummy on the crosshair, `metres` past the camera's target.
fn dummy_on_crosshair(tps: &mut Tps, metres: f32) {
    let frame = tps.frame();
    let (eye, direction) = crosshair(tps);
    let point = eye + direction * ((frame.target - eye).length() + metres);
    let feet = point - Vec3::Y * 0.9;
    let foe = &mut tps.foes[0];
    foe.home = feet;
    foe.position = feet;
    foe.previous_position = feet;
}

/// How far right of the middle of the screen `point` is (metres along the camera's right).
fn right_of_centre(tps: &Tps, point: Vec3) -> f32 {
    let (eye, direction) = crosshair(tps);
    let right = direction.cross(Vec3::Y).normalize();
    (point - eye).dot(right)
}

fn click(tps: &mut Tps, held: Input) {
    run(tps, Input { shoot: true, ..held }, 1.0 / 60.0);
    run(tps, held, 1.0 / 60.0);
}

#[test]
fn a_click_with_the_bow_shoots_from_the_hip_at_what_the_screen_centre_looks_at() {
    let mut tps = with_bow(10.0);
    // Turn away a little: the hip shot still goes where the camera looks, and the hero turns to it.
    tps.camera.yaw = 0.3;
    run(&mut tps, Input::default(), 1.0 / 60.0);
    dummy_on_crosshair(&mut tps, 8.0);
    let full = tps.foes[0].health;
    tps.drain_events();
    click(&mut tps, Input::default());
    assert!(!tps.hero.combat.aiming(), "a click alone does not aim");
    run(&mut tps, Input::default(), 0.3);
    assert_eq!(tps.arrows.len(), 1, "a quick shot");
    assert!(tps.drain_events().contains(&GameEvent::Shot));
    assert!((hero_yaw_gap(&tps)).abs() < 0.1, "the hero turned to the shot: {}", hero_yaw_gap(&tps));
    run(&mut tps, Input::default(), 1.0);
    assert_eq!(tps.foes[0].health, full - ARROW_DAMAGE, "the arrow hit what the screen centre looked at");
}

fn hero_yaw_gap(tps: &Tps) -> f32 {
    let gap = (tps.hero.facing - tps.camera.yaw).rem_euclid(std::f32::consts::TAU);
    if gap > std::f32::consts::PI { gap - std::f32::consts::TAU } else { gap }
}

#[test]
fn holding_aim_moves_the_camera_over_the_shoulder_with_the_hero_on_the_left() {
    let mut tps = with_bow(10.0);
    let behind = tps.frame();
    run(&mut tps, AIM, 1.0 / 60.0);
    assert!(tps.hero.combat.aiming(), "aim mode");
    // Smooth: one step in, the camera has only started to move.
    let blend = tps.camera.aim_blend();
    assert!(blend > 0.0 && blend < 0.5, "eases in: {blend}");
    run(&mut tps, AIM, 0.6);
    assert!(tps.camera.aim_blend() > 0.95);
    let aimed = tps.frame();
    assert!((aimed.eye - aimed.target).length() < (behind.eye - behind.target).length() - 1.5, "closer");
    let hero_middle = tps.hero.position + Vec3::Y * 1.2;
    assert!(right_of_centre(&tps, hero_middle) < -0.3, "the hero is on the left: {}", right_of_centre(&tps, hero_middle));
    // Aiming walks, facing where the camera looks.
    tps.camera.yaw = 0.8;
    run(&mut tps, Input { forward: 1.0, run: true, ..AIM }, 1.0);
    assert!(hero_yaw_gap(&tps).abs() < 0.1);
    assert!(tps.hero.velocity.length() < 2.5, "walking pace: {}", tps.hero.velocity.length());
}

#[test]
fn a_click_in_aim_mode_draws_then_shoots_at_the_crosshair() {
    let mut tps = with_bow(10.0);
    run(&mut tps, AIM, 0.8);
    dummy_on_crosshair(&mut tps, 14.0);
    let full = tps.foes[0].health;
    click(&mut tps, AIM);
    assert!(tps.arrows.is_empty(), "the string is drawn first");
    run(&mut tps, AIM, 0.4);
    assert_eq!(tps.arrows.len(), 1, "then the arrow goes");
    run(&mut tps, AIM, 1.0);
    assert_eq!(tps.foes[0].health, full - ARROW_DAMAGE, "it hit what the crosshair was on");
    // Another click shoots again, still aiming.
    click(&mut tps, AIM);
    run(&mut tps, AIM, 1.2);
    assert!(tps.hero.combat.aiming());
    assert!(tps.foes[0].health < full - ARROW_DAMAGE, "hit again: {}", tps.foes[0].health);
}

#[test]
fn a_far_target_under_the_crosshair_is_hit_despite_the_arrow_dropping() {
    let mut tps = with_bow(10.0);
    run(&mut tps, AIM, 0.8);
    dummy_on_crosshair(&mut tps, 28.0);
    let full = tps.foes[0].health;
    click(&mut tps, AIM);
    run(&mut tps, AIM, 2.0);
    assert_eq!(tps.foes[0].health, full - ARROW_DAMAGE);
}

#[test]
fn letting_go_of_aim_leaves_aim_mode_and_the_camera_eases_back() {
    let mut tps = with_bow(10.0);
    run(&mut tps, AIM, 0.8);
    // Mid-draw: letting go cancels the shot.
    click(&mut tps, AIM);
    run(&mut tps, Input::default(), 1.0 / 60.0);
    assert!(!tps.hero.combat.aiming());
    let blend = tps.camera.aim_blend();
    assert!(blend < 1.0 && blend > 0.5, "eases out: {blend}");
    run(&mut tps, Input::default(), 1.0);
    assert!(tps.camera.aim_blend() < 0.02);
    assert!(tps.arrows.is_empty(), "no arrow left once aim was let go mid-draw");
}

#[test]
fn the_shoulder_button_swaps_the_camera_to_the_other_side_smoothly() {
    let mut tps = with_bow(10.0);
    run(&mut tps, AIM, 0.8);
    let hero_middle = |tps: &Tps| tps.hero.position + Vec3::Y * 1.2;
    let left = right_of_centre(&tps, hero_middle(&tps));
    run(&mut tps, Input { shoulder: true, ..AIM }, 1.0 / 60.0);
    run(&mut tps, AIM, 1.0 / 60.0);
    let moving = right_of_centre(&tps, hero_middle(&tps));
    assert!(moving > left && moving < 0.0, "on its way: {left} → {moving}");
    run(&mut tps, AIM, 0.8);
    assert!(right_of_centre(&tps, hero_middle(&tps)) > 0.3, "the hero is on the right now");
    // Pressed again, back to the left.
    run(&mut tps, Input { shoulder: true, ..AIM }, 1.0 / 60.0);
    run(&mut tps, AIM, 0.8);
    assert!(right_of_centre(&tps, hero_middle(&tps)) < -0.3);
}

#[test]
fn aim_does_nothing_without_the_bow() {
    let mut tps = with_bow(10.0);
    run(&mut tps, Input { weapon: 1, ..Input::default() }, 1.0 / 60.0);
    run(&mut tps, Input::default(), 1.0);
    run(&mut tps, AIM, 0.5);
    assert!(!tps.hero.combat.aiming());
    assert!(tps.camera.aim_blend() < 0.01);
}

#[test]
fn a_roll_ends_aim_mode() {
    let mut tps = with_bow(10.0);
    run(&mut tps, AIM, 0.5);
    run(&mut tps, Input { roll: true, strafe: 1.0, ..AIM }, 1.0 / 60.0);
    run(&mut tps, AIM, 0.1);
    assert!(!tps.hero.combat.aiming(), "rolling, not aiming");
    run(&mut tps, AIM, 1.5);
    assert!(tps.hero.combat.aiming(), "aiming again once the roll is over");
}

#[test]
fn the_crosshair_point_stops_at_the_ground_and_skips_the_hero() {
    let mut tps = Tps::new();
    tps.foes.clear();
    tps.camera.pitch = -0.6;
    let (eye, direction) = crosshair(&tps);
    let point = tps.aim_point();
    assert!((point - eye).normalize().dot(direction) > 0.999, "on the ray");
    assert!((point.y - tps.world().ground_height(point.x, point.z)).abs() < 0.2, "on the ground: {point:?}");
    let hero = tps.hero.position;
    assert!(glam::Vec2::new(point.x - hero.x, point.z - hero.z).length() > 1.0, "past the hero, not on it");
}

#[test]
fn junk_camera_angles_give_a_finite_crosshair_point() {
    let mut tps = Tps::new();
    for (yaw, pitch) in [(f32::NAN, 0.0), (0.0, f32::INFINITY), (1e30, -1e30)] {
        tps.camera.yaw = yaw;
        tps.camera.pitch = pitch;
        run(&mut tps, Input { shoulder: true, ..AIM }, 1.0 / 60.0);
        assert!(tps.aim_point().is_finite());
        assert!(tps.frame().eye.is_finite());
    }
}

#[test]
fn aiming_right_after_a_hip_click_still_shoots_it() {
    let mut tps = with_bow(10.0);
    tps.drain_events();
    click(&mut tps, Input::default());
    run(&mut tps, AIM, 0.5);
    assert!(tps.drain_events().contains(&GameEvent::Shot), "the hip shot went");
    assert!(tps.hero.combat.aiming());
    run(&mut tps, AIM, 0.5);
    assert_eq!(tps.hero.animator.overlay_clip(), Some(ClipId::BowAim), "then the bow comes up nocked");
}
