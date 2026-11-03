use figcraft_game::arena::{Primitive, SceneObject};
use glam::{EulerRot, Mat4, Quat, Vec3};

#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub(super) struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    // tone, emissive intensity, roughness, viewmodel flag.
    pub material: [f32; 4],
}

pub(super) const ATTRIBUTES: [egui_wgpu::wgpu::VertexAttribute; 3] =
    egui_wgpu::wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x4];

pub(super) fn transform(position: Vec3, rotation: Vec3) -> Mat4 {
    Mat4::from_rotation_translation(Quat::from_euler(EulerRot::XYZ, rotation.x, rotation.y, rotation.z), position)
}

fn triangle(vertices: &mut Vec<Vertex>, points: [Vec3; 3], matrix: Mat4, material: [f32; 4]) {
    let points = points.map(|point| matrix.transform_point3(point));
    let normal = (points[1] - points[0]).cross(points[2] - points[0]).normalize_or_zero();
    vertices.extend(points.map(|point| Vertex { position: point.to_array(), normal: normal.to_array(), material }));
}

pub(super) fn object(vertices: &mut Vec<Vertex>, object: &SceneObject, parent: Mat4, viewmodel: bool) {
    let matrix = parent * transform(object.position, object.rotation) * Mat4::from_scale(object.size);
    let material = [object.tone.min(5) as f32, object.emissive, object.roughness, if viewmodel { 1.0 } else { 0.0 }];
    match object.primitive {
        Primitive::Box => {
            for (normal, u, v) in [
                (Vec3::X, -Vec3::Z, Vec3::Y),
                (-Vec3::X, Vec3::Z, Vec3::Y),
                (Vec3::Y, Vec3::X, -Vec3::Z),
                (-Vec3::Y, Vec3::X, Vec3::Z),
                (Vec3::Z, Vec3::X, Vec3::Y),
                (-Vec3::Z, -Vec3::X, Vec3::Y),
            ] {
                let center = normal * 0.5;
                let a = center - u * 0.5 - v * 0.5;
                let b = center + u * 0.5 - v * 0.5;
                let c = center + u * 0.5 + v * 0.5;
                let d = center - u * 0.5 + v * 0.5;
                triangle(vertices, [a, b, c], matrix, material);
                triangle(vertices, [a, c, d], matrix, material);
            }
        }
        Primitive::Cylinder { sides } => {
            let sides = sides.clamp(3, 32);
            for index in 0..sides {
                let a = index as f32 * std::f32::consts::TAU / sides as f32;
                let b = (index + 1) as f32 * std::f32::consts::TAU / sides as f32;
                let p = Vec3::new(a.cos() * 0.5, -0.5, a.sin() * 0.5);
                let q = Vec3::new(b.cos() * 0.5, -0.5, b.sin() * 0.5);
                let r = q + Vec3::Y;
                let s = p + Vec3::Y;
                triangle(vertices, [p, s, r], matrix, material);
                triangle(vertices, [p, r, q], matrix, material);
                triangle(vertices, [Vec3::Y * 0.5, r, s], matrix, material);
                triangle(vertices, [-Vec3::Y * 0.5, p, q], matrix, material);
            }
        }
        Primitive::Octahedron => {
            for index in 0..4 {
                let a = index as f32 * std::f32::consts::FRAC_PI_2;
                let b = (index + 1) as f32 * std::f32::consts::FRAC_PI_2;
                let p = Vec3::new(a.cos(), 0.0, a.sin()) * 0.5;
                let q = Vec3::new(b.cos(), 0.0, b.sin()) * 0.5;
                triangle(vertices, [Vec3::Y * 0.5, q, p], matrix, material);
                triangle(vertices, [-Vec3::Y * 0.5, p, q], matrix, material);
            }
        }
        Primitive::Banner => {
            let points = [
                Vec3::new(-0.5, 0.5, 0.0),
                Vec3::new(0.5, 0.5, 0.0),
                Vec3::new(0.5, -0.5, 0.0),
                Vec3::new(0.0, -0.35, 0.0),
                Vec3::new(-0.5, -0.5, 0.0),
            ];
            for points in [[points[0], points[3], points[1]], [points[1], points[3], points[2]], [points[0], points[4], points[3]]] {
                triangle(vertices, points, matrix, material);
                triangle(vertices, [points[2], points[1], points[0]], matrix, material);
            }
        }
    }
}

