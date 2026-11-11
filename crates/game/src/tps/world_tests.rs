//! The valley's data: the heightfield, the props, the tags and their budgets.
use super::*;

fn world() -> World {
    World::new()
}

/// Which way a prop kind is allowed to turn: blockers turn by quarter turns only.
fn blocks(kind: PropKind) -> bool {
    !matches!(kind, PropKind::Pine | PropKind::Oak | PropKind::Rock)
}

#[test]
fn the_ground_is_about_thirteen_thousand_triangles_on_a_two_metre_grid() {
    let world = world();
    let count = world.terrain().triangles().count();
    assert_eq!(count, 2 * CELLS * CELLS);
    assert!(count <= 13_000, "{count}");
    assert_eq!(CELL * CELLS as f32, HALF_EXTENT * 2.0);
}

#[test]
fn the_world_is_the_same_every_time_it_is_made() {
    assert_eq!(World::new(), World::new());
    assert_eq!(World::new().checksum(), World::new().checksum());
    // A golden number: if you change the valley on purpose, update it. If you did not, something
    // started to depend on the machine and the browser build would show a different valley.
    assert_eq!(world().terrain().checksum(), GOLDEN_TERRAIN, "terrain checksum");
    assert_eq!(world().checksum(), GOLDEN_WORLD, "world checksum");
}

const GOLDEN_TERRAIN: u64 = 13850094856200085605;
const GOLDEN_WORLD: u64 = 11807657429920657555;

#[test]
fn the_height_lookup_is_the_surface_that_is_drawn() {
    let world = world();
    let terrain = world.terrain();
    let mut seed = 5_u32;
    for _ in 0..2000 {
        let x = (crate::arena::random(&mut seed) * 2.0 - 1.0) * HALF_EXTENT * 0.98;
        let z = (crate::arena::random(&mut seed) * 2.0 - 1.0) * HALF_EXTENT * 0.98;
        let (i, j) = ((((x + HALF_EXTENT) / CELL) as usize).min(CELLS - 1), (((z + HALF_EXTENT) / CELL) as usize).min(CELLS - 1));
        let lookup = world.ground_height(x, z);
        // The point lies in one of the cell's two triangles; the lookup is that triangle's plane.
        let fits = terrain.cell_triangles(i, j).iter().any(|[a, b, c]| {
            let normal = (*b - *a).cross(*c - *a);
            normal.y > 0.0 && (a.y - (normal.x * (x - a.x) + normal.z * (z - a.z)) / normal.y - lookup).abs() < 1e-3
        });
        assert!(fits, "({x}, {z}) -> {lookup} is on neither triangle of cell ({i}, {j})");
    }
}

#[test]
fn every_triangle_of_the_ground_faces_up() {
    for [a, b, c] in world().terrain().triangles() {
        assert!((b - a).cross(c - a).y > 0.0);
    }
}

#[test]
fn the_height_is_finite_everywhere_even_off_the_map_and_for_junk() {
    let world = world();
    for (x, z) in [(0.0, 0.0), (1e9, -1e9), (f32::NAN, 1.0), (1.0, f32::INFINITY), (-HALF_EXTENT, HALF_EXTENT), (HALF_EXTENT * 3.0, 0.0)] {
        assert!(world.ground_height(x, z).is_finite(), "({x}, {z})");
    }
    assert_eq!(world.ground_height(f32::NAN, 0.0), 0.0);
}

#[test]
fn the_ground_is_a_hill_a_cliff_a_valley_and_a_plateau() {
    let world = world();
    let spawn = world.spawn();
    let meadow = world.ground_height(10.0, 25.0);
    let valley = world.ground_height(10.0, -40.0);
    let plateau = world.ground_height(-60.0, -10.0);
    assert!(spawn.y > meadow + 2.0, "hill {} over meadow {meadow}", spawn.y);
    assert!(meadow > valley + 3.5, "cliff: meadow {meadow}, valley {valley}");
    assert!(plateau > meadow + 2.5 && plateau > valley + 7.0, "plateau {plateau}");
    // The cliff is steep somewhere, the meadow is walkable.
    let steepest = (-30..30).map(|k| world.slope(k as f32 * 2.0, terrain_cliff(k as f32 * 2.0))).fold(0.0_f32, f32::max);
    assert!(steepest > 1.0, "steepest cliff {steepest}");
    assert!(world.slope(10.0, 25.0) < 0.3);
}

fn terrain_cliff(x: f32) -> f32 {
    terrain::cliff_z(x)
}

