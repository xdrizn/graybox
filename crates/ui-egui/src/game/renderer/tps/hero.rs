//! The hero (G11.3): an original faceted, hooded adventurer with a cloak, built in code from
//! prisms and boxes, one group of meshes per bone of the rigid-part skeleton
//! (`figcraft_game::tps::rig`). Each frame every bone's meshes are moved by that bone's transform
//! into the dynamic vertex buffer, which is drawn in one call with the rest of the moving world
//! (and casts its shadow with it).
//!
//! Weapons are small meshes in grip space (the grip at the origin, the weapon along +Y): on the
//! hand bone when drawn (the bow in the left hand), on the back when not. The bowstring follows the
//! drawing hand.
use super::super::mesh::{self, Vertex};
use figcraft_game::arena::Primitive;
use figcraft_game::tps::rig::{BONES, Bone, HEEL, TOE};
use figcraft_game::tps::{HeroPose, Weapon};
use glam::{Mat4, Quat, Vec2, Vec3};
use std::sync::OnceLock;

// Grey tones of the theme (0 darkest … 5 lightest).
const SKIN: u32 = 1;
const CLOTH: u32 = 2;
const TUNIC: u32 = 3;
const LEATHER: u32 = 1;
const METAL: u32 = 5;
const WOOD: u32 = 2;
const TRIM: u32 = 4;

#[derive(Default)]
struct Model {
    bones: Vec<Vec<Vertex>>,
    weapons: Vec<(Weapon, Vec<Vertex>)>,
}

fn model() -> &'static Model {
    static MODEL: OnceLock<Model> = OnceLock::new();
    MODEL.get_or_init(build)
}

/// One end of a prism: its height, radii (x, z) and centre (x, z).
#[derive(Clone, Copy)]
struct Ring {
    y: f32,
    radius: Vec2,
    centre: Vec2,
}

fn ring(y: f32, radius: Vec2, centre: Vec2) -> Ring {
    Ring { y, radius, centre }
}

/// Bone-local geometry, added a part at a time.
#[derive(Default)]
struct Mesh {
    out: Vec<Vertex>,
}

fn material(tone: u32, roughness: f32) -> [f32; 4] {
    [tone.min(5) as f32, 0.0, roughness, 0.0]
}

impl Mesh {
    fn triangle(&mut self, points: [Vec3; 3], material: [f32; 4]) {
        let normal = (points[1] - points[0]).cross(points[2] - points[0]).normalize_or_zero();
        self.out.extend(points.map(|point| Vertex { position: point.to_array(), normal: normal.to_array(), material }));
    }

    fn quad(&mut self, a: Vec3, b: Vec3, c: Vec3, d: Vec3, material: [f32; 4]) {
        self.triangle([a, b, c], material);
        self.triangle([a, c, d], material);
    }

    /// A faceted prism along Y from `y0` to `y1`: `sides` facets, elliptical ends of radii
    /// (x, z), each end centred at (x, z) `centre0` / `centre1`. A radius of zero makes a point.
    fn prism(&mut self, sides: u32, bottom: Ring, top: Ring, tone: u32) {
        let (y0, radius0, centre0) = (bottom.y, bottom.radius, bottom.centre);
        let (y1, radius1, centre1) = (top.y, top.radius, top.centre);
        let material = material(tone, 0.85);
        let sides = sides.clamp(3, 16);
        let ring = |y: f32, radius: Vec2, centre: Vec2, index: u32| {
            let angle = (index as f32 + 0.5) * std::f32::consts::TAU / sides as f32;
            Vec3::new(centre.x + radius.x * angle.cos(), y, centre.y + radius.y * angle.sin())
        };
        let bottom_middle = Vec3::new(centre0.x, y0, centre0.y);
        let top_middle = Vec3::new(centre1.x, y1, centre1.y);
        for index in 0..sides {
            let (a, b) = (ring(y0, radius0, centre0, index), ring(y0, radius0, centre0, index + 1));
            let (c, d) = (ring(y1, radius1, centre1, index + 1), ring(y1, radius1, centre1, index));
            // Outward-facing: y0 below y1.
            if radius1.length_squared() > 1e-8 {
                self.quad(a, d, c, b, material);
                self.triangle([top_middle, c, d], material);
            } else {
                self.triangle([a, top_middle, b], material);
            }
            if radius0.length_squared() > 1e-8 {
                self.triangle([bottom_middle, a, b], material);
            }
        }
    }

