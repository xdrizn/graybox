//! G11.4 movement: collision, climbing every ladder, wall and ledge of the valley, rolls, falls,
//! and the camera's line of sight.
use super::*;
use crate::state::Input;

fn run(tps: &mut Tps, input: Input, seconds: f32) {
    let steps = (seconds * 60.0).round() as usize;
    for _ in 0..steps {
        tps.input = input;
        tps.advance(STEP_SECS);
    }
}

/// Push the stick so the hero goes along `direction` (the camera turned to look that way).
fn toward(tps: &mut Tps, direction: Vec3) -> Input {
    tps.camera.yaw = direction.x.atan2(-direction.z);
    Input { forward: 1.0, ..Input::default() }
}

fn place(tps: &mut Tps, x: f32, z: f32) {
    let y = tps.world().ground_height(x, z);
    tps.hero = Hero::at(Vec3::new(x, y, z));
}

/// The hero overlaps a solid it is beside in height.
fn inside_a_solid(tps: &Tps) -> Option<Solid> {
    let feet = tps.hero.position;
    tps.world().solids().iter().copied().find(|solid| {
        let beside = solid.max.y > feet.y + 0.3 && solid.min.y < feet.y + HERO_HEIGHT;
        let near = Vec3::new(feet.x.clamp(solid.min.x, solid.max.x), feet.y, feet.z.clamp(solid.min.z, solid.max.z));
        beside && (Vec3::new(feet.x, feet.y, feet.z) - near).length() < HERO_RADIUS - 0.02
    })
}

#[test]
fn walking_into_trunks_pillars_and_walls_stops_at_them() {
    let mut tps = Tps::new();
    // Every direction from the start, then on through the valley: never inside a solid.
    for turn in 0..16 {
        let angle = turn as f32 * std::f32::consts::TAU / 16.0;
        let input = toward(&mut tps, Vec3::new(angle.sin(), 0.0, -angle.cos()));
        let input = Input { run: turn % 2 == 0, ..input };
        for _ in 0..240 {
            tps.input = input;
            tps.advance(STEP_SECS);
            assert_eq!(inside_a_solid(&tps), None, "inside a solid at {:?}", tps.hero.position);
        }
    }
}

#[test]
fn a_wall_in_the_way_stops_the_hero() {
    let mut tps = Tps::new();
    // The tower's base: walk at it from the east.
    let base = tps.world().solids().iter().copied().filter(|s| s.max.y - s.min.y > 1.0 && s.max.x - s.min.x > 4.0).min_by(|a, b| {
        let (a, b) = ((a.min + a.max) * 0.5, (b.min + b.max) * 0.5);
        (a - Vec3::new(-10.0, 0.0, 6.0)).length().total_cmp(&(b - Vec3::new(-10.0, 0.0, 6.0)).length())
    });
    let Some(base) = base else { panic!("no tower base") };
    let middle = (base.min + base.max) * 0.5;
    place(&mut tps, base.max.x + 3.0, middle.z);
    let input = toward(&mut tps, -Vec3::X);
    run(&mut tps, input, 2.0);
    assert!(tps.hero.position.x >= base.max.x + HERO_RADIUS - 0.02, "walked into the tower: {:?}", tps.hero.position);
    assert!(tps.hero.position.x < base.max.x + 0.5, "stopped against it: {:?}", tps.hero.position);
}

#[test]
fn every_ladder_climbs_to_the_top() {
    let ladders: Vec<Climbable> = Tps::new().world().climbables().iter().copied().filter(|c| c.kind == ClimbKind::Ladder).collect();
    assert!(!ladders.is_empty());
    for ladder in ladders {
        let mut tps = Tps::new();
        let middle = (ladder.min + ladder.max) * 0.5;
        let start = middle + ladder.normal * 1.2;
        place(&mut tps, start.x, start.z);
        let input = toward(&mut tps, -ladder.normal);
        run(&mut tps, input, 0.5);
        assert_eq!(tps.hero.moves.action.name(), "ladder", "grabbed the ladder at {start:?}");
        // Up (W) all the way, then over the top.
        let height = ladder.max.y - ladder.min.y;
        run(&mut tps, input, height / 1.8 + 2.5);
        assert_eq!(tps.hero.moves.action, Action::Free);
        assert!((tps.hero.position.y - ladder.max.y).abs() < 0.8, "on top: {:?} vs {}", tps.hero.position, ladder.max.y);
        assert!(tps.hero.grounded);
    }
}