#[test]
fn the_hero_starts_on_flat_dry_ground_inside_the_map_clear_of_everything() {
    let world = world();
    let spawn = world.spawn();
    assert!(spawn.is_finite());
    assert_eq!(spawn.y, world.ground_height(spawn.x, spawn.z));
    assert!(spawn.x.abs() < world.walk_limit() && spawn.z.abs() < world.walk_limit());
    assert!(world.slope(spawn.x, spawn.z) < 0.15, "slope {}", world.slope(spawn.x, spawn.z));
    assert!(!world.in_water(spawn.x, spawn.z));
    let body = Solid::standing(spawn, 1.0, 2.0);
    for solid in world.solids() {
        let overlap = body.min.cmple(solid.max).all() && body.max.cmpge(solid.min).all();
        assert!(!overlap, "spawn is inside {solid:?}");
    }
    assert_eq!(world.keep_on_map(Vec3::new(f32::NAN, 0.0, 0.0)), spawn);
    assert_eq!(world.keep_on_map(Vec3::new(1e6, 4.0, -1e6)), Vec3::new(world.walk_limit(), 4.0, -world.walk_limit()));
}

#[test]
fn the_river_runs_in_a_channel_and_falls_from_the_plateau_to_the_valley() {
    let world = world();
    let river = world.river();
    assert_eq!(river.len(), 2 * RIVER_END as usize + 1);
    for point in &river {
        assert!(point.is_finite());
        // The water stands over the bed everywhere along the middle line (except the very ends, where
        // the channel fades into the rim).
        if point.x.abs() < RIVER_END - 8.0 && (point.x - FALL_X).abs() > 5.0 {
            let bed = world.ground_height(point.x, point.z);
            assert!(point.y - bed > 0.5 && point.y - bed < 1.6, "x {}: bed {bed}, water {}", point.x, point.y);
            assert!(world.in_water(point.x, point.z));
        }
    }
    let spring = river.first().map_or(0.0, |p| p.y);
    let mouth = river.last().map_or(0.0, |p| p.y);
    assert!(spring > mouth + 7.0, "spring {spring}, mouth {mouth}");
    // It drops between two points a few metres apart: the waterfall.
    let drop = river.windows(2).map(|pair| pair[0].y - pair[1].y).fold(0.0_f32, f32::max);
    assert!(drop > 1.5, "steepest drop in one metre: {drop}");
    let fall = river.iter().find(|p| (p.x - FALL_X).abs() < 0.5).map_or(0.0, |p| p.y);
    assert!((fall - mouth).abs() > 1.0 && (fall - spring).abs() > 1.0, "the fall is in between");
}

#[test]
fn the_water_is_not_a_place_to_plant_trees_or_stand_ruins() {
    let world = world();
    for prop in world.props() {
        assert!(!world.in_water(prop.position.x, prop.position.z), "{prop:?} in the river");
    }
}

#[test]
fn trees_rocks_and_ruins_are_placed_within_their_budgets() {
    let world = world();
    let count = |kinds: &[PropKind]| world.props().iter().filter(|prop| kinds.contains(&prop.kind)).count();
    let trees = count(&[PropKind::Pine, PropKind::Oak]);
    let rocks = count(&[PropKind::Rock]);
    assert!((100..=TREE_MAX).contains(&trees), "{trees} trees");
    assert!((60..=ROCK_MAX).contains(&rocks), "{rocks} rocks");
    assert!(count(&[PropKind::Pillar, PropKind::BrokenPillar]) >= 10);
    assert!(count(&[PropKind::Arch]) >= 4, "arches");
    assert!(count(&[PropKind::Crate, PropKind::Block]) >= 8);
    assert!(world.props().len() <= 500, "{} props", world.props().len());
}

#[test]
fn every_prop_stands_on_the_ground_inside_the_map_and_blockers_turn_by_quarter_turns() {
    let world = world();
    for prop in world.props() {
        assert!(prop.position.is_finite() && prop.yaw.is_finite() && prop.scale.is_finite() && prop.scale > 0.0);
        assert!(prop.position.x.abs() < HALF_EXTENT && prop.position.z.abs() < HALF_EXTENT, "{prop:?}");
        // On the ground, or on a crate stacked on it.
        let ground = world.ground_height(prop.position.x, prop.position.z);
        assert!(prop.position.y >= ground - 1e-4 && prop.position.y <= ground + 1.2, "{prop:?} floats over {ground}");
        if blocks(prop.kind) {
            let turns = prop.yaw / std::f32::consts::FRAC_PI_2;
            assert!((turns - turns.round()).abs() < 1e-5, "{prop:?} is not turned by a quarter");
        }
    }
}

#[test]
fn solids_are_real_boxes_and_the_hero_can_stand_under_an_arch() {
    let world = world();
    assert!(world.solids().len() < 1200, "{}", world.solids().len());
    for solid in world.solids() {
        assert!(solid.is_finite(), "{solid:?}");
    }
    let arch = Prop { kind: PropKind::Arch, position: Vec3::ZERO, yaw: 0.0, scale: 1.0 };
    let legs = arch.solids();
    assert_eq!(legs.len(), 2);
    // Between the legs there is room to walk (wider than the hero), and it is the gap along X.
    let gap = legs.iter().map(|leg| leg.min.x).fold(f32::MIN, f32::max) - legs.iter().map(|leg| leg.max.x).fold(f32::MAX, f32::min);
    assert!(gap > 1.2, "{gap}");
    let turned = Prop { yaw: std::f32::consts::FRAC_PI_2, ..arch }.solids();
    assert!(turned.iter().all(|leg| leg.min.x > -1.0 && leg.max.x < 1.0), "a turned arch spans along Z");
}

