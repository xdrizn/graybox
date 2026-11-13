//! The third-person world's picture, counted on the CPU: budgets, determinism and the shapes' winding.
use super::super::mesh::Vertex;
use super::grass::{self, FADE_END, TUFT_VERTICES};
use super::scene::{SHADOW_SIZE, SceneData};
use super::shapes::{blob, frustum, material};
use super::{props, sky};
use figcraft_game::tps::{HALF_EXTENT, PropKind, World};
use glam::Vec3;

fn data() -> (World, SceneData) {
    let world = World::new();
    let data = SceneData::build(&world);
    (world, data)
}

fn triangles(vertices: &[Vertex]) -> impl Iterator<Item = [Vec3; 3]> + '_ {
    vertices.chunks_exact(3).map(|tri| [0, 1, 2].map(|k| Vec3::from_array(tri[k].position)))
}

fn normal(points: [Vec3; 3]) -> Vec3 {
    (points[1] - points[0]).cross(points[2] - points[0]).normalize_or_zero()
}

#[test]
fn the_picture_stays_inside_the_budgets_of_the_design() {
    let (_, data) = data();
    // Under 250k triangles (counting the shadow pass too, to be safe), under 150 draw calls.
    let drawn = data.most_triangles_drawn();
    let shadow = data.shadow_triangles();
    assert!(drawn + shadow < 250_000, "{drawn} drawn + {shadow} in the shadow map");
    let calls = data.most_draw_calls();
    assert!(calls < 150, "{calls} draw calls");
    // Under 40 MB of GPU memory with a 2048² shadow map on the desktop and a 1024² one in the browser.
    for size in [2048, 1024] {
        let bytes = data.gpu_bytes(size);
        assert!(bytes < 40 * 1024 * 1024, "{bytes} bytes with a {size}² shadow map");
    }
    assert_eq!(SHADOW_SIZE, if cfg!(target_arch = "wasm32") { 1024 } else { 2048 });
    println!("drawn {drawn}, shadow {shadow}, calls {calls}, bytes {} / {}", data.gpu_bytes(2048), data.gpu_bytes(1024));
}

#[test]
fn the_ground_is_most_of_the_static_triangles_and_the_ground_alone_is_about_thirteen_thousand() {
    let (world, data) = data();
    let solid = (data.statics.solid.end - data.statics.solid.start) / 3;
    assert!(solid as usize >= world.terrain().triangles().count());
    assert!(data.statics.backdrop.end > data.statics.backdrop.start, "mountains and keep");
    assert!(data.statics.water.end > data.statics.water.start, "river");
    assert!(data.statics.sky.end > data.statics.sky.start, "the sky dome");
    assert_eq!(data.statics.sky.start, data.statics.water.end, "the sky follows the water");
    assert_eq!(data.statics.vertices.len() as u32, data.statics.sky.end);
}

#[test]
fn the_scene_data_is_the_same_every_time() {
    let (_, one) = data();
    let (_, two) = data();
    assert_eq!(bytemuck::cast_slice::<_, u8>(&one.statics.vertices), bytemuck::cast_slice::<_, u8>(&two.statics.vertices));
    assert_eq!(one.grass.tufts, two.grass.tufts);
    for (a, b) in one.batches.iter().zip(&two.batches) {
        assert_eq!(a.instances, b.instances);
    }
}