    /// A plain prism of round section, centred on the Y axis.
    fn limb(&mut self, sides: u32, y0: f32, y1: f32, r0: f32, r1: f32, tone: u32) {
        self.prism(sides, ring(y0, Vec2::splat(r0), Vec2::ZERO), ring(y1, Vec2::splat(r1), Vec2::ZERO), tone);
    }

    /// A box or other primitive of the arena's kind (full sizes, Euler XYZ radians).
    fn piece(&mut self, primitive: Primitive, centre: [f32; 3], size: [f32; 3], rotation: [f32; 3], tone: u32) {
        let mut part = mesh::part(centre, size, tone, rotation);
        part.primitive = primitive;
        part.roughness = 0.85;
        mesh::object(&mut self.out, &part, Mat4::IDENTITY, false);
    }

    fn cuboid(&mut self, centre: [f32; 3], size: [f32; 3], rotation: [f32; 3], tone: u32) {
        self.piece(Primitive::Box, centre, size, rotation, tone);
    }

    fn ball(&mut self, centre: [f32; 3], size: [f32; 3], tone: u32) {
        self.piece(Primitive::Octahedron, centre, size, [0.0; 3], tone);
    }

    /// A curved sheet hanging down (a cloak panel), seen from both sides: an arc of `segments`
    /// facets round the back (+Z) between `from` and `to` (radians), from radii `top` at y 0 to
    /// `bottom` at `depth`. Every other bottom point is a little higher: a ragged hem.
    fn sheet(&mut self, segments: u32, (from, to): (f32, f32), top: Ring, bottom: Ring, ragged: f32, (outer, inner): (u32, u32)) {
        let (centre, depth) = (top.centre, bottom.y);
        let (top, bottom) = (top.radius, bottom.radius);
        let segments = segments.clamp(1, 16);
        let point = |radius: Vec2, y: f32, index: u32| {
            let angle = from + (to - from) * index as f32 / segments as f32;
            Vec3::new(centre.x + radius.x * angle.cos(), y, centre.y + radius.y * angle.sin())
        };
        for index in 0..segments {
            let hem = |i: u32| depth + if i % 2 == 1 { ragged } else { 0.0 };
            let (a, b) = (point(top, 0.0, index), point(top, 0.0, index + 1));
            let (c, d) = (point(bottom, hem(index + 1), index + 1), point(bottom, hem(index), index));
            self.quad(a, b, c, d, material(outer, 0.9));
            self.quad(a, d, c, b, material(inner, 0.9));
        }
    }
}

/// The hand: a closed fist (glove) with a thumb on the inner, forward side.
fn hand(mesh: &mut Mesh, right: bool) {
    let inward = if right { -1.0 } else { 1.0 };
    mesh.cuboid([0.0, -0.05, -0.005], [0.07, 0.1, 0.075], [0.0; 3], LEATHER);
    mesh.cuboid([0.025 * inward, -0.035, -0.035], [0.025, 0.05, 0.025], [0.0, 0.0, -0.3 * inward], LEATHER);
}