#[test]
fn ladders_ledges_and_a_climb_wall_are_tagged() {
    let world = world();
    let tags = |kind: ClimbKind| world.climbables().iter().filter(|tag| tag.kind == kind).copied().collect::<Vec<_>>();
    let (ladders, ledges, walls) = (tags(ClimbKind::Ladder), tags(ClimbKind::Mantle), tags(ClimbKind::Wall));
    assert!(ladders.len() >= 2, "ladders");
    assert!(ledges.len() >= 8, "ledges");
    assert!(!walls.is_empty(), "climb wall");
    for tag in world.climbables() {
        assert!(tag.min.is_finite() && tag.max.is_finite() && tag.min.cmple(tag.max).all(), "{tag:?}");
    }
    for ladder in &ladders {
        assert!((ladder.normal.length() - 1.0).abs() < 1e-5 && ladder.normal.y == 0.0, "{ladder:?}");
        let height = ladder.max.y - ladder.min.y;
        assert!((4.0..12.0).contains(&height), "ladder {height} m");
        // Its foot is on the ground you walk up to it from.
        let foot = ladder.min + (ladder.max - ladder.min) * Vec3::new(0.5, 0.0, 0.5) + ladder.normal * 0.6;
        assert!((world.ground_height(foot.x, foot.z) - ladder.min.y).abs() < 0.8, "{ladder:?}");
    }
    for ledge in &ledges {
        let height = ledge.max.y - ledge.min.y;
        assert!((0.8..2.0).contains(&height), "a ledge to mantle is chest high, not {height}");
    }
    for wall in &walls {
        assert!(wall.max.y - wall.min.y >= 5.0 && wall.normal.y == 0.0);
    }
}

#[test]
fn the_watchtower_has_a_beacon_above_the_ground() {
    let world = world();
    let beacon = world.beacon();
    assert!(beacon.is_finite() && beacon.y > world.ground_height(beacon.x, beacon.z) + 10.0);
    let glowing = world.structures().iter().filter(|piece| piece.emissive > 1.0).count();
    assert!(glowing >= 3, "{glowing} glowing pieces");
}

#[test]
fn the_backdrop_stands_outside_the_map() {
    let world = world();
    assert!(world.backdrop().len() >= 10);
    for piece in world.backdrop() {
        assert!(piece.position.is_finite() && piece.size.is_finite());
        assert!(piece.position.x.abs().max(piece.position.z.abs()) > HALF_EXTENT * 1.4, "{piece:?}");
    }
}

#[test]
fn grass_grows_near_the_start_and_not_in_water_on_cliffs_or_on_ruins() {
    let world = world();
    let spawn = world.spawn();
    let mut near = 0.0;
    for k in 0..40 {
        near += world.grass_cover(spawn.x + (k % 8) as f32 * 1.5 - 6.0, spawn.z - (k / 8) as f32 * 1.5);
    }
    assert!(near > 5.0, "grass near the start: {near}");
    assert_eq!(world.grass_cover(0.0, river_middle(0.0)), 0.0);
    assert_eq!(world.grass_cover(34.0, 10.0), 0.0, "paved courtyard");
    assert_eq!(world.grass_cover(f32::NAN, 0.0), 0.0);
    assert_eq!(world.grass_cover(HALF_EXTENT, 0.0), 0.0);
}

fn river_middle(x: f32) -> f32 {
    terrain::river_z(x)
}

#[test]
fn the_world_data_fits_the_memory_budget() {
    // "simulation and world data under 8 MB of RAM" (the spec); the valley is a small part of that.
    let bytes = world().bytes();
    assert!(bytes < 2_000_000, "{bytes} bytes");
}

#[test]
fn the_hero_walks_on_the_slopes_of_the_hill_and_the_height_never_jumps() {
    // Walking in a straight line over the whole map, the ground never jumps by more than a step.
    let world = world();
    let mut worst = 0.0_f32;
    for lane in [-60.0, -30.0, 0.0, 30.0, 60.0] {
        let mut last = world.ground_height(-HALF_EXTENT, lane);
        for k in 1..=1600 {
            let height = world.ground_height(-HALF_EXTENT + k as f32 * 0.1, lane);
            worst = worst.max((height - last).abs());
            last = height;
        }
    }
    // A 0.1 m step over a face of slope 3 changes the height by 0.3 m at most.
    assert!(worst < 0.45, "worst jump in 10 cm: {worst}");
}
