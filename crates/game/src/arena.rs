//! Original primitive arena shared by collision detection and the scene renderer.
use glam::{EulerRot, Quat, Vec3};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Primitive {
    Box,
    Cylinder { sides: u32 },
    Octahedron,
    Banner,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneObject {
    pub position: Vec3,
    pub size: Vec3,
    pub rotation: Vec3,
    pub tone: u32,
    pub emissive: f32,
    pub roughness: f32,
    pub collision: bool,
    pub primitive: Primitive,
}

impl SceneObject {
    fn block(position: Vec3, size: Vec3, tone: u32) -> Self {
        Self { position, size, rotation: Vec3::ZERO, tone, emissive: 0.0, roughness: 0.9, collision: true, primitive: Primitive::Box }
    }

    fn glow(position: Vec3, size: Vec3, emissive: f32) -> Self {
        Self { emissive, collision: false, ..Self::block(position, size, 0) }
    }

    pub(crate) fn is_ramp(&self) -> bool {
        self.collision && self.rotation.x != 0.0
    }
}

/// All dimensions are full extents, with Euler XYZ rotations in radians.
/// The ramp's sign and endpoints connect the floor to the platform in right-handed Y-up space.
pub fn objects() -> Vec<SceneObject> {
    let mut out = Vec::with_capacity(180);
    let mut floor = SceneObject::block(Vec3::new(0.0, -0.5, -10.0), Vec3::new(40.0, 1.0, 62.0), 1);
    floor.roughness = 0.35;
    out.push(floor);
    for x in [-20.5, 20.5] {
        out.push(SceneObject::block(Vec3::new(x, 9.0, -10.0), Vec3::new(1.0, 18.0, 62.0), 2));
    }
    for x in [-11.05, 11.05] {
        out.push(SceneObject::block(Vec3::new(x, 9.0, -40.5), Vec3::new(17.9, 18.0, 1.0), 2));
    }
    out.push(SceneObject::block(Vec3::new(0.0, 13.25, -40.5), Vec3::new(4.2, 9.5, 1.0), 2));
    out.push(SceneObject::block(Vec3::new(0.0, 9.0, 21.5), Vec3::new(42.0, 18.0, 1.0), 2));
    let mut ceiling = SceneObject::block(Vec3::new(0.0, 18.5, -10.0), Vec3::new(42.0, 1.0, 64.0), 0);
    ceiling.collision = false;
    out.push(ceiling);
    for x in [-9.0_f32, 9.0] {
        for z in [6.0, -6.0, -18.0, -30.0] {
            out.push(SceneObject::block(Vec3::new(x, 7.0, z), Vec3::new(2.4, 14.0, 2.4), 3));
            out.push(SceneObject::block(Vec3::new(x, 0.7, z), Vec3::new(3.4, 1.4, 3.4), 2));
            let mut cap = SceneObject::block(Vec3::new(x, 13.4, z), Vec3::new(3.0, 1.2, 3.0), 2);
            cap.collision = false;
            out.push(cap);
            out.push(SceneObject::glow(Vec3::new(x - x.signum() * 1.71, 0.8, z), Vec3::new(0.02, 0.5, 0.18), 1.6));
        }
    }
    for i in 0..10 {
        let h = 0.4 * (i + 1) as f32;
        out.push(SceneObject::block(Vec3::new(15.0, h * 0.5, -2.5 - i as f32), Vec3::new(5.0, h, 1.0), 4));
    }
    out.push(SceneObject::block(Vec3::new(15.0, 2.0, -17.0), Vec3::new(10.0, 4.0, 10.0), 3));
    out.push(SceneObject::block(Vec3::new(11.2, 4.5, -17.0), Vec3::new(0.4, 1.0, 10.0), 2));
    let angle = 3.0_f32.atan2(11.0);
    let mut ramp = SceneObject::block(Vec3::new(-15.0, 1.5 - 0.2 / angle.cos(), -2.5), Vec3::new(4.0, 0.4, 130.0_f32.sqrt()), 4);
    ramp.rotation.x = angle;
    out.push(ramp);
    out.push(SceneObject::block(Vec3::new(-15.0, 1.5, -12.0), Vec3::new(6.0, 3.0, 8.0), 3));
    for (x, y, z, s) in [
        (-5.0, 0.0, 2.0, 2.0),
        (-3.0, 0.0, 2.0, 2.0),
        (5.0, 0.0, -10.0, 2.0),
        (-6.0, 0.0, -22.0, 1.6),
        (3.0, 0.0, -28.0, 2.0),
        (16.0, 4.0, -20.0, 1.6),
    ] {
        out.push(SceneObject::block(Vec3::new(x, y + s * 0.5, z), Vec3::splat(s), 2));
        for side in [-1.0, 1.0] {
            for angle in [-std::f32::consts::FRAC_PI_4, std::f32::consts::FRAC_PI_4] {
                let mut brace = SceneObject::block(Vec3::new(x, y + s * 0.5, z + side * (s * 0.5 + 0.01)), Vec3::new(s * 1.25, 0.16, 0.04), 1);
                brace.rotation.z = angle;
                brace.collision = false;
                out.push(brace);
            }
        }
    }
    let mut seed = 7_u32;
    for _ in 0..40 {
        let side = 0.15 + random(&mut seed) * 0.45;
        let height = side * (0.5 + random(&mut seed) * 0.5);
        let mut rubble = SceneObject::block(
            Vec3::new((random(&mut seed) * 2.0 - 1.0) * 17.0, height * 0.5, -35.0 + random(&mut seed) * 50.0),
            Vec3::new(side, height, side),
            4,
        );
        rubble.rotation.y = random(&mut seed) * std::f32::consts::TAU;
        rubble.collision = false;
        out.push(rubble);
    }
    out.push(SceneObject::glow(Vec3::new(0.0, 4.0, -40.6), Vec3::new(4.2, 8.6, 0.1), 2.0));
    for x in [-1.9_f32, 1.9] {
        let mut arch = SceneObject::block(Vec3::new(x, 7.6, -40.0), Vec3::new(2.4, 1.6, 1.0), 2);
        arch.rotation.z = -x.signum() * std::f32::consts::PI / 4.5;
        arch.collision = false;
        out.push(arch);
    }
    let mut banner = SceneObject::glow(Vec3::new(-19.9, 8.0, -14.0), Vec3::new(1.6, 4.0, 0.02), 0.9);
    banner.rotation.y = std::f32::consts::FRAC_PI_2;
    banner.primitive = Primitive::Banner;
    out.push(banner);
    let mut pole = SceneObject::block(Vec3::new(-19.85, 10.05, -14.0), Vec3::new(2.2, 0.08, 0.08), 1);
    pole.rotation.y = std::f32::consts::FRAC_PI_2;
    pole.collision = false;
    out.push(pole);
    let mut stripe = SceneObject::glow(Vec3::new(-6.0, 0.02, -26.0), Vec3::new(0.14, 0.02, 30.0), 1.8);
    stripe.rotation.y = 0.42;
    out.push(stripe);
    for i in 0..3 {
        for j in 0..3 {
            out.push(SceneObject::glow(Vec3::new(19.95, 11.0 + 0.8 * j as f32, -26.0 + 0.8 * i as f32), Vec3::new(0.02, 0.45, 0.45), 2.0));
        }
    }
    for (position, size) in [
        (Vec3::new(-19.95, 5.0, 0.0), Vec3::new(0.02, 2.2, 0.25)),
        (Vec3::new(19.95, 4.0, 8.0), Vec3::new(0.02, 1.6, 0.2)),
        (Vec3::new(-4.0, 12.0, -39.95), Vec3::new(0.3, 1.6, 0.02)),
    ] {
        out.push(SceneObject::glow(position, size, 1.6));
    }
    out
}

pub(crate) fn random(seed: &mut u32) -> f32 {
    *seed = (seed.wrapping_mul(9301).wrapping_add(49297)) % 233280;
    *seed as f32 / 233280.0
}

/// Nearest collidable surface on a ray, including the rotated ramp. Direction is normalized.
pub fn raycast(objects: &[SceneObject], origin: Vec3, direction: Vec3, limit: f32) -> Option<f32> {
    let length_squared = direction.length_squared();
    if !origin.is_finite() || !direction.is_finite() || !limit.is_finite() || limit < 0.0 || !length_squared.is_finite() || length_squared < 1e-12 {
        return None;
    }
    let direction = direction / length_squared.sqrt();
    objects
        .iter()
        .filter(|object| object.collision)
        .filter_map(|object| {
            let relative = origin - object.position;
            let (origin, direction) = if object.rotation == Vec3::ZERO {
                (relative, direction)
            } else {
                let inverse = Quat::from_euler(EulerRot::XYZ, object.rotation.x, object.rotation.y, object.rotation.z).conjugate();
                (inverse * relative, inverse * direction)
            };
            ray_box(origin, direction, -object.size * 0.5, object.size * 0.5, limit)
        })
        .min_by(f32::total_cmp)
}

pub(crate) fn ray_box(origin: Vec3, direction: Vec3, min: Vec3, max: Vec3, limit: f32) -> Option<f32> {
    let mut near = 0.0_f32;
    let mut far = limit;
    for axis in 0..3 {
        let o = origin[axis];
        let d = direction[axis];
        if d.abs() < 1e-7 {
            if o < min[axis] || o > max[axis] {
                return None;
            }
        } else {
            let a = (min[axis] - o) / d;
            let b = (max[axis] - o) / d;
            near = near.max(a.min(b));
            far = far.min(a.max(b));
            if near > far {
                return None;
            }
        }
    }
    Some(near)
}