fn bone_mesh(bone: Bone) -> Vec<Vertex> {
    let mut mesh = Mesh::default();
    let v = Vec2::new;
    match bone {
        Bone::Pelvis => {
            // Hips in trousers, a tunic hem over them, a belt with a buckle and a pouch.
            mesh.prism(6, ring(-0.14, v(0.15, 0.115), v(0.0, 0.0)), ring(0.04, v(0.15, 0.11), v(0.0, 0.0)), CLOTH);
            mesh.prism(6, ring(-0.22, v(0.19, 0.15), v(0.0, -0.005)), ring(0.0, v(0.16, 0.12), v(0.0, 0.0)), TUNIC);
            mesh.prism(8, ring(-0.005, v(0.168, 0.128), v(0.0, 0.0)), ring(0.045, v(0.165, 0.125), v(0.0, 0.0)), LEATHER);
            mesh.cuboid([0.0, 0.02, -0.128], [0.05, 0.042, 0.02], [0.0; 3], TRIM);
            mesh.cuboid([0.155, -0.04, -0.03], [0.05, 0.08, 0.08], [0.0, 0.0, 0.12], LEATHER);
        }
        Bone::Spine => {
            mesh.prism(6, ring(-0.06, v(0.155, 0.115), v(0.0, 0.0)), ring(0.19, v(0.17, 0.12), v(0.0, 0.0)), TUNIC);
        }
        Bone::Chest => {
            // Torso, a short neck, the hood's cowl over the shoulders, a strap for the back weapon.
            mesh.prism(6, ring(-0.01, v(0.17, 0.12), v(0.0, 0.0)), ring(0.2, v(0.205, 0.13), v(0.0, 0.01)), TUNIC);
            mesh.limb(6, 0.18, 0.28, 0.05, 0.048, SKIN);
            mesh.prism(8, ring(0.12, v(0.27, 0.19), v(0.0, 0.02)), ring(0.27, v(0.11, 0.1), v(0.0, 0.02)), CLOTH);
            mesh.cuboid([0.0, 0.08, -0.13], [0.035, 0.36, 0.02], [0.0, 0.0, -0.62], LEATHER);
            mesh.cuboid([0.0, 0.08, 0.15], [0.035, 0.36, 0.02], [0.0, 0.0, 0.62], LEATHER);
        }
        Bone::Head => {
            // A faceted head in the shadow of a pointed hood that leans back.
            mesh.prism(6, ring(0.0, v(0.06, 0.07), v(0.0, -0.005)), ring(0.12, v(0.085, 0.095), v(0.0, -0.01)), SKIN);
            mesh.prism(6, ring(0.12, v(0.085, 0.095), v(0.0, -0.01)), ring(0.23, v(0.07, 0.08), v(0.0, 0.0)), SKIN);
            mesh.cuboid([0.0, 0.115, -0.1], [0.025, 0.045, 0.03], [-0.3, 0.0, 0.0], SKIN);
            mesh.prism(6, ring(-0.01, v(0.13, 0.125), v(0.0, 0.04)), ring(0.2, v(0.125, 0.125), v(0.0, 0.035)), CLOTH);
            mesh.prism(6, ring(0.2, v(0.125, 0.125), v(0.0, 0.035)), ring(0.33, v(0.0, 0.0), v(0.0, 0.15)), CLOTH);
            mesh.cuboid([0.0, 0.215, -0.07], [0.2, 0.04, 0.06], [-0.35, 0.0, 0.0], CLOTH);
            mesh.cuboid([0.105, 0.1, -0.06], [0.03, 0.2, 0.06], [0.0, 0.0, 0.08], CLOTH);
            mesh.cuboid([-0.105, 0.1, -0.06], [0.03, 0.2, 0.06], [0.0, 0.0, -0.08], CLOTH);
        }
        Bone::CloakUpper => {
            // Hung from the top of the back (the bone sits 0.1 behind the chest's middle).
            mesh.sheet(
                6,
                (0.15, std::f32::consts::PI - 0.15),
                ring(0.0, v(0.22, 0.13), v(0.0, -0.1)),
                ring(-0.46, v(0.27, 0.19), v(0.0, -0.1)),
                0.0,
                (CLOTH, 1),
            );
        }
        Bone::CloakLower => {
            mesh.sheet(
                6,
                (0.15, std::f32::consts::PI - 0.15),
                ring(0.0, v(0.27, 0.19), v(0.0, -0.15)),
                ring(-0.42, v(0.31, 0.22), v(0.0, -0.15)),
                0.07,
                (CLOTH, 1),
            );
        }
        Bone::UpperArmL | Bone::UpperArmR => {
            mesh.ball([0.0, -0.01, 0.0], [0.14, 0.13, 0.14], TUNIC);
            mesh.limb(6, -0.27, 0.0, 0.052, 0.064, TUNIC);
        }
        Bone::ForearmL | Bone::ForearmR => {
            mesh.ball([0.0, 0.0, 0.0], [0.1, 0.1, 0.1], TUNIC);
            mesh.limb(6, -0.12, 0.0, 0.048, 0.052, TUNIC);
            mesh.limb(6, -0.245, -0.1, 0.044, 0.056, LEATHER);
        }
        Bone::HandL => hand(&mut mesh, false),
        Bone::HandR => hand(&mut mesh, true),
        Bone::ThighL | Bone::ThighR => {
            mesh.ball([0.0, 0.0, 0.0], [0.17, 0.14, 0.17], CLOTH);
            mesh.limb(6, -0.42, 0.0, 0.062, 0.086, CLOTH);
        }
        Bone::ShinL | Bone::ShinR => {
            mesh.ball([0.0, 0.0, -0.01], [0.12, 0.12, 0.12], LEATHER);
            mesh.limb(6, -0.2, 0.0, 0.052, 0.06, CLOTH);
            mesh.limb(6, -0.4, -0.17, 0.056, 0.066, LEATHER);
            mesh.cuboid([0.0, -0.17, 0.0], [0.13, 0.03, 0.13], [0.0, 0.0, 0.0], LEATHER);
        }
        Bone::FootL | Bone::FootR => {
            // A boot whose sole runs from the heel to the toe, flat on the ground at rest.
            let length = TOE + HEEL;
            mesh.cuboid([0.0, -0.038, (HEEL - TOE) * 0.5], [0.1, 0.045, length], [0.0; 3], LEATHER);
            mesh.cuboid([0.0, 0.0, -0.045], [0.09, 0.06, 0.14], [0.35, 0.0, 0.0], LEATHER);
            mesh.limb(6, -0.03, 0.04, 0.058, 0.055, LEATHER);
        }
    }
    mesh.out
}

