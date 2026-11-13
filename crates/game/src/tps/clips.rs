//! Every clip the hero has, authored in code (G11.3).
//!
//! Angles are degrees, by the rule in `rig.rs`: limbs that hang swing forward with +X (the right arm
//! lifts sideways with +Z, the left with -Z); pelvis, spine, chest and head bend forward with -X
//! and turn left with +Y; feet lift their toes with +X; knees bend with -X, elbows with +X.
//! Cycles are written as their first half (left foot forward to right foot forward) and mirrored.
//! `plant(lift)` puts the lowest sole `lift` metres above the ground, so feet touch it.
use super::clip::{Clip, ClipId, Ease, Key, Layer, Mode, cycle};
use super::rig::Bone::{
    Chest, FootL, FootR, ForearmL, ForearmR, HandL, HandR, Head, Pelvis, ShinL, ShinR, Spine, ThighL, ThighR, UpperArmL, UpperArmR,
};
use std::sync::OnceLock;

/// The clip `id` (built once).
pub fn clip(id: ClipId) -> &'static Clip {
    static LIBRARY: OnceLock<Vec<Clip>> = OnceLock::new();
    static EMPTY: Clip =
        Clip { id: ClipId::Idle, duration: 1.0, mode: Mode::Loop, layer: Layer::Base, fade_in: 0.2, fade_out: 0.2, swap_at: None, keys: Vec::new() };
    LIBRARY.get_or_init(|| ClipId::ALL.into_iter().map(build).collect()).get(id.index()).filter(|clip| clip.id == id).unwrap_or(&EMPTY)
}

fn build(id: ClipId) -> Clip {
    match id {
        ClipId::Idle => idle(),
        ClipId::Walk => walk(),
        ClipId::Jog => jog(),
        ClipId::Run => run(),
        ClipId::CrouchIdle => crouch_idle(),
        ClipId::CrouchWalk => crouch_walk(),
        ClipId::JumpUp => jump_up(),
        ClipId::Fall => fall(),
        ClipId::Land => land(),
        ClipId::LandRoll => land_roll(),
        ClipId::ClimbLadder => climb_ladder(),
        ClipId::ClimbWall => climb_wall(),
        ClipId::Mantle => mantle(),
        ClipId::Sword1 => sword1(),
        ClipId::Sword2 => sword2(),
        ClipId::Sword3 => sword3(),
        ClipId::SpearThrust => spear_thrust(),
        ClipId::ClubSwing => club_swing(),
        ClipId::BowDraw => bow_draw(),
        ClipId::BowShoot => bow_shoot(),
        ClipId::HitReact => hit_react(),
        ClipId::Knockdown => knockdown(),
        ClipId::GetUp => get_up(),
        ClipId::Die => die(),
        ClipId::WeaponDraw => weapon_draw(),
        ClipId::WeaponSheathe => weapon_sheathe(),
        ClipId::BowAim => bow_ready(),
    }
}

/// Locomotion speeds each cycle is authored for (m/s): the animator plays them faster or slower
/// to match the hero's real speed, so the feet do not slide.
pub const WALK_SPEED: f32 = 1.6;
pub const JOG_SPEED: f32 = 5.0;
pub const RUN_SPEED: f32 = 8.5;
pub const CROUCH_WALK_SPEED: f32 = 1.9;

// ---- Shared sub-poses -------------------------------------------------------------------------

/// Standing legs, knees soft, toes a little out.
fn stand_legs(k: Key) -> Key {
    k.sym(ThighL, [3.0, 6.0, -3.0]).sym(ShinL, [-6.0, 0.0, 0.0]).sym(FootL, [3.0, 0.0, 3.0])
}

/// Arms hanging relaxed, a little away from the body.
fn rest_arms(k: Key) -> Key {
    k.sym(UpperArmL, [3.0, 0.0, -9.0]).sym(ForearmL, [12.0, 0.0, 0.0]).sym(HandL, [0.0, 0.0, 0.0])
}

fn neutral(t: f32) -> Key {
    rest_arms(stand_legs(Key::at(t))).b(Chest, [-1.0, 0.0, 0.0]).b(Head, [2.0, 0.0, 0.0]).plant(0.0)
}

/// Fighting stance: left foot forward, right back and turned out, weapon arm forward.
fn ready_legs(k: Key) -> Key {
    k.b(ThighL, [16.0, 8.0, -6.0])
        .b(ShinL, [-20.0, 0.0, 0.0])
        .b(FootL, [4.0, 0.0, 4.0])
        .b(ThighR, [-8.0, -16.0, 6.0])
        .b(ShinR, [-16.0, 0.0, 0.0])
        .b(FootR, [24.0, 0.0, -4.0])
}

fn ready(t: f32) -> Key {
    ready_legs(Key::at(t))
        .b(Pelvis, [0.0, -6.0, 0.0])
        .b(Spine, [-4.0, 2.0, 0.0])
        .b(Chest, [-2.0, 4.0, 0.0])
        .b(Head, [4.0, 0.0, 0.0])
        .b(UpperArmR, [30.0, 0.0, 14.0])
        .b(ForearmR, [52.0, 0.0, 0.0])
        .b(HandR, [-10.0, 0.0, 0.0])
        .b(UpperArmL, [22.0, 0.0, -20.0])
        .b(ForearmL, [44.0, 0.0, 0.0])
        .plant(0.0)
}

// ---- Locomotion -------------------------------------------------------------------------------

fn idle() -> Clip {
    let breathe_in = |t: f32, shift: f32, look: f32| {
        rest_arms(stand_legs(Key::at(t)))
            .b(Pelvis, [0.0, 0.0, shift])
            .sym(ThighL, [3.0, 6.0, -3.0 - shift])
            .b(Spine, [0.0, 0.0, -shift * 0.6])
            .b(Chest, [1.5, 0.0, 0.0])
            .b(Head, [1.0, look, -shift * 0.4])
            .sym(UpperArmL, [1.0, 0.0, -11.0])
            .root([-shift * 0.006, 0.0, 0.0])
            .plant(0.0)
    };
    let breathe_out = |t: f32, shift: f32, look: f32| {
        rest_arms(stand_legs(Key::at(t)))
            .b(Pelvis, [0.0, 0.0, shift])
            .sym(ThighL, [3.0, 6.0, -3.0 - shift])
            .b(Spine, [0.0, 0.0, -shift * 0.6])
            .b(Chest, [-1.5, 0.0, 0.0])
            .b(Head, [3.0, look, -shift * 0.4])
            .root([-shift * 0.006, 0.0, 0.0])
            .plant(-0.004)
    };
    Clip::new(
        ClipId::Idle,
        4.0,
        Mode::Loop,
        vec![breathe_out(0.0, 1.2, -2.0), breathe_in(1.0, 0.6, 1.0), breathe_out(2.0, -1.2, 4.0), breathe_in(3.0, -0.4, 1.0)],
    )
}

fn walk() -> Clip {
    let half = vec![
        // Left heel strikes, right toes push off.
        Key::at(0.0)
            .b(ThighL, [26.0, 4.0, -2.0])
            .b(ShinL, [-4.0, 0.0, 0.0])
            .b(FootL, [-7.0, 0.0, 2.0])
            .b(ThighR, [-18.0, -4.0, 2.0])
            .b(ShinR, [-22.0, 0.0, 0.0])
            .b(FootR, [5.0, 0.0, -2.0])
            .b(Pelvis, [0.0, -5.0, -1.0])
            .b(Spine, [-3.0, 2.0, 0.0])
            .b(Chest, [-1.0, 5.0, 0.0])
            .b(Head, [3.0, -2.0, 0.0])
            .b(UpperArmL, [-16.0, 0.0, -8.0])
            .b(ForearmL, [14.0, 0.0, 0.0])
            .b(UpperArmR, [20.0, 0.0, 8.0])
            .b(ForearmR, [28.0, 0.0, 0.0])
            .plant(0.0),
        // Weight comes onto the left leg, the knee takes it.
        Key::at(0.12)
            .b(ThighL, [20.0, 3.0, -2.0])
            .b(ShinL, [-14.0, 0.0, 0.0])
            .b(FootL, [-6.0, 0.0, 2.0])
            .b(ThighR, [-8.0, -3.0, 2.0])
            .b(ShinR, [-50.0, 0.0, 0.0])
            .b(FootR, [18.0, 0.0, -2.0])
            .b(Pelvis, [0.0, -3.0, -3.0])
            .b(Spine, [-3.0, 1.0, 1.0])
            .b(Chest, [-1.0, 3.0, 1.0])
            .b(Head, [3.0, -1.0, 0.0])
            .b(UpperArmL, [-12.0, 0.0, -8.0])
            .b(ForearmL, [16.0, 0.0, 0.0])
            .b(UpperArmR, [15.0, 0.0, 8.0])
            .b(ForearmR, [26.0, 0.0, 0.0])
            .plant(0.0),
        // Passing: the right foot swings by, the body at its highest.
        Key::at(0.25)
            .b(ThighL, [2.0, 0.0, -2.0])
            .b(ShinL, [-4.0, 0.0, 0.0])
            .b(FootL, [2.0, 0.0, 2.0])
            .b(ThighR, [18.0, -1.0, 2.0])
            .b(ShinR, [-62.0, 0.0, 0.0])
            .b(FootR, [24.0, 0.0, -2.0])
            .b(Pelvis, [0.0, 0.0, -2.0])
            .b(Spine, [-3.0, 0.0, 0.0])
            .b(Chest, [-1.0, 0.0, 0.0])
            .b(Head, [3.0, 0.0, 0.0])
            .b(UpperArmL, [2.0, 0.0, -8.0])
            .b(ForearmL, [16.0, 0.0, 0.0])
            .b(UpperArmR, [3.0, 0.0, 8.0])
            .b(ForearmR, [18.0, 0.0, 0.0])
            .plant(0.0),
        // The right leg reaches, the left heel starts to lift.
        Key::at(0.37)
            .b(ThighL, [-10.0, -2.0, -2.0])
            .b(ShinL, [-6.0, 0.0, 0.0])
            .b(FootL, [10.0, 0.0, 2.0])
            .b(ThighR, [30.0, -3.0, 2.0])
            .b(ShinR, [-28.0, 0.0, 0.0])
            .b(FootR, [10.0, 0.0, -2.0])
            .b(Pelvis, [0.0, 3.0, -1.0])
            .b(Spine, [-3.0, -1.0, 0.0])
            .b(Chest, [-1.0, -3.0, 0.0])
            .b(Head, [3.0, 1.0, 0.0])
            .b(UpperArmL, [12.0, 0.0, -8.0])
            .b(ForearmL, [24.0, 0.0, 0.0])
            .b(UpperArmR, [-10.0, 0.0, 8.0])
            .b(ForearmR, [16.0, 0.0, 0.0])
            .plant(0.0),
    ];
    Clip::new(ClipId::Walk, 1.0, Mode::Loop, cycle(1.0, half))
}