#[test]
fn a_ladder_climbs_down_and_lets_go_with_a_jump() {
    let tps = Tps::new();
    let Some(ladder) = tps.world().climbables().iter().copied().find(|c| c.kind == ClimbKind::Ladder) else { panic!("no ladder") };
    let mut tps = tps;
    let middle = (ladder.min + ladder.max) * 0.5;
    let start = middle + ladder.normal * 1.2;
    place(&mut tps, start.x, start.z);
    let up = toward(&mut tps, -ladder.normal);
    run(&mut tps, up, 1.5);
    let high = tps.hero.position.y;
    run(&mut tps, Input { forward: -1.0, ..Input::default() }, 0.5);
    assert!(tps.hero.position.y < high - 0.5, "went down");
    for _ in 0..300 {
        run(&mut tps, Input { forward: -1.0, ..Input::default() }, 1.0 / 60.0);
        if tps.hero.moves.action == Action::Free {
            break;
        }
    }
    assert_eq!(tps.hero.moves.action, Action::Free, "off at the bottom");
    assert!(tps.hero.grounded);
    run(&mut tps, up, 0.3);
    assert_eq!(tps.hero.moves.action.name(), "ladder");
    run(&mut tps, Input { jump: true, ..Input::default() }, 1.0 / 60.0);
    assert_eq!(tps.hero.moves.action, Action::Free, "a jump lets go");
}

#[test]
fn the_rock_wall_climbs_to_the_top_and_costs_stamina() {
    let walls: Vec<Climbable> = Tps::new().world().climbables().iter().copied().filter(|c| c.kind == ClimbKind::Wall).collect();
    assert!(!walls.is_empty());
    for wall in walls {
        // Somewhere along the wall with nothing standing in front of it.
        let side = Vec3::new(wall.normal.z, 0.0, -wall.normal.x);
        let middle = (wall.min + wall.max) * 0.5;
        let grabbed = [0.0, -2.5, 2.5, -3.5, 3.5].into_iter().find_map(|along| {
            let mut tps = Tps::new();
            let start = middle + side * along + wall.normal * 1.5;
            place(&mut tps, start.x, start.z);
            let input = toward(&mut tps, -wall.normal);
            run(&mut tps, input, 0.6);
            (tps.hero.moves.action.name() == "wall").then_some((tps, input))
        });
        let Some((mut tps, input)) = grabbed else { panic!("could not grab the wall at {middle:?}") };
        run(&mut tps, input, 1.0);
        assert!(tps.hero.moves.stamina < STAMINA_MAX, "climbing tires");
        // Up to the top and over it (then stop pushing, or it walks off the back).
        for _ in 0..((wall.max.y - wall.min.y) / 1.3 * 60.0) as usize + 120 {
            run(&mut tps, input, 1.0 / 60.0);
            if tps.hero.moves.action == Action::Free {
                break;
            }
        }
        assert_eq!(tps.hero.moves.action, Action::Free);
        assert!(tps.hero.position.y > wall.max.y - 0.2, "on top: {:?} vs {}", tps.hero.position, wall.max.y);
    }
}