fn weapon_mesh(weapon: Weapon) -> Vec<Vertex> {
    let mut mesh = Mesh::default();
    let v = Vec2::new;
    let blade = |mesh: &mut Mesh, y0: f32, y1: f32, tip: f32, width: f32, thickness: f32| {
        // A diamond section (four facets) narrowing a little, then a point.
        mesh.prism(4, ring(y0, v(width, thickness), v(0.0, 0.0)), ring(y1, v(width * 0.85, thickness * 0.85), v(0.0, 0.0)), METAL);
        mesh.prism(4, ring(y1, v(width * 0.85, thickness * 0.85), v(0.0, 0.0)), ring(tip, v(0.0, 0.0), v(0.0, 0.0)), METAL);
    };
    match weapon {
        Weapon::None => {}
        Weapon::Sword => {
            mesh.ball([0.0, -0.11, 0.0], [0.045, 0.045, 0.045], TRIM);
            mesh.limb(6, -0.095, 0.075, 0.017, 0.019, LEATHER);
            mesh.cuboid([0.0, 0.088, 0.0], [0.2, 0.026, 0.04], [0.0; 3], TUNIC);
            blade(&mut mesh, 0.1, 0.76, 0.9, 0.03, 0.008);
        }
        Weapon::Spear => {
            mesh.ball([0.0, -0.77, 0.0], [0.04, 0.05, 0.04], TUNIC);
            mesh.limb(6, -0.76, 1.13, 0.016, 0.016, WOOD);
            mesh.limb(6, 1.1, 1.19, 0.022, 0.02, LEATHER);
            blade(&mut mesh, 1.19, 1.33, 1.5, 0.042, 0.01);
        }
        Weapon::Club => {
            mesh.limb(6, -0.12, 0.28, 0.022, 0.026, LEATHER);
            mesh.limb(6, 0.25, 0.6, 0.05, 0.085, WOOD);
            mesh.prism(6, ring(0.6, v(0.085, 0.085), v(0.0, 0.0)), ring(0.67, v(0.04, 0.04), v(0.0, 0.0)), WOOD);
            for index in 0..5 {
                let angle = index as f32 * std::f32::consts::TAU / 5.0;
                mesh.ball([angle.cos() * 0.08, 0.5, angle.sin() * 0.08], [0.035, 0.035, 0.035], TRIM);
            }
        }
        Weapon::Bow => {
            // A D-shaped bow: the grip furthest from the string, the tips coming back to it.
            mesh.cuboid([0.0, 0.0, 0.0], [0.034, 0.13, 0.042], [0.0; 3], LEATHER);
            let curve = [v(0.06, 0.0), v(0.3, 0.035), v(0.5, 0.085), v(0.63, 0.15)];
            for side in [1.0_f32, -1.0] {
                for pair in curve.windows(2) {
                    let (a, b) = (Vec3::new(0.0, pair[0].x * side, pair[0].y), Vec3::new(0.0, pair[1].x * side, pair[1].y));
                    let middle = (a + b) * 0.5;
                    let along = b - a;
                    let tilt = along.z.atan2(along.y);
                    let thick = 0.034 - 0.012 * (pair[1].x / 0.63);
                    mesh.cuboid(middle.to_array(), [thick, along.length() + 0.01, thick * 1.2], [-tilt, 0.0, 0.0], WOOD);
                }
                mesh.ball([0.0, 0.63 * side, 0.15], [0.03, 0.04, 0.03], TRIM);
            }
        }
    }
    mesh.out
}