fn jog() -> Clip {
    let lean = |k: Key, twist: f32| {
        k.b(Pelvis, [0.0, -7.0 * twist, 0.0]).b(Spine, [-6.0, 3.0 * twist, 0.0]).b(Chest, [-4.0, 8.0 * twist, 0.0]).b(Head, [9.0, -4.0 * twist, 0.0])
    };
    let half = vec![
        lean(Key::at(0.0), 1.0)
            .b(ThighL, [32.0, 4.0, -2.0])
            .b(ShinL, [-12.0, 0.0, 0.0])
            .b(FootL, [-6.0, 0.0, 2.0])
            .b(ThighR, [-22.0, -4.0, 2.0])
            .b(ShinR, [-55.0, 0.0, 0.0])
            .b(FootR, [20.0, 0.0, -2.0])
            .b(UpperArmL, [-28.0, 0.0, -10.0])
            .b(ForearmL, [70.0, 0.0, 0.0])
            .b(UpperArmR, [32.0, 0.0, 10.0])
            .b(ForearmR, [85.0, 0.0, 0.0])
            .plant(0.0),
        lean(Key::at(0.09), 0.5)
            .b(ThighL, [16.0, 2.0, -2.0])
            .b(ShinL, [-30.0, 0.0, 0.0])
            .b(FootL, [14.0, 0.0, 2.0])
            .b(ThighR, [8.0, -2.0, 2.0])
            .b(ShinR, [-95.0, 0.0, 0.0])
            .b(FootR, [30.0, 0.0, -2.0])
            .b(Pelvis, [0.0, -3.0, -3.0])
            .b(UpperArmL, [-12.0, 0.0, -10.0])
            .b(ForearmL, [75.0, 0.0, 0.0])
            .b(UpperArmR, [18.0, 0.0, 10.0])
            .b(ForearmR, [80.0, 0.0, 0.0])
            .plant(0.0),
        lean(Key::at(0.19), -0.4)
            .b(ThighL, [-16.0, -2.0, -2.0])
            .b(ShinL, [-14.0, 0.0, 0.0])
            .b(FootL, [-10.0, 0.0, 2.0])
            .b(ThighR, [38.0, -2.0, 2.0])
            .b(ShinR, [-80.0, 0.0, 0.0])
            .b(FootR, [20.0, 0.0, -2.0])
            .b(UpperArmL, [10.0, 0.0, -10.0])
            .b(ForearmL, [80.0, 0.0, 0.0])
            .b(UpperArmR, [-8.0, 0.0, 10.0])
            .b(ForearmR, [72.0, 0.0, 0.0])
            .plant(0.02),
        // In the air: the right leg reaches for the next step.
        lean(Key::at(0.28), -0.9)
            .b(ThighL, [-26.0, -4.0, -2.0])
            .b(ShinL, [-50.0, 0.0, 0.0])
            .b(FootL, [16.0, 0.0, 2.0])
            .b(ThighR, [36.0, -3.0, 2.0])
            .b(ShinR, [-30.0, 0.0, 0.0])
            .b(FootR, [-4.0, 0.0, -2.0])
            .b(UpperArmL, [24.0, 0.0, -10.0])
            .b(ForearmL, [82.0, 0.0, 0.0])
            .b(UpperArmR, [-24.0, 0.0, 10.0])
            .b(ForearmR, [72.0, 0.0, 0.0])
            .plant(0.07),
    ];
    Clip::new(ClipId::Jog, 0.74, Mode::Loop, cycle(0.74, half))
}

fn run() -> Clip {
    let lean = |k: Key, twist: f32| {
        k.b(Pelvis, [0.0, -9.0 * twist, 0.0])
            .b(Spine, [-10.0, 4.0 * twist, 0.0])
            .b(Chest, [-6.0, 10.0 * twist, 0.0])
            .b(Head, [15.0, -5.0 * twist, 0.0])
    };
    let half = vec![
        lean(Key::at(0.0), 1.0)
            .b(ThighL, [40.0, 4.0, -2.0])
            .b(ShinL, [-18.0, 0.0, 0.0])
            .b(FootL, [-8.0, 0.0, 2.0])
            .b(ThighR, [-28.0, -4.0, 2.0])
            .b(ShinR, [-75.0, 0.0, 0.0])
            .b(FootR, [25.0, 0.0, -2.0])
            .b(UpperArmL, [-42.0, 0.0, -12.0])
            .b(ForearmL, [80.0, 0.0, 0.0])
            .b(UpperArmR, [48.0, 0.0, 12.0])
            .b(ForearmR, [95.0, 0.0, 0.0])
            .sym(HandL, [10.0, 0.0, 0.0])
            .plant(0.0),
        lean(Key::at(0.075), 0.5)
            .b(ThighL, [22.0, 2.0, -2.0])
            .b(ShinL, [-42.0, 0.0, 0.0])
            .b(FootL, [20.0, 0.0, 2.0])
            .b(ThighR, [12.0, -2.0, 2.0])
            .b(ShinR, [-120.0, 0.0, 0.0])
            .b(FootR, [40.0, 0.0, -2.0])
            .b(Pelvis, [0.0, -4.0, -4.0])
            .b(UpperArmL, [-20.0, 0.0, -12.0])
            .b(ForearmL, [85.0, 0.0, 0.0])
            .b(UpperArmR, [26.0, 0.0, 12.0])
            .b(ForearmR, [90.0, 0.0, 0.0])
            .sym(HandL, [10.0, 0.0, 0.0])
            .plant(0.0),
        lean(Key::at(0.16), -0.4)
            .b(ThighL, [-20.0, -2.0, -2.0])
            .b(ShinL, [-18.0, 0.0, 0.0])
            .b(FootL, [-14.0, 0.0, 2.0])
            .b(ThighR, [50.0, -2.0, 2.0])
            .b(ShinR, [-95.0, 0.0, 0.0])
            .b(FootR, [25.0, 0.0, -2.0])
            .b(UpperArmL, [16.0, 0.0, -12.0])
            .b(ForearmL, [90.0, 0.0, 0.0])
            .b(UpperArmR, [-14.0, 0.0, 12.0])
            .b(ForearmR, [80.0, 0.0, 0.0])
            .sym(HandL, [10.0, 0.0, 0.0])
            .plant(0.02),
        lean(Key::at(0.235), -0.9)
            .b(ThighL, [-34.0, -4.0, -2.0])
            .b(ShinL, [-70.0, 0.0, 0.0])
            .b(FootL, [20.0, 0.0, 2.0])
            .b(ThighR, [46.0, -3.0, 2.0])
            .b(ShinR, [-40.0, 0.0, 0.0])
            .b(FootR, [-2.0, 0.0, -2.0])
            .b(UpperArmL, [38.0, 0.0, -12.0])
            .b(ForearmL, [92.0, 0.0, 0.0])
            .b(UpperArmR, [-38.0, 0.0, 12.0])
            .b(ForearmR, [78.0, 0.0, 0.0])
            .sym(HandL, [10.0, 0.0, 0.0])
            .plant(0.11),
    ];
    Clip::new(ClipId::Run, 0.62, Mode::Loop, cycle(0.62, half))
}

/// The crouched upper body: back bent, head up to look ahead, hands ready.
fn crouch_top(k: Key, breath: f32, look: f32) -> Key {
    k.b(Spine, [-22.0 - breath, 0.0, 0.0])
        .b(Chest, [-12.0 + breath, 0.0, 0.0])
        .b(Head, [28.0, look, 0.0])
        .b(UpperArmL, [24.0, 0.0, -16.0])
        .b(ForearmL, [52.0, 0.0, 0.0])
        .b(UpperArmR, [30.0, 0.0, 16.0])
        .b(ForearmR, [56.0, 0.0, 0.0])
        .root([0.0, 0.0, 0.08])
}

fn crouch_idle() -> Clip {
    let key = |t: f32, breath: f32, look: f32| {
        crouch_top(Key::at(t), breath, look)
            .b(ThighL, [78.0, 8.0, -7.0])
            .b(ShinL, [-104.0, 0.0, 0.0])
            .b(FootL, [24.0, 0.0, 4.0])
            .b(ThighR, [62.0, -10.0, 7.0])
            .b(ShinR, [-96.0, 0.0, 0.0])
            .b(FootR, [32.0, 0.0, -4.0])
            .plant(0.0)
    };
    Clip::new(ClipId::CrouchIdle, 3.0, Mode::Loop, vec![key(0.0, 0.0, -3.0), key(1.5, 2.0, 5.0)])
}