#[test]
fn ledges_crates_and_blocks_are_mantled_onto() {
    let world = World::new();
    let mut climbed = 0;
    let ledges: Vec<Climbable> = world.climbables().iter().copied().filter(|c| c.kind == ClimbKind::Mantle).collect();
    for ledge in &ledges {
        let middle = (ledge.min + ledge.max) * 0.5;
        // A crate with another stacked on it has no room on top.
        let covered = ledges.iter().any(|other| {
            (other.min.y - ledge.max.y).abs() < 0.05
                && other.min.x < ledge.max.x
                && other.max.x > ledge.min.x
                && other.min.z < ledge.max.z
                && other.max.z > ledge.min.z
        });
        if covered {
            continue;
        }
        let half = (ledge.max - ledge.min) * 0.5;
        let mut reachable = false;
        let mut done = false;
        for side in [Vec3::X, -Vec3::X, Vec3::Z, -Vec3::Z] {
            let start = middle + side * (half.dot(side.abs()) + 0.9);
            let ground = world.ground_height(start.x, start.z);
            let rise = ledge.max.y - ground;
            if !(0.5..1.9).contains(&rise) {
                continue;
            }
            let mut tps = Tps::new();
            place(&mut tps, start.x, start.z);
            if inside_a_solid(&tps).is_some() {
                continue;
            }
            reachable = true;
            let input = toward(&mut tps, -side);
            for _ in 0..120 {
                run(&mut tps, input, 1.0 / 60.0);
                if (tps.hero.position.y - ledge.max.y).abs() < 0.05 && tps.hero.moves.action == Action::Free {
                    done = true;
                    break;
                }
            }
            if done {
                break;
            }
        }
        assert!(!reachable || done, "could not climb onto the ledge at {middle:?}");
        climbed += usize::from(done);
    }
    assert!(climbed >= 3, "climbed {climbed} ledges");
}

#[test]
fn a_roll_dodges_forward_and_costs_stamina() {
    let mut tps = Tps::new();
    let start = tps.hero.position;
    run(&mut tps, Input { roll: true, ..Input::default() }, 1.0 / 60.0);
    assert_eq!(tps.hero.moves.action.name(), "roll");
    assert!(tps.hero.moves.stamina < STAMINA_MAX);
    run(&mut tps, Input::default(), 1.0);
    assert_eq!(tps.hero.moves.action, Action::Free);
    assert!((tps.hero.position - start).length() > 3.0, "rolled {:?}", tps.hero.position - start);
}

#[test]
fn a_hard_fall_knocks_down_unless_rolled() {
    let mut tps = Tps::new();
    tps.hero.position.y += 12.0;
    tps.hero.grounded = false;
    run(&mut tps, Input::default(), 1.5);
    assert_eq!(tps.hero.moves.action.name(), "down", "knocked down");
    // It cannot move while down, then gets up by itself.
    let lying = tps.hero.position;
    run(&mut tps, Input { forward: 1.0, ..Input::default() }, 0.5);
    assert!((tps.hero.position - lying).length() < 0.01);
    run(&mut tps, Input::default(), 3.0);
    assert_eq!(tps.hero.moves.action, Action::Free, "got up");

    let mut tps = Tps::new();
    tps.hero.position.y += 12.0;
    tps.hero.grounded = false;
    // Roll pressed just before touching down (the fall takes about 1.1 s).
    for _ in 0..200 {
        let low = tps.hero.position.y - tps.world().ground_height(tps.hero.position.x, tps.hero.position.z) < 2.5;
        run(&mut tps, Input { roll: low, ..Input::default() }, 1.0 / 60.0);
        if tps.hero.grounded {
            break;
        }
    }
    assert_eq!(tps.hero.moves.action.name(), "roll", "the roll takes the fall");
}

#[test]
fn a_jump_pressed_just_before_landing_still_jumps() {
    let mut tps = Tps::new();
    run(&mut tps, Input { jump: true, ..Input::default() }, 1.0 / 60.0);
    assert!(!tps.hero.grounded);
    // Fall back down and press jump a few steps before the feet touch.
    let mut pressed = false;
    for _ in 0..120 {
        let height = tps.hero.position.y - tps.world().ground_height(tps.hero.position.x, tps.hero.position.z);
        let press = !pressed && tps.hero.velocity.y < 0.0 && height < 0.3;
        pressed |= press;
        run(&mut tps, Input { jump: press, ..Input::default() }, 1.0 / 60.0);
        if pressed && tps.hero.velocity.y > 0.0 {
            return;
        }
    }
    panic!("the early press was lost");
}