fn build() -> Model {
    Model {
        bones: Bone::ALL.iter().map(|bone| bone_mesh(*bone)).collect(),
        weapons: [Weapon::Sword, Weapon::Spear, Weapon::Club, Weapon::Bow].into_iter().map(|weapon| (weapon, weapon_mesh(weapon))).collect(),
    }
}

/// Where the bowstring meets each tip, in bow space.
const BOW_TIP: Vec3 = Vec3::new(0.0, 0.63, 0.15);

/// Grip on the hand: the weapon's +Y comes out of the fist forward (the hand's -Z), tipped
/// towards the line of the forearm (the hand's -Y) by a angle that suits the weapon: a sword
/// hangs forward and down from a lowered arm and stands up in a guard, a spear lies nearly along
/// the arm, the bow stands across it.
fn hand_socket(weapon: Weapon) -> Mat4 {
    let along: f32 = match weapon {
        Weapon::Sword | Weapon::Club => 40.0,
        Weapon::Spear => 50.0,
        Weapon::Bow | Weapon::None => 0.0,
    };
    Mat4::from_translation(Vec3::new(0.0, -0.06, -0.005)) * Mat4::from_rotation_x(-(90.0 + along).to_radians())
}

/// Where each weapon hangs on the back, in chest space (+Z is behind the hero).
fn back_socket(weapon: Weapon) -> Mat4 {
    let (at, turn, spin) = match weapon {
        Weapon::Sword => (Vec3::new(0.15, 0.32, 0.25), 153_f32, 0.0_f32),
        Weapon::Spear => (Vec3::new(0.05, 0.12, 0.26), -28.0, 0.0),
        Weapon::Club => (Vec3::new(0.14, 0.3, 0.25), 168.0, 0.0),
        Weapon::Bow => (Vec3::new(-0.02, 0.1, 0.26), 34.0, 180.0),
        Weapon::None => (Vec3::ZERO, 0.0, 0.0),
    };
    Mat4::from_translation(at) * Mat4::from_rotation_z(turn.to_radians()) * Mat4::from_rotation_y(spin.to_radians())
}

fn push(out: &mut Vec<Vertex>, vertices: &[Vertex], matrix: Mat4) {
    out.extend(vertices.iter().map(|vertex| Vertex {
        position: matrix.transform_point3(Vec3::from_array(vertex.position)).to_array(),
        normal: matrix.transform_vector3(Vec3::from_array(vertex.normal)).normalize_or_zero().to_array(),
        material: vertex.material,
    }));
}

/// A thin straight piece from `a` to `b` (bow space), for the string and the arrow.
fn rod(out: &mut Vec<Vertex>, a: Vec3, b: Vec3, width: f32, tone: u32, matrix: Mat4) {
    let along = b - a;
    let length = along.length();
    if !length.is_finite() || length < 1e-4 {
        return;
    }
    let rotation = Quat::from_rotation_arc(Vec3::Y, along / length);
    let place = matrix * Mat4::from_rotation_translation(rotation, (a + b) * 0.5) * Mat4::from_scale(Vec3::new(width, length, width));
    let mut part = mesh::part([0.0; 3], [1.0; 3], tone, [0.0; 3]);
    part.roughness = 0.5;
    mesh::object(out, &part, place, false);
}

/// The bowstring (pulled towards the drawing hand) and, while drawing, the arrow.
fn bowstring(out: &mut Vec<Vertex>, bow: Mat4, draw: f32, hand: Option<Vec3>) {
    let draw = if draw.is_finite() { draw.clamp(0.0, 1.0) } else { 0.0 };
    let rest = Vec3::new(0.0, 0.0, BOW_TIP.z);
    let pulled = hand.filter(|at| at.is_finite() && at.length() < 1.0).unwrap_or(rest + Vec3::Z * 0.5);
    let nock = rest.lerp(pulled, draw);
    for tip in [BOW_TIP, BOW_TIP * Vec3::new(1.0, -1.0, 1.0)] {
        rod(out, tip, nock, 0.006, METAL, bow);
    }
    if draw > 0.05 {
        let forward = (Vec3::ZERO - nock).normalize_or_zero();
        let head = nock + forward * 0.8;
        rod(out, nock, head, 0.011, WOOD, bow);
        let mut point = mesh::part(head.to_array(), [0.03, 0.06, 0.03], METAL, [0.0; 3]);
        point.primitive = Primitive::Octahedron;
        let along = Mat4::from_rotation_translation(Quat::from_rotation_arc(Vec3::Y, forward), head) * Mat4::from_translation(-head);
        mesh::object(out, &point, bow * along, false);
        rod(out, nock + forward * 0.02, nock + forward * 0.12, 0.03, TRIM, bow);
    }
}