fn crouch_walk() -> Clip {
    let top = |k: Key, twist: f32| {
        crouch_top(k, 0.0, -3.0 * twist)
            .b(Pelvis, [0.0, -5.0 * twist, 0.0])
            .b(Chest, [-12.0, 5.0 * twist, 0.0])
            .b(UpperArmL, [24.0 - 12.0 * twist, 0.0, -16.0])
            .b(UpperArmR, [30.0 + 12.0 * twist, 0.0, 16.0])
    };
    let half = vec![
        top(Key::at(0.0), 1.0)
            .b(ThighL, [84.0, 6.0, -6.0])
            .b(ShinL, [-92.0, 0.0, 0.0])
            .b(FootL, [14.0, 0.0, 2.0])
            .b(ThighR, [30.0, -6.0, 6.0])
            .b(ShinR, [-112.0, 0.0, 0.0])
            .b(FootR, [50.0, 0.0, -2.0])
            .plant(0.0),
        top(Key::at(0.1), 0.5)
            .b(ThighL, [74.0, 4.0, -6.0])
            .b(ShinL, [-100.0, 0.0, 0.0])
            .b(FootL, [26.0, 0.0, 2.0])
            .b(ThighR, [48.0, -4.0, 6.0])
            .b(ShinR, [-128.0, 0.0, 0.0])
            .b(FootR, [55.0, 0.0, -2.0])
            .plant(0.0),
        top(Key::at(0.2), 0.0)
            .b(ThighL, [60.0, 0.0, -6.0])
            .b(ShinL, [-96.0, 0.0, 0.0])
            .b(FootL, [36.0, 0.0, 2.0])
            .b(ThighR, [80.0, -2.0, 6.0])
            .b(ShinR, [-130.0, 0.0, 0.0])
            .b(FootR, [40.0, 0.0, -2.0])
            .plant(0.0),
        top(Key::at(0.3), -0.6)
            .b(ThighL, [40.0, -2.0, -6.0])
            .b(ShinL, [-100.0, 0.0, 0.0])
            .b(FootL, [54.0, 0.0, 2.0])
            .b(ThighR, [92.0, -4.0, 6.0])
            .b(ShinR, [-104.0, 0.0, 0.0])
            .b(FootR, [18.0, 0.0, -2.0])
            .plant(0.0),
    ];
    Clip::new(ClipId::CrouchWalk, 0.8, Mode::Loop, cycle(0.8, half))
}

// ---- Air --------------------------------------------------------------------------------------

fn jump_up() -> Clip {
    let keys = vec![
        // Push off: one leg extends, the other knee drives up, the arms swing up.
        Key::at(0.0)
            .ease(Ease::Out)
            .b(ThighL, [8.0, 4.0, -3.0])
            .b(ShinL, [-6.0, 0.0, 0.0])
            .b(FootL, [-25.0, 0.0, 0.0])
            .b(ThighR, [55.0, -4.0, 3.0])
            .b(ShinR, [-80.0, 0.0, 0.0])
            .b(FootR, [10.0, 0.0, 0.0])
            .b(Spine, [-4.0, 0.0, 0.0])
            .b(Chest, [3.0, 0.0, 0.0])
            .b(Head, [-2.0, 0.0, 0.0])
            .b(UpperArmL, [70.0, 0.0, -14.0])
            .b(ForearmL, [30.0, 0.0, 0.0])
            .b(UpperArmR, [-20.0, 0.0, 20.0])
            .b(ForearmR, [40.0, 0.0, 0.0])
            .plant(0.0),
        // Rising: the legs tuck, the arms open.
        Key::at(0.22)
            .ease(Ease::InOut)
            .b(ThighL, [40.0, 4.0, -6.0])
            .b(ShinL, [-70.0, 0.0, 0.0])
            .b(FootL, [-10.0, 0.0, 0.0])
            .b(ThighR, [70.0, -4.0, 6.0])
            .b(ShinR, [-100.0, 0.0, 0.0])
            .b(Spine, [-8.0, 0.0, 0.0])
            .b(Head, [5.0, 0.0, 0.0])
            .b(UpperArmL, [40.0, 0.0, -40.0])
            .b(ForearmL, [40.0, 0.0, 0.0])
            .b(UpperArmR, [20.0, 0.0, 40.0])
            .b(ForearmR, [45.0, 0.0, 0.0])
            .plant(0.12),
        // The top of the jump.
        Key::at(0.5)
            .b(ThighL, [30.0, 4.0, -6.0])
            .b(ShinL, [-50.0, 0.0, 0.0])
            .b(FootL, [-10.0, 0.0, 0.0])
            .b(ThighR, [45.0, -4.0, 6.0])
            .b(ShinR, [-70.0, 0.0, 0.0])
            .b(FootR, [-5.0, 0.0, 0.0])
            .b(Spine, [-4.0, 0.0, 0.0])
            .b(Head, [2.0, 0.0, 0.0])
            .b(UpperArmL, [20.0, 0.0, -50.0])
            .b(ForearmL, [30.0, 0.0, 0.0])
            .b(UpperArmR, [15.0, 0.0, 50.0])
            .b(ForearmR, [35.0, 0.0, 0.0])
            .plant(0.08),
    ];
    Clip::new(ClipId::JumpUp, 0.5, Mode::Hold, keys).fades(0.08, 0.2)
}

fn fall() -> Clip {
    let half = vec![
        Key::at(0.0)
            .b(ThighL, [30.0, 4.0, -8.0])
            .b(ShinL, [-40.0, 0.0, 0.0])
            .b(FootL, [-10.0, 0.0, 0.0])
            .b(ThighR, [15.0, -4.0, 8.0])
            .b(ShinR, [-25.0, 0.0, 0.0])
            .b(FootR, [-10.0, 0.0, 0.0])
            .b(Spine, [4.0, 0.0, 0.0])
            .b(Chest, [6.0, 0.0, 0.0])
            .b(Head, [-6.0, 0.0, 0.0])
            .b(UpperArmL, [30.0, 0.0, -70.0])
            .b(ForearmL, [25.0, 0.0, 0.0])
            .b(UpperArmR, [10.0, 0.0, 60.0])
            .b(ForearmR, [35.0, 0.0, 0.0])
            .plant(0.06),
        Key::at(0.25)
            .b(ThighL, [22.0, 4.0, -8.0])
            .b(ShinL, [-30.0, 0.0, 0.0])
            .b(FootL, [-10.0, 0.0, 0.0])
            .b(ThighR, [24.0, -4.0, 8.0])
            .b(ShinR, [-36.0, 0.0, 0.0])
            .b(FootR, [-10.0, 0.0, 0.0])
            .b(Spine, [4.0, 0.0, 2.0])
            .b(Chest, [7.0, 0.0, 0.0])
            .b(Head, [-6.0, 0.0, 0.0])
            .b(UpperArmL, [20.0, 0.0, -78.0])
            .b(ForearmL, [30.0, 0.0, 0.0])
            .b(UpperArmR, [20.0, 0.0, 66.0])
            .b(ForearmR, [30.0, 0.0, 0.0])
            .plant(0.06),
    ];
    Clip::new(ClipId::Fall, 1.0, Mode::Loop, cycle(1.0, half)).fades(0.3, 0.1)
}

fn land() -> Clip {
    let squat = |t: f32, depth: f32, ease: Ease| {
        Key::at(t)
            .ease(ease)
            .sym(ThighL, [40.0 + 22.0 * depth, 4.0, -6.0])
            .sym(ShinL, [-70.0 - 30.0 * depth, 0.0, 0.0])
            .sym(FootL, [30.0 + 8.0 * depth, 0.0, 0.0])
            .b(Spine, [-14.0 - 8.0 * depth, 0.0, 0.0])
            .b(Chest, [-6.0 - 2.0 * depth, 0.0, 0.0])
            .b(Head, [12.0 + 6.0 * depth, 0.0, 0.0])
            .sym(UpperArmL, [26.0 - 8.0 * depth, 0.0, -28.0 - 6.0 * depth])
            .sym(ForearmL, [30.0 + 10.0 * depth, 0.0, 0.0])
            .plant(0.0)
    };
    let keys = vec![squat(0.0, 0.0, Ease::Out), squat(0.1, 1.0, Ease::InOut), neutral(0.5)];
    Clip::new(ClipId::Land, 0.5, Mode::Once, keys).layer(Layer::Over).fades(0.05, 0.3)
}

/// The tucked body of a roll.
fn tuck(k: Key) -> Key {
    k.b(Spine, [-25.0, 0.0, 0.0])
        .b(Chest, [-20.0, 0.0, 0.0])
        .b(Head, [-35.0, 0.0, 0.0])
        .sym(ThighL, [120.0, 4.0, -6.0])
        .sym(ShinL, [-140.0, 0.0, 0.0])
        .sym(FootL, [30.0, 0.0, 0.0])
        .sym(UpperArmL, [60.0, 0.0, -25.0])
        .sym(ForearmL, [90.0, 0.0, 0.0])
}