#[test]
fn grass_is_a_few_tens_of_thousands_of_tufts_none_in_water_and_a_few_rows_near_the_camera() {
    let (world, data) = data();
    let count = data.grass.tufts.len();
    assert!((8_000..60_000).contains(&count), "{count} tufts");
    assert_eq!(data.tuft.len() as u32, TUFT_VERTICES);
    for tuft in &data.grass.tufts {
        assert!(tuft.iter().all(|value| value.is_finite()));
        assert!(tuft[0].abs() < HALF_EXTENT && tuft[2].abs() < HALF_EXTENT);
        assert!(!world.in_water(tuft[0], tuft[2]), "a tuft in the river at {tuft:?}");
        assert!((0.0..1.0).contains(&tuft[3]));
    }
    let spawn = world.spawn();
    let runs = data.grass.visible(spawn, FADE_END);
    assert!(!runs.is_empty() && runs.len() <= 11, "{} draw calls of grass", runs.len());
    let near: u32 = runs.iter().map(|run| run.end - run.start).sum();
    assert!(near > 500, "{near} tufts near the start");
    // Every tuft in the runs is close enough to matter, give or take a chunk; none are missed.
    let mut seen = 0;
    for run in &runs {
        for tuft in data.grass.tufts.get(run.start as usize..run.end as usize).unwrap_or(&[]) {
            seen += 1;
            assert!((Vec3::new(tuft[0], 0.0, tuft[2]) - Vec3::new(spawn.x, 0.0, spawn.z)).length() < FADE_END + 12.0);
        }
    }
    assert_eq!(seen, near);
    let missed = data
        .grass
        .tufts
        .iter()
        .enumerate()
        .filter(|(index, tuft)| {
            (Vec3::new(tuft[0], 0.0, tuft[2]) - Vec3::new(spawn.x, 0.0, spawn.z)).length() < FADE_END
                && !runs.iter().any(|run| run.contains(&(*index as u32)))
        })
        .count();
    assert_eq!(missed, 0, "tufts within the fade distance that would not be drawn");
}

#[test]
fn a_camera_off_the_map_or_broken_draws_no_grass() {
    let (_, data) = data();
    assert!(data.grass.visible(Vec3::new(f32::NAN, 0.0, 0.0), FADE_END).is_empty());
    assert!(data.grass.visible(Vec3::new(1e6, 0.0, 0.0), FADE_END).is_empty());
}

#[test]
fn every_prop_has_a_mesh_inside_its_footprint_and_instances_that_are_finite() {
    let (_, data) = data();
    assert_eq!(data.batches.len(), PropKind::ALL.len());
    for batch in &data.batches {
        assert!(!batch.vertices.is_empty(), "{:?}", batch.kind);
        assert_eq!(batch.vertices.len() % 3, 0);
        let tris = batch.vertices.len() / 3;
        assert!(tris < 140, "{:?} has {tris} triangles", batch.kind);
        for vertex in &batch.vertices {
            assert!(vertex.position.iter().chain(&vertex.normal).all(|value| value.is_finite()));
            let tall = Vec3::from_array(vertex.position).y;
            assert!((-0.2..7.0).contains(&tall), "{:?} reaches {tall} m", batch.kind);
        }
        for instance in &batch.instances {
            assert!(instance.iter().all(|value| value.is_finite()));
        }
    }
}

#[test]
fn trees_stand_as_tall_as_the_collision_boxes_say() {
    // A pine is about 4 m, an oak about 4.5 m at scale 1, above the 3 m of trunk box.
    for (kind, low, high) in [(PropKind::Pine, 3.5, 5.0), (PropKind::Oak, 3.8, 5.5)] {
        let top = props::mesh(kind).iter().map(|vertex| vertex.position[1]).fold(0.0_f32, f32::max);
        assert!((low..high).contains(&top), "{kind:?} is {top} m tall");
    }
    let pillar = props::mesh(PropKind::Pillar).iter().map(|vertex| vertex.position[1]).fold(0.0_f32, f32::max);
    assert!((pillar - figcraft_game::tps::PILLAR_HEIGHT).abs() < 1e-3, "{pillar}");
}

#[test]
fn shapes_face_outwards() {
    let mut cone = Vec::new();
    frustum(&mut cone, Vec3::ZERO, 6, [1.0, 0.0, 2.0], material(1, 0.0, 0.9), None);
    let mut trunk = Vec::new();
    frustum(&mut trunk, Vec3::ZERO, 8, [0.5, 0.4, 3.0], material(1, 0.0, 0.9), Some(Vec3::new(0.1, 0.3, 0.0)));
    for mesh in [&cone, &trunk] {
        for points in triangles(mesh) {
            let (n, c) = (normal(points), (points[0] + points[1] + points[2]) / 3.0);
            assert!(n.y > 0.0 || n.dot(Vec3::new(c.x, 0.0, c.z)) > 0.0, "inward triangle {points:?}");
        }
    }
    let mut lump = Vec::new();
    blob(&mut lump, Vec3::new(0.0, 1.0, 0.0), Vec3::new(1.0, 0.8, 1.0), 9, material(2, 0.0, 0.9));
    assert_eq!(lump.len() / 3, 20);
    for points in triangles(&lump) {
        let c = (points[0] + points[1] + points[2]) / 3.0;
        assert!(normal(points).dot(c - Vec3::new(0.0, 1.0, 0.0)) > 0.0, "inward triangle {points:?}");
    }
}