#[test]
fn the_camera_stops_short_of_a_wall_behind_the_hero() {
    let mut tps = Tps::new();
    // Stand with the tower base right behind (the camera looks from the south, the base is north).
    let base = tps.world().solids().iter().copied().filter(|s| s.max.x - s.min.x > 4.0 && s.max.y - s.min.y > 1.4).min_by(|a, b| {
        let at = Vec3::new(-10.0, 0.0, 6.0);
        (((a.min + a.max) * 0.5) - at).length().total_cmp(&(((b.min + b.max) * 0.5) - at).length())
    });
    let Some(base) = base else { panic!("no tower base") };
    let middle = (base.min + base.max) * 0.5;
    place(&mut tps, middle.x, base.max.z + 1.0);
    // Looking south: the camera would sit inside the base, to the north.
    tps.camera.yaw = std::f32::consts::PI;
    tps.camera.pitch = -0.1;
    let frame = tps.frame();
    assert!(frame.eye.z > base.max.z, "the camera stays in front of the wall: {:?}", frame.eye);
    assert!((frame.eye - frame.target).length() < 4.4);
}

#[test]
fn a_jump_off_a_ladder_is_only_a_small_push_back() {
    let tps = Tps::new();
    let Some(ladder) = tps.world().climbables().iter().copied().find(|c| c.kind == ClimbKind::Ladder) else { panic!("no ladder") };
    let mut tps = tps;
    let start = (ladder.min + ladder.max) * 0.5 + ladder.normal * 1.2;
    place(&mut tps, start.x, start.z);
    let up = toward(&mut tps, -ladder.normal);
    run(&mut tps, up, 2.0);
    assert_eq!(tps.hero.moves.action.name(), "ladder");
    let held = tps.hero.position.y;
    run(&mut tps, Input { jump: true, ..Input::default() }, 1.0 / 60.0);
    let mut highest = tps.hero.position.y;
    for _ in 0..60 {
        run(&mut tps, Input::default(), 1.0 / 60.0);
        highest = highest.max(tps.hero.position.y);
    }
    // Pushed off with 3 m/s up: about 0.2 m higher, not a whole jump (1.2 m) on top.
    assert!(highest - held < 0.5, "rose {} m letting go", highest - held);
}

#[test]
fn a_long_fall_onto_a_crate_lands_on_its_top() {
    let world = World::new();
    let ledges: Vec<Climbable> = world.climbables().iter().copied().filter(|c| c.kind == ClimbKind::Mantle).collect();
    let mut landed = 0;
    for ledge in &ledges {
        let middle = (ledge.min + ledge.max) * 0.5;
        let open = !world
            .solids()
            .iter()
            .any(|s| s.min.y > ledge.max.y - 0.05 && s.min.x < middle.x && s.max.x > middle.x && s.min.z < middle.z && s.max.z > middle.z);
        if !open {
            continue;
        }
        // From several heights, so the last step before the top ends at different depths below it.
        for height in [9.0, 12.0, 15.5, 19.0, 24.0, 30.0] {
            let mut tps = Tps::new();
            tps.hero = Hero::at(Vec3::new(middle.x, ledge.max.y + height, middle.z));
            tps.hero.grounded = false;
            run(&mut tps, Input::default(), 3.0);
            assert!((tps.hero.position.y - ledge.max.y).abs() < 0.05, "fell {height} m through the top of {ledge:?}: {:?}", tps.hero.position);
        }
        landed += 1;
    }
    assert!(landed >= 3, "landed on {landed} crates");
}

#[test]
fn climbing_the_wall_sideways_stops_at_what_stands_in_front_of_it() {
    let walls: Vec<Climbable> = Tps::new().world().climbables().iter().copied().filter(|c| c.kind == ClimbKind::Wall).collect();
    for wall in walls {
        let side = Vec3::new(wall.normal.z, 0.0, -wall.normal.x);
        let middle = (wall.min + wall.max) * 0.5;
        for along in [-3.5_f32, -2.5, 2.5, 3.5] {
            let mut tps = Tps::new();
            let start = middle + side * along + wall.normal * 1.5;
            place(&mut tps, start.x, start.z);
            let input = toward(&mut tps, -wall.normal);
            run(&mut tps, input, 0.6);
            if tps.hero.moves.action.name() != "wall" {
                continue;
            }
            // Low on the wall, across the whole of it toward the other end.
            let across = Input { strafe: -along.signum(), ..Input::default() };
            for _ in 0..360 {
                run(&mut tps, across, 1.0 / 60.0);
                assert_eq!(inside_a_solid(&tps), None, "climbed into a solid at {:?}", tps.hero.position);
            }
        }
    }
}