fn land_roll() -> Clip {
    let keys = vec![
        // Dive: crouched, reaching forward.
        Key::at(0.0)
            .ease(Ease::Out)
            .b(Pelvis, [-30.0, 0.0, 0.0])
            .b(Spine, [-30.0, 0.0, 0.0])
            .b(Chest, [-20.0, 0.0, 0.0])
            .b(Head, [-10.0, 0.0, 0.0])
            .sym(ThighL, [100.0, 4.0, -6.0])
            .sym(ShinL, [-120.0, 0.0, 0.0])
            .sym(FootL, [20.0, 0.0, 0.0])
            .sym(UpperArmL, [110.0, 0.0, -20.0])
            .sym(ForearmL, [20.0, 0.0, 0.0])
            .root([0.0, -0.42, -0.05]),
        tuck(Key::at(0.15)).ease(Ease::Linear).b(Pelvis, [-110.0, 0.0, 0.0]).root([0.0, -0.5, -0.12]),
        tuck(Key::at(0.32)).ease(Ease::Linear).b(Pelvis, [-200.0, 0.0, 0.0]).root([0.0, -0.42, -0.15]),
        tuck(Key::at(0.47)).ease(Ease::Smooth).b(Pelvis, [-290.0, 0.0, 0.0]).sym(ThighL, [110.0, 4.0, -6.0]).root([0.0, -0.48, -0.1]),
        // Coming up onto the feet, crouched, arms out for balance.
        Key::at(0.6)
            .ease(Ease::InOut)
            .b(Pelvis, [-350.0, 0.0, 0.0])
            .b(Spine, [-25.0, 0.0, 0.0])
            .b(Chest, [-10.0, 0.0, 0.0])
            .b(Head, [18.0, 0.0, 0.0])
            .sym(ThighL, [80.0, 4.0, -6.0])
            .sym(ShinL, [-115.0, 0.0, 0.0])
            .sym(FootL, [30.0, 0.0, 0.0])
            .sym(UpperArmL, [40.0, 0.0, -30.0])
            .sym(ForearmL, [30.0, 0.0, 0.0])
            .root([0.0, -0.36, -0.04]),
        Key::at(0.8)
            .b(Pelvis, [-360.0, 0.0, 0.0])
            .b(Spine, [-6.0, 0.0, 0.0])
            .b(Chest, [-2.0, 0.0, 0.0])
            .b(Head, [6.0, 0.0, 0.0])
            .sym(ThighL, [14.0, 4.0, -3.0])
            .sym(ShinL, [-24.0, 0.0, 0.0])
            .sym(FootL, [10.0, 0.0, 0.0])
            .sym(UpperArmL, [10.0, 0.0, -12.0])
            .sym(ForearmL, [40.0, 0.0, 0.0])
            .plant(0.0),
    ];
    Clip::new(ClipId::LandRoll, 0.8, Mode::Once, keys).fades(0.06, 0.2)
}

// ---- Climbing ---------------------------------------------------------------------------------

fn climb_ladder() -> Clip {
    let half = vec![
        // Left hand high on a rung, right knee up to the next one.
        Key::at(0.0)
            .b(UpperArmL, [165.0, 0.0, -10.0])
            .b(ForearmL, [20.0, 0.0, 0.0])
            .b(UpperArmR, [120.0, 0.0, 10.0])
            .b(ForearmR, [55.0, 0.0, 0.0])
            .b(ThighL, [30.0, 4.0, -6.0])
            .b(ShinL, [-40.0, 0.0, 0.0])
            .b(FootL, [10.0, 0.0, 0.0])
            .b(ThighR, [75.0, -4.0, 6.0])
            .b(ShinR, [-100.0, 0.0, 0.0])
            .b(FootR, [25.0, 0.0, 0.0])
            .b(Spine, [-4.0, 0.0, 0.0])
            .b(Head, [14.0, 0.0, 0.0]),
        Key::at(0.25)
            .b(UpperArmL, [145.0, 0.0, -10.0])
            .b(ForearmL, [40.0, 0.0, 0.0])
            .b(UpperArmR, [140.0, 0.0, 10.0])
            .b(ForearmR, [40.0, 0.0, 0.0])
            .b(ThighL, [50.0, 4.0, -6.0])
            .b(ShinL, [-70.0, 0.0, 0.0])
            .b(FootL, [18.0, 0.0, 0.0])
            .b(ThighR, [55.0, -4.0, 6.0])
            .b(ShinR, [-70.0, 0.0, 0.0])
            .b(FootR, [18.0, 0.0, 0.0])
            .b(Spine, [-4.0, 0.0, 0.0])
            .b(Head, [10.0, 0.0, 0.0])
            .root([0.0, 0.04, 0.0]),
    ];
    Clip::new(ClipId::ClimbLadder, 1.0, Mode::Loop, cycle(1.0, half)).fades(0.15, 0.2)
}

fn climb_wall() -> Clip {
    let half = vec![
        Key::at(0.0)
            .b(Pelvis, [0.0, 0.0, 4.0])
            .b(UpperArmL, [150.0, 0.0, -35.0])
            .b(ForearmL, [30.0, 0.0, 0.0])
            .b(UpperArmR, [110.0, 0.0, 40.0])
            .b(ForearmR, [60.0, 0.0, 0.0])
            .b(ThighL, [40.0, 6.0, -25.0])
            .b(ShinL, [-60.0, 0.0, 0.0])
            .b(FootL, [20.0, 0.0, 0.0])
            .b(ThighR, [70.0, -6.0, 25.0])
            .b(ShinR, [-110.0, 0.0, 0.0])
            .b(FootR, [40.0, 0.0, 0.0])
            .b(Spine, [-6.0, 0.0, 0.0])
            .b(Head, [14.0, 10.0, 0.0]),
        Key::at(0.4)
            .b(UpperArmL, [130.0, 0.0, -38.0])
            .b(ForearmL, [50.0, 0.0, 0.0])
            .b(UpperArmR, [130.0, 0.0, 38.0])
            .b(ForearmR, [50.0, 0.0, 0.0])
            .b(ThighL, [55.0, 6.0, -25.0])
            .b(ShinL, [-85.0, 0.0, 0.0])
            .b(FootL, [30.0, 0.0, 0.0])
            .b(ThighR, [55.0, -6.0, 25.0])
            .b(ShinR, [-85.0, 0.0, 0.0])
            .b(FootR, [30.0, 0.0, 0.0])
            .b(Spine, [-6.0, 0.0, 0.0])
            .b(Head, [10.0, 0.0, 0.0])
            .root([0.0, 0.05, 0.0]),
    ];
    Clip::new(ClipId::ClimbWall, 1.6, Mode::Loop, cycle(1.6, half)).fades(0.2, 0.2)
}

fn mantle() -> Clip {
    let keys = vec![
        // Reach up for the edge.
        Key::at(0.0)
            .ease(Ease::Out)
            .sym(UpperArmL, [160.0, 0.0, -12.0])
            .sym(ForearmL, [10.0, 0.0, 0.0])
            .sym(ThighL, [20.0, 4.0, -3.0])
            .sym(ShinL, [-30.0, 0.0, 0.0])
            .sym(FootL, [10.0, 0.0, 0.0])
            .b(Spine, [4.0, 0.0, 0.0])
            .b(Head, [16.0, 0.0, 0.0])
            .plant(0.0),
        // Pull: elbows bend, knees come up.
        Key::at(0.2)
            .ease(Ease::InOut)
            .sym(UpperArmL, [100.0, 0.0, -20.0])
            .sym(ForearmL, [100.0, 0.0, 0.0])
            .b(ThighL, [80.0, 4.0, -8.0])
            .b(ShinL, [-110.0, 0.0, 0.0])
            .b(FootL, [20.0, 0.0, 0.0])
            .b(ThighR, [50.0, -4.0, 8.0])
            .b(ShinR, [-80.0, 0.0, 0.0])
            .b(FootR, [10.0, 0.0, 0.0])
            .b(Spine, [-20.0, 0.0, 0.0])
            .b(Chest, [-10.0, 0.0, 0.0])
            .b(Head, [20.0, 0.0, 0.0]),
        // Push over the edge, one knee onto it.
        Key::at(0.45)
            .ease(Ease::InOut)
            .sym(UpperArmL, [20.0, 0.0, -25.0])
            .sym(ForearmL, [70.0, 0.0, 0.0])
            .b(ThighL, [100.0, 4.0, -8.0])
            .b(ShinL, [-130.0, 0.0, 0.0])
            .b(FootL, [30.0, 0.0, 0.0])
            .b(ThighR, [20.0, -4.0, 8.0])
            .b(ShinR, [-70.0, 0.0, 0.0])
            .b(FootR, [20.0, 0.0, 0.0])
            .b(Spine, [-35.0, 0.0, 0.0])
            .b(Chest, [-15.0, 0.0, 0.0])
            .b(Head, [25.0, 0.0, 0.0])
            .root([0.0, -0.15, 0.0]),
        // Rise from the crouch on top.
        Key::at(0.68)
            .ease(Ease::InOut)
            .sym(UpperArmL, [30.0, 0.0, -15.0])
            .sym(ForearmL, [30.0, 0.0, 0.0])
            .b(ThighL, [60.0, 4.0, -6.0])
            .b(ShinL, [-80.0, 0.0, 0.0])
            .b(FootL, [20.0, 0.0, 0.0])
            .b(ThighR, [70.0, -4.0, 6.0])
            .b(ShinR, [-110.0, 0.0, 0.0])
            .b(FootR, [40.0, 0.0, 0.0])
            .b(Spine, [-15.0, 0.0, 0.0])
            .b(Chest, [-5.0, 0.0, 0.0])
            .b(Head, [10.0, 0.0, 0.0])
            .plant(0.0),
        neutral(0.9),
    ];
    Clip::new(ClipId::Mantle, 0.9, Mode::Once, keys).fades(0.1, 0.25)
}

// ---- Weapons ----------------------------------------------------------------------------------