#[test]
fn the_ground_the_river_and_the_ring_face_the_viewer() {
    let (_, data) = data();
    // Every ground triangle and every water strip faces up.
    let (solid, water) = (&data.statics.solid, &data.statics.water);
    for vertex in data.statics.vertices.get(water.start as usize..water.end as usize).unwrap_or(&[]) {
        assert!(vertex.normal[1] > 0.0, "a strip of water faces down");
        assert!((0.0..=1.0).contains(&vertex.material[3]), "edge {}", vertex.material[3]);
    }
    let ground = data.statics.vertices.get(solid.start as usize..solid.start as usize + 3 * 12_800).unwrap_or(&[]);
    assert!(triangles(ground).all(|points| normal(points).y > 0.0));
    // The ring is in front of the camera and faces it.
    let eye = Vec3::new(3.0, 5.0, 40.0);
    let mut ring = Vec::new();
    sky::vertices(&mut ring, eye);
    assert!(ring.len() > 100 && ring.iter().all(|vertex| vertex.material[3] == 1.0 && vertex.material[1] > 0.0));
    for points in triangles(&ring) {
        let centre = (points[0] + points[1] + points[2]) / 3.0;
        assert!(normal(points).dot(eye - centre) > 0.0, "the ring turns its back");
        assert!((centre - eye).length() > 250.0);
    }
    assert!(ring.len() / 3 < 400);
    let mut none = Vec::new();
    sky::vertices(&mut none, Vec3::new(f32::NAN, 0.0, 0.0));
    assert!(none.is_empty());
}

#[test]
fn the_backdrop_is_flagged_so_the_fog_does_not_swallow_it() {
    let (_, data) = data();
    let range = data.statics.backdrop.clone();
    let backdrop = data.statics.vertices.get(range.start as usize..range.end as usize).unwrap_or(&[]);
    assert!(!backdrop.is_empty() && backdrop.iter().all(|vertex| vertex.material[3] == 1.0));
    let solid = data.statics.vertices.get(..range.start as usize).unwrap_or(&[]);
    assert!(solid.iter().all(|vertex| vertex.material[3] == 0.0));
}

#[test]
fn the_grass_tuft_is_three_blades_that_lean_and_are_lit_from_above() {
    let tuft = grass::tuft_mesh();
    assert_eq!(tuft.len() as u32, TUFT_VERTICES);
    for vertex in &tuft {
        assert!((0.0..=1.0 + 1e-6).contains(&vertex.position[1]));
        assert!(vertex.normal[1] > 0.8, "{:?}", vertex.normal);
        assert!(vertex.position[0].abs() < 0.6 && vertex.position[2].abs() < 0.6);
    }
}

fn foe_pose(kind: figcraft_game::tps::FoeKind) -> figcraft_game::tps::FoePose {
    figcraft_game::tps::FoePose { kind, position: Vec3::new(3.0, 2.0, -4.0), facing: 0.7, ..Default::default() }
}

#[test]
fn husks_and_dummies_are_closed_boxes_standing_on_their_feet_and_only_the_husk_eyes_glow() {
    use figcraft_game::tps::{FoeKind, HUSK_HEIGHT};
    for kind in [FoeKind::Husk, FoeKind::Dummy] {
        let mut out = Vec::new();
        super::foes::foe(&mut out, &foe_pose(kind));
        assert!(!out.is_empty() && out.len() % 36 == 0, "{kind:?}: {} vertices", out.len());
        assert!(out.iter().all(|v| v.position.iter().chain(&v.normal).chain(&v.material).all(|x| x.is_finite())));
        let low = out.iter().map(|v| v.position[1]).fold(f32::MAX, f32::min);
        let high = out.iter().map(|v| v.position[1]).fold(f32::MIN, f32::max);
        assert!((low - 2.0).abs() < 0.05, "{kind:?} stands on its feet: {low}");
        assert!(high - 2.0 < HUSK_HEIGHT + 0.2, "{kind:?} is about as tall as it collides: {high}");
        let glowing = out.iter().filter(|v| v.material[1] > 0.0).count();
        assert_eq!(glowing > 0, kind == FoeKind::Husk, "{kind:?}");
        // Every box faces out of its middle.
        for (index, cube) in out.chunks_exact(36).enumerate() {
            let middle = cube.iter().fold(Vec3::ZERO, |sum, v| sum + Vec3::from_array(v.position)) / 36.0;
            for points in triangles(cube) {
                let centre = (points[0] + points[1] + points[2]) / 3.0;
                assert!(normal(points).dot(centre - middle) > -1e-4, "{kind:?} box {index} turned inside out");
            }
        }
    }
}

