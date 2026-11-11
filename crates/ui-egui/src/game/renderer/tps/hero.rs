//! The hero's meshes. PLACEHOLDER (G11.1): a faceted capsule (a cylinder with a point at each end)
//! with an orange visor to show which way it faces. G11.3 replaces this with the rig and its
//! animated parts, drawn from the same [`HeroPose`].
use super::super::mesh::{self, Vertex};
use figcraft_game::arena::Primitive;
use figcraft_game::tps::HeroPose;
use glam::{Mat4, Vec3};

pub(super) fn vertices(out: &mut Vec<Vertex>, pose: &HeroPose) {
    if !pose.position.is_finite() || !pose.facing.is_finite() {
        return;
    }
    // The model looks along -Z; a hero facing `f` is turned by -f about Y (the first-person
    // Sentinels use the same rule).
    let parent: Mat4 = mesh::transform(pose.position, Vec3::new(0.0, -pose.facing, 0.0));
    let mut body = mesh::part([0.0, 0.85, 0.0], [0.6, 1.1, 0.6], 3, [0.0; 3]);
    body.primitive = Primitive::Cylinder { sides: 8 };
    let mut top = mesh::part([0.0, 1.4, 0.0], [0.6, 0.6, 0.6], 3, [0.0; 3]);
    top.primitive = Primitive::Octahedron;
    let mut bottom = mesh::part([0.0, 0.3, 0.0], [0.6, 0.6, 0.6], 2, [0.0; 3]);
    bottom.primitive = Primitive::Octahedron;
    let mut visor = mesh::part([0.0, 1.3, -0.29], [0.36, 0.09, 0.05], 0, [0.0; 3]);
    visor.emissive = 1.6;
    for piece in [body, top, bottom, visor] {
        mesh::object(out, &piece, parent, false);
    }
}