fn sword1() -> Clip {
    let keys = vec![
        ready(0.0).ease(Ease::Out),
        // Wind up: the body turns right, the sword arm goes out and back.
        ready_legs(Key::at(0.14))
            .ease(Ease::Linear)
            .b(Pelvis, [0.0, -14.0, 0.0])
            .b(Spine, [-4.0, -10.0, 0.0])
            .b(Chest, [2.0, -30.0, 0.0])
            .b(Head, [2.0, 44.0, 0.0])
            .b(UpperArmR, [50.0, 30.0, 85.0])
            .b(ForearmR, [70.0, 0.0, 0.0])
            .b(HandR, [0.0, 0.0, -20.0])
            .b(UpperArmL, [40.0, 0.0, -30.0])
            .b(ForearmL, [60.0, 0.0, 0.0])
            .root([0.0, 0.0, 0.05])
            .plant(0.0),
        ready_legs(Key::at(0.19))
            .ease(Ease::In)
            .b(Pelvis, [0.0, -16.0, 0.0])
            .b(Spine, [-4.0, -11.0, 0.0])
            .b(Chest, [2.0, -33.0, 0.0])
            .b(Head, [2.0, 48.0, 0.0])
            .b(UpperArmR, [52.0, 32.0, 88.0])
            .b(ForearmR, [74.0, 0.0, 0.0])
            .b(HandR, [0.0, 0.0, -24.0])
            .b(UpperArmL, [42.0, 0.0, -30.0])
            .b(ForearmL, [62.0, 0.0, 0.0])
            .root([0.0, 0.0, 0.06])
            .plant(0.0),
        // The cut: a step in, the body unwinds, the arm whips across.
        Key::at(0.27)
            .ease(Ease::Out)
            .b(ThighL, [32.0, 6.0, -6.0])
            .b(ShinL, [-34.0, 0.0, 0.0])
            .b(FootL, [2.0, 0.0, 4.0])
            .b(ThighR, [-16.0, -14.0, 6.0])
            .b(ShinR, [-10.0, 0.0, 0.0])
            .b(FootR, [26.0, 0.0, -4.0])
            .b(Pelvis, [0.0, 10.0, 0.0])
            .b(Spine, [-6.0, 10.0, 0.0])
            .b(Chest, [-4.0, 28.0, 0.0])
            .b(Head, [6.0, -34.0, 0.0])
            .b(UpperArmR, [85.0, 0.0, 10.0])
            .b(ForearmR, [8.0, 0.0, 0.0])
            .b(HandR, [-35.0, 0.0, 10.0])
            .b(UpperArmL, [10.0, 0.0, -30.0])
            .b(ForearmL, [50.0, 0.0, 0.0])
            .root([0.0, 0.0, -0.08])
            .plant(0.0),
        // Follow-through: the swing carries on past the target.
        Key::at(0.34)
            .ease(Ease::InOut)
            .b(ThighL, [32.0, 6.0, -6.0])
            .b(ShinL, [-36.0, 0.0, 0.0])
            .b(FootL, [4.0, 0.0, 4.0])
            .b(ThighR, [-16.0, -14.0, 6.0])
            .b(ShinR, [-12.0, 0.0, 0.0])
            .b(FootR, [28.0, 0.0, -4.0])
            .b(Pelvis, [0.0, 14.0, 0.0])
            .b(Spine, [-6.0, 14.0, 0.0])
            .b(Chest, [-6.0, 45.0, 0.0])
            .b(Head, [6.0, -50.0, 0.0])
            .b(UpperArmR, [75.0, 0.0, -35.0])
            .b(ForearmR, [30.0, 0.0, 0.0])
            .b(HandR, [-30.0, 0.0, 20.0])
            .b(UpperArmL, [4.0, 0.0, -34.0])
            .b(ForearmL, [46.0, 0.0, 0.0])
            .root([0.0, 0.0, -0.1])
            .plant(0.0),
        ready(0.6),
    ];
    Clip::new(ClipId::Sword1, 0.6, Mode::Once, keys).fades(0.08, 0.22)
}

fn sword2() -> Clip {
    let keys = vec![
        ready(0.0).ease(Ease::Out),
        // Wind up across the body to the left.
        ready_legs(Key::at(0.13))
            .ease(Ease::Linear)
            .b(Pelvis, [0.0, 10.0, 0.0])
            .b(Spine, [-4.0, 10.0, 0.0])
            .b(Chest, [-2.0, 35.0, 0.0])
            .b(Head, [4.0, -44.0, 0.0])
            .b(UpperArmR, [70.0, 0.0, -45.0])
            .b(ForearmR, [85.0, 0.0, 0.0])
            .b(HandR, [0.0, 0.0, 30.0])
            .b(UpperArmL, [10.0, 0.0, -30.0])
            .b(ForearmL, [60.0, 0.0, 0.0])
            .plant(0.0),
        ready_legs(Key::at(0.18))
            .ease(Ease::In)
            .b(Pelvis, [0.0, 11.0, 0.0])
            .b(Spine, [-4.0, 11.0, 0.0])
            .b(Chest, [-2.0, 38.0, 0.0])
            .b(Head, [4.0, -48.0, 0.0])
            .b(UpperArmR, [72.0, 0.0, -48.0])
            .b(ForearmR, [88.0, 0.0, 0.0])
            .b(HandR, [0.0, 0.0, 32.0])
            .b(UpperArmL, [8.0, 0.0, -30.0])
            .b(ForearmL, [62.0, 0.0, 0.0])
            .plant(0.0),
        // Backhand cut to the right, stepping with the right foot.
        Key::at(0.26)
            .ease(Ease::Out)
            .b(ThighL, [-4.0, 6.0, -6.0])
            .b(ShinL, [-16.0, 0.0, 0.0])
            .b(FootL, [16.0, 0.0, 4.0])
            .b(ThighR, [24.0, -10.0, 6.0])
            .b(ShinR, [-28.0, 0.0, 0.0])
            .b(FootR, [4.0, 0.0, -4.0])
            .b(Pelvis, [0.0, -10.0, 0.0])
            .b(Spine, [-6.0, -8.0, 0.0])
            .b(Chest, [-4.0, -25.0, 0.0])
            .b(Head, [6.0, 30.0, 0.0])
            .b(UpperArmR, [80.0, 0.0, 40.0])
            .b(ForearmR, [6.0, 0.0, 0.0])
            .b(HandR, [-35.0, 0.0, -10.0])
            .b(UpperArmL, [24.0, 0.0, -36.0])
            .b(ForearmL, [50.0, 0.0, 0.0])
            .root([0.0, 0.0, -0.06])
            .plant(0.0),
        Key::at(0.33)
            .ease(Ease::InOut)
            .b(ThighL, [-4.0, 6.0, -6.0])
            .b(ShinL, [-16.0, 0.0, 0.0])
            .b(FootL, [16.0, 0.0, 4.0])
            .b(ThighR, [26.0, -10.0, 6.0])
            .b(ShinR, [-30.0, 0.0, 0.0])
            .b(FootR, [4.0, 0.0, -4.0])
            .b(Pelvis, [0.0, -14.0, 0.0])
            .b(Spine, [-6.0, -12.0, 0.0])
            .b(Chest, [-6.0, -40.0, 0.0])
            .b(Head, [6.0, 46.0, 0.0])
            .b(UpperArmR, [60.0, 0.0, 80.0])
            .b(ForearmR, [35.0, 0.0, 0.0])
            .b(HandR, [-30.0, 0.0, -20.0])
            .b(UpperArmL, [28.0, 0.0, -36.0])
            .b(ForearmL, [48.0, 0.0, 0.0])
            .root([0.0, 0.0, -0.08])
            .plant(0.0),
        ready(0.6),
    ];
    Clip::new(ClipId::Sword2, 0.6, Mode::Once, keys).fades(0.08, 0.22)
}

fn sword3() -> Clip {
    let lunge = |k: Key| {
        k.b(ThighL, [55.0, 6.0, -6.0])
            .b(ShinL, [-75.0, 0.0, 0.0])
            .b(FootL, [20.0, 0.0, 4.0])
            .b(ThighR, [-20.0, -14.0, 6.0])
            .b(ShinR, [-35.0, 0.0, 0.0])
            .b(FootR, [40.0, 0.0, -4.0])
    };
    let keys = vec![
        ready(0.0).ease(Ease::Out),
        // Big wind-up: the sword goes over the head, the back arches, up on the toes.
        Key::at(0.22)
            .ease(Ease::Linear)
            .b(ThighL, [10.0, 6.0, -6.0])
            .b(ShinL, [-10.0, 0.0, 0.0])
            .b(FootL, [-10.0, 0.0, 4.0])
            .b(ThighR, [-6.0, -14.0, 6.0])
            .b(ShinR, [-10.0, 0.0, 0.0])
            .b(FootR, [6.0, 0.0, -4.0])
            .b(Pelvis, [0.0, -6.0, 0.0])
            .b(Spine, [6.0, -6.0, 0.0])
            .b(Chest, [10.0, -14.0, 0.0])
            .b(Head, [-8.0, 18.0, 0.0])
            .b(UpperArmR, [170.0, 0.0, 20.0])
            .b(ForearmR, [60.0, 0.0, 0.0])
            .b(HandR, [-20.0, 0.0, 0.0])
            .b(UpperArmL, [120.0, 0.0, -30.0])
            .b(ForearmL, [50.0, 0.0, 0.0])
            .root([0.0, 0.0, 0.06])
            .plant(0.0),
        Key::at(0.3)
            .ease(Ease::In)
            .b(ThighL, [10.0, 6.0, -6.0])
            .b(ShinL, [-10.0, 0.0, 0.0])
            .b(FootL, [-12.0, 0.0, 4.0])
            .b(ThighR, [-6.0, -14.0, 6.0])
            .b(ShinR, [-10.0, 0.0, 0.0])
            .b(FootR, [4.0, 0.0, -4.0])
            .b(Pelvis, [0.0, -7.0, 0.0])
            .b(Spine, [8.0, -6.0, 0.0])
            .b(Chest, [12.0, -15.0, 0.0])
            .b(Head, [-10.0, 19.0, 0.0])
            .b(UpperArmR, [176.0, 0.0, 20.0])
            .b(ForearmR, [66.0, 0.0, 0.0])
            .b(HandR, [-24.0, 0.0, 0.0])
            .b(UpperArmL, [124.0, 0.0, -30.0])
            .b(ForearmL, [50.0, 0.0, 0.0])
            .root([0.0, 0.0, 0.07])
            .plant(0.0),
        // Slam down into a lunge.
        lunge(Key::at(0.4))
            .ease(Ease::Out)
            .b(Pelvis, [-10.0, 4.0, 0.0])
            .b(Spine, [-20.0, 2.0, 0.0])
            .b(Chest, [-20.0, 6.0, 0.0])
            .b(Head, [20.0, -10.0, 0.0])
            .b(UpperArmR, [55.0, 0.0, 10.0])
            .b(ForearmR, [0.0, 0.0, 0.0])
            .b(HandR, [10.0, 0.0, 0.0])
            .b(UpperArmL, [30.0, 0.0, -40.0])
            .b(ForearmL, [20.0, 0.0, 0.0])
            .root([0.0, 0.0, -0.15])
            .plant(0.0),
        // Held on the impact a moment.
        lunge(Key::at(0.52))
            .ease(Ease::Linear)
            .b(Pelvis, [-12.0, 4.0, 0.0])
            .b(Spine, [-24.0, 2.0, 0.0])
            .b(Chest, [-22.0, 6.0, 0.0])
            .b(Head, [24.0, -10.0, 0.0])
            .b(UpperArmR, [45.0, 0.0, 10.0])
            .b(ForearmR, [4.0, 0.0, 0.0])
            .b(HandR, [14.0, 0.0, 0.0])
            .b(UpperArmL, [26.0, 0.0, -42.0])
            .b(ForearmL, [24.0, 0.0, 0.0])
            .root([0.0, 0.0, -0.16])
            .plant(0.0),
        lunge(Key::at(0.6))
            .ease(Ease::InOut)
            .b(Pelvis, [-12.0, 4.0, 0.0])
            .b(Spine, [-23.0, 2.0, 0.0])
            .b(Chest, [-21.0, 6.0, 0.0])
            .b(Head, [24.0, -10.0, 0.0])
            .b(UpperArmR, [46.0, 0.0, 10.0])
            .b(ForearmR, [6.0, 0.0, 0.0])
            .b(HandR, [14.0, 0.0, 0.0])
            .b(UpperArmL, [26.0, 0.0, -42.0])
            .b(ForearmL, [24.0, 0.0, 0.0])
            .root([0.0, 0.0, -0.16])
            .plant(0.0),
        ready(0.9),
    ];
    Clip::new(ClipId::Sword3, 0.9, Mode::Once, keys).fades(0.08, 0.25)
}