#[test]
fn a_broken_foe_falls_apart_and_is_gone_after_its_pieces_time() {
    use figcraft_game::tps::{FoeKind, PIECES_TIME};
    let whole = {
        let mut out = Vec::new();
        super::foes::foe(&mut out, &foe_pose(FoeKind::Husk));
        out
    };
    let mut pieces = Vec::new();
    super::foes::foe(&mut pieces, &figcraft_game::tps::FoePose { broken: Some(0.5), ..foe_pose(FoeKind::Husk) });
    assert_eq!(pieces.len(), whole.len(), "the same parts, flying apart");
    assert_ne!(pieces.first().map(|v| v.position), whole.first().map(|v| v.position));
    assert!(pieces.iter().all(|v| v.position[1] > 2.0 - 0.3), "pieces stay above the ground");
    let mut late = Vec::new();
    super::foes::foe(&mut late, &figcraft_game::tps::FoePose { broken: Some(PIECES_TIME * 0.9), ..foe_pose(FoeKind::Husk) });
    for (index, cube) in late.chunks_exact(36).enumerate() {
        let middle = cube.iter().map(|v| v.position[1]).sum::<f32>() / 36.0;
        assert!(middle > 2.0 - 0.05, "piece {index} lies on the ground, not in it: {middle}");
    }
    let mut gone = Vec::new();
    super::foes::foe(&mut gone, &figcraft_game::tps::FoePose { broken: Some(PIECES_TIME), ..foe_pose(FoeKind::Husk) });
    assert!(gone.is_empty());
    let mut junk = Vec::new();
    super::foes::foe(&mut junk, &figcraft_game::tps::FoePose { position: Vec3::NAN, ..foe_pose(FoeKind::Dummy) });
    super::foes::foe(&mut junk, &figcraft_game::tps::FoePose { broken: Some(f32::NAN), raise: f32::INFINITY, ..foe_pose(FoeKind::Dummy) });
    assert!(junk.iter().all(|v| v.position.iter().all(|x| x.is_finite())));
}

#[test]
fn arrows_point_where_they_fly_and_broken_ones_are_not_drawn() {
    use figcraft_game::tps::ArrowPose;
    let mut out = Vec::new();
    super::foes::arrow(&mut out, &ArrowPose { position: Vec3::new(0.0, 1.0, 0.0), direction: Vec3::new(0.0, 0.0, -2.0) });
    assert_eq!(out.len(), 72, "a shaft and a fletch");
    // All of it is behind the tip.
    assert!(out.iter().all(|v| v.position[2] > -0.05), "behind the tip, along +Z");
    assert!(out.iter().all(|v| v.position.iter().all(|x| x.is_finite())));
    for pose in [ArrowPose { position: Vec3::ONE, direction: Vec3::ZERO }, ArrowPose { position: Vec3::NAN, direction: Vec3::X }] {
        let mut none = Vec::new();
        super::foes::arrow(&mut none, &pose);
        assert!(none.is_empty(), "{pose:?}");
    }
}

#[test]
fn the_frame_draws_the_valley_foes_and_arrows() {
    use figcraft_game::tps::Tps;
    let tps = Tps::new();
    let frame = tps.frame();
    assert!(frame.foe_count >= 6, "the valley has dummies and husks: {}", frame.foe_count);
    let draw = |frame: &figcraft_game::tps::Frame, foes: usize| {
        let mut vertices = Vec::new();
        super::dynamic(
            &mut vertices,
            &super::Draw {
                pose: frame.hero,
                eye: frame.eye,
                time: 0.0,
                look: super::look::TpsLook::default(),
                foes: frame.foes,
                foe_count: foes,
                arrows: frame.arrows,
                arrow_count: frame.arrow_count,
            },
        );
        vertices.len()
    };
    assert!(draw(&frame, frame.foe_count) > draw(&frame, 0) + frame.foe_count * 36 * 6);
}