pub(super) fn part(position: [f32; 3], size: [f32; 3], tone: u32, rotation: [f32; 3]) -> SceneObject {
    SceneObject {
        position: Vec3::from_array(position),
        size: Vec3::from_array(size),
        rotation: Vec3::from_array(rotation),
        tone,
        emissive: 0.0,
        roughness: 0.9,
        collision: false,
        primitive: Primitive::Box,
    }
}

pub(super) fn pistol() -> Vec<Vertex> {
    let mut vertices = Vec::with_capacity(1100);
    let parts = [
        part([0.0, 0.031, 0.035], [0.062, 0.058, 0.13], 2, [0.0; 3]),
        part([0.0, 0.035, -0.115], [0.057, 0.05, 0.17], 2, [0.0; 3]),
        part([0.0, 0.008, -0.172], [0.059, 0.016, 0.055], 1, [0.0; 3]),
        part([0.0, 0.056, -0.06], [0.059, 0.012, 0.06], 0, [0.0; 3]),
        part([0.0, 0.066, -0.188], [0.01, 0.013, 0.018], 0, [-0.3, 0.0, 0.0]),
        part([-0.012, 0.066, 0.088], [0.011, 0.013, 0.012], 0, [0.0; 3]),
        part([0.012, 0.066, 0.088], [0.011, 0.013, 0.012], 0, [0.0; 3]),
        part([0.0, 0.05, 0.106], [0.018, 0.024, 0.022], 0, [-0.45, 0.0, 0.0]),
        part([0.0, -0.012, -0.04], [0.054, 0.028, 0.2], 1, [0.0; 3]),
        part([0.0, -0.06, -0.046], [0.016, 0.008, 0.072], 1, [0.08, 0.0, 0.0]),
        part([0.0, -0.042, -0.083], [0.016, 0.042, 0.009], 1, [0.35, 0.0, 0.0]),
        part([0.0, -0.04, -0.036], [0.008, 0.032, 0.009], 0, [-0.35, 0.0, 0.0]),
        part([0.0, -0.022, 0.084], [0.052, 0.03, 0.03], 1, [-0.25, 0.0, 0.0]),
    ];
    for mut piece in parts {
        piece.roughness = match piece.tone {
            2 => 0.55,
            1 => 0.7,
            _ => 0.6,
        };
        object(&mut vertices, &piece, Mat4::IDENTITY, true);
    }
    for z in [0.032, 0.046, 0.06, 0.074] {
        object(&mut vertices, &part([0.0, 0.032, z], [0.0635, 0.036, 0.005], 0, [0.38, 0.0, 0.0]), Mat4::IDENTITY, true);
    }
    let mut barrel = part([0.0, 0.03, -0.204], [0.026, 0.014, 0.026], 0, [std::f32::consts::FRAC_PI_2, 0.0, 0.0]);
    barrel.primitive = Primitive::Cylinder { sides: 8 };
    object(&mut vertices, &barrel, Mat4::IDENTITY, true);
    let grip = transform(Vec3::new(0.0, -0.086, 0.054), Vec3::new(-0.3, 0.0, 0.0));
    object(&mut vertices, &part([0.0; 3], [0.056, 0.14, 0.07], 1, [0.0; 3]), grip, true);
    for y in [0.036, 0.009, -0.018, -0.045] {
        object(&mut vertices, &part([0.0, y, -0.034], [0.06, 0.016, 0.016], 0, [0.0; 3]), grip, true);
    }
    object(&mut vertices, &part([0.0, -0.074, 0.003], [0.06, 0.012, 0.08], 0, [0.0; 3]), grip, true);
    vertices
}