/// A low guard with a long weapon held at the hip, pointing forward.
fn spear_ready(t: f32) -> Key {
    ready(t).b(UpperArmR, [12.0, 0.0, 14.0]).b(ForearmR, [80.0, 0.0, 0.0]).b(HandR, [-30.0, 0.0, 0.0]).b(UpperArmL, [40.0, 0.0, -20.0])
}

fn spear_thrust() -> Clip {
    let lunge = |k: Key| {
        k.b(ThighL, [50.0, 6.0, -6.0])
            .b(ShinL, [-60.0, 0.0, 0.0])
            .b(FootL, [10.0, 0.0, 4.0])
            .b(ThighR, [-26.0, -14.0, 6.0])
            .b(ShinR, [-10.0, 0.0, 0.0])
            .b(FootR, [36.0, 0.0, -4.0])
    };
    let keys = vec![
        spear_ready(0.0).ease(Ease::Out),
        // Draw back: weight on the back foot, the spear pulled to the shoulder.
        Key::at(0.2)
            .ease(Ease::Linear)
            .b(ThighL, [20.0, 6.0, -6.0])
            .b(ShinL, [-14.0, 0.0, 0.0])
            .b(FootL, [-6.0, 0.0, 4.0])
            .b(ThighR, [-4.0, -14.0, 6.0])
            .b(ShinR, [-30.0, 0.0, 0.0])
            .b(FootR, [34.0, 0.0, -4.0])
            .b(Pelvis, [0.0, -10.0, 0.0])
            .b(Spine, [-2.0, -10.0, 0.0])
            .b(Chest, [0.0, -30.0, 0.0])
            .b(Head, [2.0, 40.0, 0.0])
            .b(UpperArmR, [-30.0, 0.0, 20.0])
            .b(ForearmR, [100.0, 0.0, 0.0])
            .b(HandR, [-10.0, 0.0, 0.0])
            .b(UpperArmL, [60.0, 0.0, -20.0])
            .b(ForearmL, [20.0, 0.0, 0.0])
            .root([0.0, 0.0, 0.08])
            .plant(0.0),
        Key::at(0.27)
            .ease(Ease::In)
            .b(ThighL, [20.0, 6.0, -6.0])
            .b(ShinL, [-14.0, 0.0, 0.0])
            .b(FootL, [-6.0, 0.0, 4.0])
            .b(ThighR, [-4.0, -14.0, 6.0])
            .b(ShinR, [-32.0, 0.0, 0.0])
            .b(FootR, [36.0, 0.0, -4.0])
            .b(Pelvis, [0.0, -11.0, 0.0])
            .b(Spine, [-2.0, -11.0, 0.0])
            .b(Chest, [0.0, -32.0, 0.0])
            .b(Head, [2.0, 42.0, 0.0])
            .b(UpperArmR, [-34.0, 0.0, 20.0])
            .b(ForearmR, [104.0, 0.0, 0.0])
            .b(HandR, [-10.0, 0.0, 0.0])
            .b(UpperArmL, [62.0, 0.0, -20.0])
            .b(ForearmL, [18.0, 0.0, 0.0])
            .root([0.0, 0.0, 0.09])
            .plant(0.0),
        // Thrust: a lunge, the arm drives straight out.
        lunge(Key::at(0.35))
            .ease(Ease::Out)
            .b(Pelvis, [-4.0, 10.0, 0.0])
            .b(Spine, [-8.0, 8.0, 0.0])
            .b(Chest, [-6.0, 24.0, 0.0])
            .b(Head, [8.0, -34.0, 0.0])
            .b(UpperArmR, [88.0, 0.0, 4.0])
            .b(HandR, [-40.0, 0.0, 0.0])
            .b(ForearmR, [0.0, 0.0, 0.0])
            .b(UpperArmL, [-20.0, 0.0, -30.0])
            .b(ForearmL, [70.0, 0.0, 0.0])
            .root([0.0, 0.0, -0.25])
            .plant(0.0),
        lunge(Key::at(0.47))
            .ease(Ease::InOut)
            .b(Pelvis, [-4.0, 12.0, 0.0])
            .b(Spine, [-8.0, 9.0, 0.0])
            .b(Chest, [-6.0, 26.0, 0.0])
            .b(Head, [8.0, -36.0, 0.0])
            .b(UpperArmR, [86.0, 0.0, 2.0])
            .b(HandR, [-40.0, 0.0, 0.0])
            .b(ForearmR, [4.0, 0.0, 0.0])
            .b(UpperArmL, [-22.0, 0.0, -30.0])
            .b(ForearmL, [72.0, 0.0, 0.0])
            .root([0.0, 0.0, -0.26])
            .plant(0.0),
        spear_ready(0.8),
    ];
    Clip::new(ClipId::SpearThrust, 0.8, Mode::Once, keys).fades(0.08, 0.25)
}

fn club_swing() -> Clip {
    let keys = vec![
        ready(0.0).ease(Ease::Out),
        // A slow, heavy wind-up over the right shoulder.
        Key::at(0.32)
            .ease(Ease::Linear)
            .b(ThighL, [18.0, 6.0, -6.0])
            .b(ShinL, [-10.0, 0.0, 0.0])
            .b(FootL, [-8.0, 0.0, 4.0])
            .b(ThighR, [-4.0, -14.0, 6.0])
            .b(ShinR, [-24.0, 0.0, 0.0])
            .b(FootR, [28.0, 0.0, -4.0])
            .b(Pelvis, [0.0, -14.0, 0.0])
            .b(Spine, [4.0, -14.0, 0.0])
            .b(Chest, [8.0, -40.0, 0.0])
            .b(Head, [-4.0, 50.0, 0.0])
            .b(UpperArmR, [150.0, 30.0, 60.0])
            .b(ForearmR, [90.0, 0.0, 0.0])
            .b(HandR, [-30.0, 0.0, 0.0])
            .b(UpperArmL, [80.0, 0.0, -10.0])
            .b(ForearmL, [30.0, 0.0, 0.0])
            .root([0.0, 0.0, 0.08])
            .plant(0.0),
        Key::at(0.42)
            .ease(Ease::In)
            .b(ThighL, [18.0, 6.0, -6.0])
            .b(ShinL, [-10.0, 0.0, 0.0])
            .b(FootL, [-9.0, 0.0, 4.0])
            .b(ThighR, [-4.0, -14.0, 6.0])
            .b(ShinR, [-26.0, 0.0, 0.0])
            .b(FootR, [30.0, 0.0, -4.0])
            .b(Pelvis, [0.0, -15.0, 0.0])
            .b(Spine, [5.0, -15.0, 0.0])
            .b(Chest, [10.0, -42.0, 0.0])
            .b(Head, [-5.0, 52.0, 0.0])
            .b(UpperArmR, [154.0, 32.0, 62.0])
            .b(ForearmR, [94.0, 0.0, 0.0])
            .b(HandR, [-32.0, 0.0, 0.0])
            .b(UpperArmL, [82.0, 0.0, -10.0])
            .b(ForearmL, [28.0, 0.0, 0.0])
            .root([0.0, 0.0, 0.09])
            .plant(0.0),
        // The blow, down and across, with the whole body.
        Key::at(0.54)
            .ease(Ease::Out)
            .b(ThighL, [56.0, 6.0, -6.0])
            .b(ShinL, [-70.0, 0.0, 0.0])
            .b(FootL, [14.0, 0.0, 4.0])
            .b(ThighR, [-24.0, -14.0, 6.0])
            .b(ShinR, [-30.0, 0.0, 0.0])
            .b(FootR, [40.0, 0.0, -4.0])
            .b(Pelvis, [-6.0, 16.0, 0.0])
            .b(Spine, [-14.0, 10.0, 0.0])
            .b(Chest, [-24.0, 30.0, 0.0])
            .b(Head, [20.0, -40.0, 0.0])
            .b(UpperArmR, [60.0, 0.0, -30.0])
            .b(ForearmR, [6.0, 0.0, 0.0])
            .b(HandR, [10.0, 0.0, 0.0])
            .b(UpperArmL, [0.0, 0.0, -40.0])
            .b(ForearmL, [40.0, 0.0, 0.0])
            .root([0.0, 0.0, -0.18])
            .plant(0.0),
        // Its weight carries the body round.
        Key::at(0.72)
            .ease(Ease::InOut)
            .b(ThighL, [56.0, 6.0, -6.0])
            .b(ShinL, [-72.0, 0.0, 0.0])
            .b(FootL, [16.0, 0.0, 4.0])
            .b(ThighR, [-24.0, -14.0, 6.0])
            .b(ShinR, [-32.0, 0.0, 0.0])
            .b(FootR, [42.0, 0.0, -4.0])
            .b(Pelvis, [-8.0, 20.0, 0.0])
            .b(Spine, [-16.0, 16.0, 0.0])
            .b(Chest, [-26.0, 50.0, 0.0])
            .b(Head, [22.0, -56.0, 0.0])
            .b(UpperArmR, [20.0, 0.0, -50.0])
            .b(ForearmR, [20.0, 0.0, 0.0])
            .b(HandR, [20.0, 0.0, 0.0])
            .b(UpperArmL, [-6.0, 0.0, -40.0])
            .b(ForearmL, [40.0, 0.0, 0.0])
            .root([0.0, 0.0, -0.2])
            .plant(0.0),
        ready(1.1),
    ];
    Clip::new(ClipId::ClubSwing, 1.1, Mode::Once, keys).fades(0.1, 0.3)
}