pub(super) fn vertices(out: &mut Vec<Vertex>, pose: &HeroPose) {
    if !pose.position.is_finite() || !pose.facing.is_finite() {
        return;
    }
    let model = model();
    // The model looks along -Z; a hero facing `f` is turned by -f about Y (the first-person
    // Sentinels use the same rule). The whole body tilts about the feet.
    let tilt = if pose.tilt.is_finite() { pose.tilt.normalize() } else { Quat::IDENTITY };
    let root = Mat4::from_translation(pose.position) * Mat4::from_rotation_y(-pose.facing) * Mat4::from_quat(tilt);
    let bones = pose.skeleton.model_matrices();
    for (index, vertices) in model.bones.iter().enumerate().take(BONES) {
        push(out, vertices, root * bones[index]);
    }
    let Some((weapon, vertices)) = model.weapons.iter().find(|(weapon, _)| *weapon == pose.weapon) else {
        return;
    };
    let holder = if *weapon == Weapon::Bow { Bone::HandL } else { Bone::HandR };
    let place = if pose.in_hand { bones[holder.index()] * hand_socket(*weapon) } else { bones[Bone::Chest.index()] * back_socket(*weapon) };
    push(out, vertices, root * place);
    if *weapon == Weapon::Bow {
        let draw = if pose.in_hand { pose.bow_draw } else { 0.0 };
        // The drawing hand, in bow space.
        let hand = place.inverse().transform_point3(bones[Bone::HandR.index()].transform_point3(Vec3::new(0.0, -0.06, 0.0)));
        bowstring(out, root * place, draw, Some(hand));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use figcraft_game::tps::Tps;

    #[test]
    fn the_hero_is_light_and_every_bone_has_a_mesh() {
        let model = model();
        assert_eq!(model.bones.len(), BONES);
        for (bone, vertices) in Bone::ALL.iter().zip(&model.bones) {
            assert!(!vertices.is_empty(), "{bone:?} has no mesh");
            assert_eq!(vertices.len() % 3, 0);
        }
        let total: usize = model.bones.iter().map(Vec::len).sum::<usize>() + model.weapons.iter().map(|(_, v)| v.len()).sum::<usize>();
        // Under 3000 triangles for the hero and all four weapons.
        assert!(total / 3 < 3000, "{} triangles", total / 3);
        assert_eq!(model.weapons.len(), 4);
    }

    #[test]
    fn the_hero_is_drawn_where_it_stands_with_finite_vertices() {
        let tps = Tps::new();
        let mut pose = tps.frame().hero;
        for weapon in [Weapon::Sword, Weapon::Spear, Weapon::Club, Weapon::Bow, Weapon::None] {
            for in_hand in [false, true] {
                pose.weapon = weapon;
                pose.in_hand = in_hand;
                pose.bow_draw = 1.0;
                let mut out = Vec::new();
                vertices(&mut out, &pose);
                assert!(out.iter().all(|v| v.position.iter().chain(&v.normal).all(|x| x.is_finite())));
                // Heights from the ground the hero stands on (the spawn is on the terrain).
                let ground = pose.position.y;
                let top = out.iter().map(|v| v.position[1] - ground).fold(f32::MIN, f32::max);
                let bottom = out.iter().map(|v| v.position[1] - ground).fold(f32::MAX, f32::min);
                assert!(bottom > -0.05 && bottom < 0.05, "feet on the ground: {bottom}");
                assert!(top > 1.6 && top < 3.0, "about 1.7 m tall (a spear held upright reaches higher): {top}");
            }
        }
        let mut junk = pose;
        junk.position = Vec3::NAN;
        let mut out = Vec::new();
        vertices(&mut out, &junk);
        assert!(out.is_empty());
    }
}