/// Full draw: side-on, the bow arm straight out to the front, the string hand at the jaw.
fn bow_aim(k: Key) -> Key {
    k.b(Spine, [-2.0, -14.0, 0.0])
        .b(Chest, [0.0, -40.0, 0.0])
        .b(Head, [2.0, 52.0, 0.0])
        .b(UpperArmL, [90.0, 0.0, -33.0])
        .b(ForearmL, [0.0, 0.0, 0.0])
        .b(HandL, [0.0, 0.0, 0.0])
        .b(UpperArmR, [-90.0, 0.0, 72.0])
        .b(ForearmR, [150.0, 0.0, 0.0])
        .b(HandR, [0.0, 0.0, 0.0])
}

/// Raise the bow, the right hand comes to the string.
fn bow_raise(k: Key) -> Key {
    k.b(Chest, [0.0, -15.0, 0.0])
        .b(Head, [0.0, 15.0, 0.0])
        .b(UpperArmL, [60.0, 0.0, -30.0])
        .b(ForearmL, [20.0, 0.0, 0.0])
        .b(UpperArmR, [40.0, 0.0, 30.0])
        .b(ForearmR, [80.0, 0.0, 0.0])
}

/// Nock: bow arm out, string hand at the grip.
fn bow_nock(k: Key) -> Key {
    k.b(Spine, [-2.0, -8.0, 0.0])
        .b(Chest, [0.0, -30.0, 0.0])
        .b(Head, [2.0, 36.0, 0.0])
        .b(UpperArmL, [90.0, 0.0, -16.0])
        .b(UpperArmR, [100.0, 20.0, -50.0])
        .b(ForearmR, [20.0, 0.0, 0.0])
}

/// When the bow's draw reaches the nocked pose (aim mode draws from there).
pub(crate) const BOW_NOCKED: f32 = 0.25;

fn bow_draw() -> Clip {
    let keys = vec![bow_raise(Key::at(0.0)).ease(Ease::Out), bow_nock(Key::at(BOW_NOCKED)).ease(Ease::InOut), bow_aim(Key::at(0.6))];
    Clip::new(ClipId::BowDraw, 0.6, Mode::Hold, keys).layer(Layer::Upper).fades(0.12, 0.2)
}

/// Aim mode (G11.6): the bow raised with an arrow on the string, ready to draw.
fn bow_ready() -> Clip {
    let keys = vec![bow_raise(Key::at(0.0)).ease(Ease::Out), bow_nock(Key::at(BOW_NOCKED))];
    Clip::new(ClipId::BowAim, BOW_NOCKED, Mode::Hold, keys).layer(Layer::Upper).fades(0.12, 0.2)
}

fn bow_shoot() -> Clip {
    let keys = vec![
        bow_aim(Key::at(0.0)).ease(Ease::Out),
        // Release: the string hand flicks back, the bow arm kicks.
        bow_aim(Key::at(0.07))
            .ease(Ease::InOut)
            .b(Chest, [0.0, -42.0, 0.0])
            .b(UpperArmL, [94.0, 0.0, -32.0])
            .b(UpperArmR, [-96.0, 0.0, 86.0])
            .b(ForearmR, [118.0, 0.0, 0.0])
            .b(HandR, [0.0, 0.0, 30.0]),
        bow_aim(Key::at(0.5)).b(UpperArmL, [80.0, 0.0, -16.0]).b(UpperArmR, [-70.0, 0.0, 70.0]).b(ForearmR, [110.0, 0.0, 0.0]),
    ];
    Clip::new(ClipId::BowShoot, 0.5, Mode::Once, keys).layer(Layer::Upper).fades(0.02, 0.3)
}

fn weapon_draw() -> Clip {
    let keys = vec![
        rest_arms(Key::at(0.0)).ease(Ease::Out),
        // Reach over the right shoulder for the grip.
        rest_arms(Key::at(0.2))
            .ease(Ease::Linear)
            .b(Chest, [0.0, -10.0, 0.0])
            .b(Head, [0.0, 8.0, 0.0])
            .b(UpperArmR, [160.0, 0.0, 30.0])
            .b(ForearmR, [110.0, 0.0, 0.0])
            .b(UpperArmL, [10.0, 0.0, -14.0]),
        rest_arms(Key::at(0.26))
            .ease(Ease::InOut)
            .b(Chest, [0.0, -12.0, 0.0])
            .b(Head, [0.0, 10.0, 0.0])
            .b(UpperArmR, [158.0, 0.0, 34.0])
            .b(ForearmR, [104.0, 0.0, 0.0])
            .b(UpperArmL, [12.0, 0.0, -14.0]),
        // Bring it round to the front.
        rest_arms(Key::at(0.55)).b(UpperArmR, [30.0, 0.0, 14.0]).b(ForearmR, [50.0, 0.0, 0.0]).b(HandR, [-10.0, 0.0, 0.0]),
    ];
    Clip::new(ClipId::WeaponDraw, 0.55, Mode::Once, keys).layer(Layer::Upper).fades(0.1, 0.2).swap_at(0.24)
}

fn weapon_sheathe() -> Clip {
    let keys = vec![
        rest_arms(Key::at(0.0)).ease(Ease::Out).b(UpperArmR, [30.0, 0.0, 14.0]).b(ForearmR, [50.0, 0.0, 0.0]),
        rest_arms(Key::at(0.28))
            .ease(Ease::Linear)
            .b(Chest, [0.0, -10.0, 0.0])
            .b(Head, [0.0, 8.0, 0.0])
            .b(UpperArmR, [160.0, 0.0, 30.0])
            .b(ForearmR, [110.0, 0.0, 0.0]),
        rest_arms(Key::at(0.34))
            .ease(Ease::InOut)
            .b(Chest, [0.0, -9.0, 0.0])
            .b(Head, [0.0, 7.0, 0.0])
            .b(UpperArmR, [156.0, 0.0, 28.0])
            .b(ForearmR, [112.0, 0.0, 0.0]),
        rest_arms(Key::at(0.6)),
    ];
    Clip::new(ClipId::WeaponSheathe, 0.6, Mode::Once, keys).layer(Layer::Upper).fades(0.1, 0.2).swap_at(0.32)
}

// ---- Taking hits ------------------------------------------------------------------------------

fn hit_react() -> Clip {
    let keys = vec![
        Key::at(0.0)
            .ease(Ease::Out)
            .b(Spine, [10.0, 0.0, 0.0])
            .b(Chest, [14.0, 6.0, 0.0])
            .b(Head, [20.0, -6.0, 0.0])
            .b(UpperArmL, [30.0, 0.0, -35.0])
            .b(ForearmL, [50.0, 0.0, 0.0])
            .b(UpperArmR, [24.0, 0.0, 40.0])
            .b(ForearmR, [60.0, 0.0, 0.0])
            .b(ThighL, [-8.0, 4.0, -4.0])
            .b(ShinL, [-20.0, 0.0, 0.0])
            .b(FootL, [24.0, 0.0, 0.0])
            .b(ThighR, [20.0, -4.0, 4.0])
            .b(ShinR, [-30.0, 0.0, 0.0])
            .b(FootR, [10.0, 0.0, 0.0])
            .root([0.0, 0.0, 0.1])
            .plant(0.0),
        Key::at(0.1)
            .ease(Ease::InOut)
            .b(Spine, [14.0, 0.0, 0.0])
            .b(Chest, [18.0, 8.0, 0.0])
            .b(Head, [26.0, -8.0, 0.0])
            .b(UpperArmL, [34.0, 0.0, -40.0])
            .b(ForearmL, [56.0, 0.0, 0.0])
            .b(UpperArmR, [28.0, 0.0, 44.0])
            .b(ForearmR, [64.0, 0.0, 0.0])
            .b(ThighL, [-10.0, 4.0, -4.0])
            .b(ShinL, [-24.0, 0.0, 0.0])
            .b(FootL, [28.0, 0.0, 0.0])
            .b(ThighR, [24.0, -4.0, 4.0])
            .b(ShinR, [-34.0, 0.0, 0.0])
            .b(FootR, [10.0, 0.0, 0.0])
            .root([0.0, 0.0, 0.14])
            .plant(0.0),
        neutral(0.5),
    ];
    Clip::new(ClipId::HitReact, 0.5, Mode::Once, keys).layer(Layer::Over).fades(0.03, 0.3)
}

/// Lying on the back, limbs loose (the end of a knockdown, the start of getting up).
fn lying(t: f32) -> Key {
    Key::at(t)
        .b(Pelvis, [90.0, 0.0, 0.0])
        .b(Spine, [2.0, 0.0, 0.0])
        .b(Chest, [0.0, 0.0, 0.0])
        .b(Head, [-6.0, 14.0, 0.0])
        .b(ThighL, [8.0, 6.0, -12.0])
        .b(ShinL, [-14.0, 0.0, 0.0])
        .b(FootL, [-30.0, 0.0, 0.0])
        .b(ThighR, [24.0, -6.0, 12.0])
        .b(ShinR, [-40.0, 0.0, 0.0])
        .b(FootR, [-20.0, 0.0, 0.0])
        .sym(UpperArmL, [10.0, 0.0, -75.0])
        .sym(ForearmL, [30.0, 0.0, 0.0])
        .root([0.0, -0.8, 0.78])
}

fn knockdown() -> Clip {
    let keys = vec![
        // Struck: thrown back off the feet.
        Key::at(0.0)
            .ease(Ease::Out)
            .b(Spine, [12.0, 0.0, 0.0])
            .b(Chest, [18.0, 0.0, 0.0])
            .b(Head, [20.0, 0.0, 0.0])
            .sym(UpperArmL, [40.0, 0.0, -30.0])
            .sym(ForearmL, [30.0, 0.0, 0.0])
            .root([0.0, 0.0, 0.05])
            .plant(0.0),
        Key::at(0.22)
            .ease(Ease::In)
            .b(Pelvis, [40.0, 0.0, 0.0])
            .b(Spine, [10.0, 0.0, 0.0])
            .b(Chest, [10.0, 0.0, 0.0])
            .b(Head, [-10.0, 0.0, 0.0])
            .b(ThighL, [50.0, 4.0, -8.0])
            .b(ShinL, [-30.0, 0.0, 0.0])
            .b(ThighR, [30.0, -4.0, 8.0])
            .b(ShinR, [-20.0, 0.0, 0.0])
            .sym(UpperArmL, [70.0, 0.0, -40.0])
            .sym(ForearmL, [20.0, 0.0, 0.0])
            .root([0.0, -0.35, 0.3]),
        // Hits the ground on the back.
        Key::at(0.42)
            .ease(Ease::Out)
            .b(Pelvis, [88.0, 0.0, 0.0])
            .b(Spine, [4.0, 0.0, 0.0])
            .b(Chest, [2.0, 0.0, 0.0])
            .b(Head, [-14.0, 0.0, 0.0])
            .b(ThighL, [10.0, 6.0, -10.0])
            .b(ShinL, [-20.0, 0.0, 0.0])
            .b(FootL, [-20.0, 0.0, 0.0])
            .b(ThighR, [30.0, -6.0, 10.0])
            .b(ShinR, [-50.0, 0.0, 0.0])
            .b(FootR, [-10.0, 0.0, 0.0])
            .sym(UpperArmL, [20.0, 0.0, -80.0])
            .sym(ForearmL, [20.0, 0.0, 0.0])
            .root([0.0, -0.8, 0.75]),
        // A small bounce.
        Key::at(0.56)
            .ease(Ease::InOut)
            .b(Pelvis, [80.0, 0.0, 0.0])
            .b(Spine, [-4.0, 0.0, 0.0])
            .b(Head, [-24.0, 6.0, 0.0])
            .b(ThighL, [24.0, 6.0, -10.0])
            .b(ShinL, [-34.0, 0.0, 0.0])
            .b(FootL, [-20.0, 0.0, 0.0])
            .b(ThighR, [40.0, -6.0, 10.0])
            .b(ShinR, [-56.0, 0.0, 0.0])
            .b(FootR, [-10.0, 0.0, 0.0])
            .sym(UpperArmL, [24.0, 0.0, -70.0])
            .sym(ForearmL, [30.0, 0.0, 0.0])
            .root([0.0, -0.74, 0.78]),
        lying(0.85).ease(Ease::InOut),
        lying(1.2),
    ];
    Clip::new(ClipId::Knockdown, 1.2, Mode::Hold, keys).fades(0.04, 0.2)
}

fn get_up() -> Clip {
    let keys = vec![
        lying(0.0).ease(Ease::InOut),
        // Sit up, one knee up, hands behind.
        Key::at(0.35)
            .ease(Ease::InOut)
            .b(Pelvis, [80.0, 0.0, 0.0])
            .b(Spine, [-45.0, 0.0, 0.0])
            .b(Chest, [-35.0, 0.0, 0.0])
            .b(Head, [8.0, 0.0, 0.0])
            .b(ThighL, [40.0, 4.0, -6.0])
            .b(ShinL, [-90.0, 0.0, 0.0])
            .b(FootL, [20.0, 0.0, 0.0])
            .b(ThighR, [8.0, -6.0, 8.0])
            .b(ShinR, [-20.0, 0.0, 0.0])
            .sym(UpperArmL, [-40.0, 0.0, -20.0])
            .sym(ForearmL, [10.0, 0.0, 0.0])
            .root([0.0, -0.72, 0.6]),
        // Onto one knee.
        Key::at(0.7)
            .ease(Ease::InOut)
            .b(Pelvis, [20.0, 0.0, 0.0])
            .b(Spine, [-30.0, 0.0, 0.0])
            .b(Chest, [-15.0, 0.0, 0.0])
            .b(Head, [22.0, 0.0, 0.0])
            .b(ThighL, [70.0, 4.0, -6.0])
            .b(ShinL, [-110.0, 0.0, 0.0])
            .b(FootL, [25.0, 0.0, 0.0])
            .b(ThighR, [10.0, -4.0, 6.0])
            .b(ShinR, [-110.0, 0.0, 0.0])
            .b(FootR, [-10.0, 0.0, 0.0])
            .b(UpperArmL, [40.0, 0.0, -20.0])
            .b(ForearmL, [50.0, 0.0, 0.0])
            .b(UpperArmR, [10.0, 0.0, 20.0])
            .b(ForearmR, [30.0, 0.0, 0.0])
            .root([0.0, -0.45, 0.3]),
        // Push up to standing.
        Key::at(1.0)
            .ease(Ease::InOut)
            .b(Pelvis, [4.0, 0.0, 0.0])
            .b(Spine, [-14.0, 0.0, 0.0])
            .b(Chest, [-6.0, 0.0, 0.0])
            .b(Head, [10.0, 0.0, 0.0])
            .b(ThighL, [40.0, 4.0, -6.0])
            .b(ShinL, [-60.0, 0.0, 0.0])
            .b(FootL, [20.0, 0.0, 0.0])
            .b(ThighR, [6.0, -4.0, 6.0])
            .b(ShinR, [-40.0, 0.0, 0.0])
            .b(FootR, [30.0, 0.0, 0.0])
            .sym(UpperArmL, [20.0, 0.0, -14.0])
            .sym(ForearmL, [30.0, 0.0, 0.0])
            .root([0.0, 0.0, 0.1])
            .plant(0.0),
        neutral(1.35),
    ];
    Clip::new(ClipId::GetUp, 1.35, Mode::Once, keys).fades(0.15, 0.25)
}

fn die() -> Clip {
    let face_down = |t: f32, lift: f32| {
        Key::at(t)
            .b(Pelvis, [-85.0, 0.0, 0.0])
            .b(Spine, [-4.0, 0.0, 0.0])
            .b(Chest, [0.0, 0.0, 0.0])
            .b(Head, [10.0, 50.0, 0.0])
            .b(ThighL, [-5.0, 6.0, -8.0])
            .b(ShinL, [-10.0, 0.0, 0.0])
            .b(FootL, [-60.0, 0.0, 0.0])
            .b(ThighR, [5.0, -6.0, 12.0])
            .b(ShinR, [-30.0, 0.0, 0.0])
            .b(FootR, [-50.0, 0.0, 0.0])
            .b(UpperArmL, [150.0, 0.0, -30.0])
            .b(ForearmL, [20.0, 0.0, 0.0])
            .b(UpperArmR, [40.0, 0.0, 50.0])
            .b(ForearmR, [30.0, 0.0, 0.0])
            .root([0.0, -0.8 + lift, -0.55])
    };
    let keys = vec![
        Key::at(0.0)
            .ease(Ease::Out)
            .b(Spine, [12.0, 0.0, 0.0])
            .b(Chest, [16.0, 0.0, 0.0])
            .b(Head, [18.0, 0.0, 0.0])
            .sym(UpperArmL, [24.0, 0.0, -28.0])
            .sym(ForearmL, [40.0, 0.0, 0.0])
            .plant(0.0),
        // The strength goes: head drops, knees start to give.
        Key::at(0.3)
            .ease(Ease::InOut)
            .b(Spine, [-10.0, 0.0, 0.0])
            .b(Chest, [-14.0, 0.0, 0.0])
            .b(Head, [-24.0, 0.0, 0.0])
            .sym(UpperArmL, [-6.0, 0.0, -6.0])
            .sym(ForearmL, [10.0, 0.0, 0.0])
            .sym(ThighL, [30.0, 6.0, -4.0])
            .sym(ShinL, [-56.0, 0.0, 0.0])
            .sym(FootL, [26.0, 0.0, 0.0])
            .plant(0.0),
        // Onto the knees.
        Key::at(0.65)
            .ease(Ease::In)
            .b(Spine, [-20.0, 0.0, 0.0])
            .b(Chest, [-20.0, 0.0, 0.0])
            .b(Head, [-30.0, 0.0, 0.0])
            .sym(ThighL, [6.0, 6.0, -6.0])
            .sym(ShinL, [-90.0, 0.0, 0.0])
            .sym(FootL, [-40.0, 0.0, 0.0])
            .sym(UpperArmL, [10.0, 0.0, -10.0])
            .sym(ForearmL, [10.0, 0.0, 0.0])
            .root([0.0, -0.42, 0.0]),
        // Falls forward onto the face.
        face_down(1.0, 0.0).ease(Ease::Out),
        face_down(1.15, 0.04).ease(Ease::InOut),
        face_down(1.8, -0.01),
    ];
    Clip::new(ClipId::Die, 1.8, Mode::Hold, keys).fades(0.05, 0.2)
}
