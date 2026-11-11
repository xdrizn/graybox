//! Plain-wgpu Greybox scene, sharing egui's device and submission lifecycle.
//!
//! The host calls [`configure`] once for each egui renderer (and again if its
//! surface configuration changes), then [`paint`] for the Play viewport.
mod gpu;
mod mesh;
// Graybox: the third-person scene (G11).
mod tps;

use egui_wgpu::{Callback, CallbackResources, CallbackTrait, ScreenDescriptor, wgpu};
use figcraft_game::Game;
use glam::{Mat4, Vec3};
use mesh::Vertex;

pub use tps::paint as paint_tps;

/// Scene palette and lighting. Color triples are sRGB; intensities are linear.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneTheme {
    pub grays: [[f32; 3]; 6],
    pub accent: [f32; 3],
    pub accent_emissive: f32,
    pub background: [f32; 3],
    pub fog_color: [f32; 3],
    pub fog_density: f32,
    pub ambient: f32,
    pub hemi_sky: [f32; 3],
    pub hemi_ground: [f32; 3],
    pub hemi_intensity: f32,
    pub key_position: Vec3,
    pub key_color: [f32; 3],
    pub key_intensity: f32,
    pub portal_position: Vec3,
    pub portal_intensity: f32,
    pub portal_range: f32,
    pub portal_decay: f32,
    pub bloom_intensity: f32,
    pub bloom_threshold: f32,
    pub bloom_smoothing: f32,
    pub pixel_textures: bool,
}

impl Default for SceneTheme {
    fn default() -> Self {
        Self {
            grays: [16.0, 38.0, 61.0, 90.0, 124.0, 163.0].map(|gray| [gray / 255.0; 3]),
            accent: [1.0, 90.0 / 255.0, 0.0],
            accent_emissive: 1.6,
            background: [3.0 / 255.0; 3],
            fog_color: [6.0 / 255.0; 3],
            fog_density: 0.02,
            ambient: 0.35,
            hemi_sky: [191.0 / 255.0; 3],
            hemi_ground: [13.0 / 255.0; 3],
            hemi_intensity: 0.7,
            key_position: Vec3::new(14.0, 34.0, 18.0),
            key_color: [242.0 / 255.0; 3],
            key_intensity: 3.2,
            portal_position: Vec3::new(0.0, 3.0, -37.0),
            portal_intensity: 50.0,
            portal_range: 40.0,
            portal_decay: 1.6,
            bloom_intensity: 0.85,
            bloom_threshold: 0.9,
            bloom_smoothing: 0.2,
            pixel_textures: true,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Target {
    format: wgpu::TextureFormat,
    samples: u32,
    depth: Option<wgpu::TextureFormat>,
}

/// Supply the actual egui render-pass format, MSAA sample count and depth format.
/// No second device, surface, or event loop is created.
pub fn configure(state: &egui_wgpu::RenderState, msaa_samples: u32, depth_format: Option<wgpu::TextureFormat>) {
    state.renderer.write().callback_resources.insert(Target { format: state.target_format, samples: msaa_samples.max(1), depth: depth_format });
}

#[derive(Clone, Copy, Default)]
struct Enemy {
    position: Vec3,
    yaw: f32,
    flashing: bool,
}

struct Snapshot {
    view: Mat4,
    eye: Vec3,
    shot_age: f32,
    reload_remaining: f32,
    bob: f32,
    dead: bool,
    // Match the simulation's hard caps; snapshots allocate no heap buffers.
    enemies: [Enemy; 8],
    enemy_count: usize,
    projectiles: [Vec3; 32],
    projectile_count: usize,
    debris: [(Vec3, Vec3); 72],
    debris_count: usize,
    /// Where the key light looks (the middle of the shadow map): the origin, or the hero.
    focus: Vec3,
    /// Set for the third-person scene (the first-person one has no hero).
    tps: Option<tps::Draw>,
}

impl Snapshot {
    /// Nothing drawn yet but the camera.
    fn blank(view: Mat4, eye: Vec3) -> Self {
        Self {
            view,
            eye,
            shot_age: 100.0,
            reload_remaining: 0.0,
            bob: 0.0,
            dead: true,
            enemies: [Enemy::default(); 8],
            enemy_count: 0,
            projectiles: [Vec3::ZERO; 32],
            projectile_count: 0,
            debris: [(Vec3::ZERO, Vec3::ZERO); 72],
            debris_count: 0,
            focus: Vec3::ZERO,
            tps: None,
        }
    }
}

struct SceneCallback {
    snapshot: Snapshot,
    theme: SceneTheme,
    rect: egui::Rect,
}

/// Add the scene below any egui-designed HUD. Only render state crosses into
/// the callback; the simulation, input, event queue and strings are not cloned.
pub fn paint(ui: &egui::Ui, rect: egui::Rect, game: &Game, theme: &SceneTheme) {
    if !rect.is_finite() || rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }
    let player = &game.player;
    let eye = player.eye() + Vec3::Y * (player.bob * 2.0).sin() * 0.03;
    let mut snapshot = Snapshot {
        view: Mat4::look_to_rh(eye, player.direction(), Vec3::Y),
        eye,
        shot_age: player.shot_age,
        reload_remaining: player.reload_remaining,
        bob: player.bob,
        dead: player.dead,
        enemies: [Enemy::default(); 8],
        enemy_count: 0,
        projectiles: [Vec3::ZERO; 32],
        projectile_count: 0,
        debris: [(Vec3::ZERO, Vec3::ZERO); 72],
        debris_count: 0,
        focus: Vec3::ZERO,
        tps: None,
    };
    for (slot, enemy) in snapshot.enemies.iter_mut().zip(game.enemies.iter().filter(|enemy| enemy.health > 0 && enemy.respawn_remaining <= 0.0)) {
        *slot = Enemy { position: enemy.position, yaw: enemy.yaw, flashing: enemy.hit_remaining > 0.0 };
        snapshot.enemy_count += 1;
    }
    for (slot, projectile) in snapshot.projectiles.iter_mut().zip(&game.projectiles) {
        *slot = projectile.position;
        snapshot.projectile_count += 1;
    }
    for (slot, debris) in snapshot.debris.iter_mut().zip(&game.debris) {
        *slot = (debris.position, debris.rotation);
        snapshot.debris_count += 1;
    }
    ui.painter().add(Callback::new_paint_callback(rect, SceneCallback { snapshot, theme: *theme, rect }));
}

impl CallbackTrait for SceneCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        screen: &ScreenDescriptor,
        encoder: &mut wgpu::CommandEncoder,
        resources: &mut CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let Some(target) = resources.get::<Target>().copied() else {
            log::error!("Greybox renderer needs renderer::configure with the host egui render-pass configuration");
            return Vec::new();
        };
        let needs_resources = resources.get::<gpu::Resources>().is_none_or(|gpu| gpu.target != target);
        if needs_resources {
            resources.insert(gpu::Resources::new(device, queue, target, &self.theme));
        }
        if let Some(gpu) = resources.get_mut::<gpu::Resources>() {
            let maximum = device.limits().max_texture_dimension_2d;
            let width = (self.rect.width() * screen.pixels_per_point).round().clamp(1.0, maximum as f32) as u32;
            let height = (self.rect.height() * screen.pixels_per_point).round().clamp(1.0, maximum as f32) as u32;
            gpu.prepare(device, queue, encoder, [width, height], &self.snapshot, &self.theme);
        }
        Vec::new()
    }

    fn paint(&self, _info: egui::PaintCallbackInfo, pass: &mut wgpu::RenderPass<'static>, resources: &CallbackResources) {
        if let Some(gpu) = resources.get::<gpu::Resources>() {
            gpu.paint(pass);
        }
    }
}

#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
struct Uniform {
    view_projection: [[f32; 4]; 4],
    light_projection: [[f32; 4]; 4],
    camera: [f32; 4],
    fog: [f32; 4],
    accent: [f32; 4],
    tones: [[f32; 4]; 6],
    key_position: [f32; 4],
    key_color: [f32; 4],
    hemi_sky: [f32; 4],
    hemi_ground: [f32; 4],
    portal: [f32; 4],
    settings: [f32; 4],
    bloom: [f32; 4],
}

fn linear(color: [f32; 3]) -> [f32; 3] {
    color.map(|channel| if channel <= 0.04045 { channel / 12.92 } else { ((channel + 0.055) / 1.055).powf(2.4) })
}

fn rgba(color: [f32; 3], alpha: f32) -> [f32; 4] {
    let [r, g, b] = linear(color);
    [r, g, b, alpha]
}

impl Uniform {
    fn new(snapshot: &Snapshot, theme: &SceneTheme, size: [u32; 2]) -> Self {
        // Graybox: the third-person valley is seen much farther (mountains in the fog) and has its own shadow-map size.
        let (near, far, shadow_texel) = if snapshot.tps.is_some() { (0.1, 420.0, 1.0 / tps::SHADOW_SIZE as f32) } else { (0.05, 200.0, 0.0) };
        let projection = Mat4::perspective_rh(75.0_f32.to_radians(), size[0] as f32 / size[1] as f32, near, far);
        let light = Mat4::orthographic_rh(-35.0, 35.0, -40.0, 40.0, 0.1, 90.0)
            * Mat4::look_at_rh(theme.key_position + snapshot.focus, snapshot.focus, Vec3::Y);
        Self {
            view_projection: (projection * snapshot.view).to_cols_array_2d(),
            light_projection: light.to_cols_array_2d(),
            camera: snapshot.eye.extend(1.0).to_array(),
            fog: rgba(theme.fog_color, theme.fog_density),
            accent: rgba(theme.accent, theme.accent_emissive),
            tones: theme.grays.map(|tone| rgba(tone, 1.0)),
            key_position: theme.key_position.extend(1.0).to_array(),
            key_color: rgba(theme.key_color, theme.key_intensity),
            hemi_sky: rgba(theme.hemi_sky, theme.hemi_intensity),
            hemi_ground: rgba(theme.hemi_ground, 1.0),
            portal: theme.portal_position.extend(theme.portal_intensity).to_array(),
            settings: [if theme.pixel_textures { 1.0 } else { 0.0 }, theme.ambient, theme.portal_range, theme.portal_decay],
            bloom: [theme.bloom_intensity, theme.bloom_threshold, theme.bloom_smoothing, shadow_texel],
        }
    }
}

fn dynamic(vertices: &mut Vec<Vertex>, snapshot: &Snapshot, pistol: &[Vertex]) -> u32 {
    use figcraft_game::arena::Primitive;
    if let Some(draw) = &snapshot.tps {
        return tps::dynamic(vertices, draw);
    }
    vertices.clear();
    for enemy in snapshot.enemies.iter().take(snapshot.enemy_count) {
        let parent = mesh::transform(enemy.position, Vec3::new(0.0, -enemy.yaw, 0.0));
        for (position, size, tone) in [
            ([-0.25, 0.5, 0.0], [0.38, 1.0, 0.45], 2),
            ([0.25, 0.5, 0.0], [0.38, 1.0, 0.45], 2),
            ([0.0, 1.55, 0.0], [1.15, 1.1, 0.65], 3),
            ([0.0, 2.35, 0.0], [0.7, 0.55, 0.6], 2),
        ] {
            let mut piece = mesh::part(position, size, tone, [0.0; 3]);
            if enemy.flashing && position[1] > 1.0 {
                piece.emissive = 2.5;
            }
            mesh::object(vertices, &piece, parent, false);
        }
        // Sentinel forward is -Z, matching the simulation's facing direction.
        let mut slit = mesh::part([0.0, 2.38, -0.305], [0.46, 0.07, 0.02], 0, [0.0; 3]);
        slit.emissive = 1.6;
        mesh::object(vertices, &slit, parent, false);
    }
    for position in snapshot.projectiles.iter().take(snapshot.projectile_count) {
        let mut projectile = mesh::part(position.to_array(), [0.32; 3], 0, [0.0; 3]);
        projectile.primitive = Primitive::Octahedron;
        projectile.emissive = 2.0;
        mesh::object(vertices, &projectile, Mat4::IDENTITY, false);
    }
    for (position, rotation) in snapshot.debris.iter().take(snapshot.debris_count) {
        mesh::object(vertices, &mesh::part(position.to_array(), [0.35; 3], 3, rotation.to_array()), Mat4::IDENTITY, false);
    }
    let world_count = vertices.len() as u32;
    if !snapshot.dead {
        let kick = (1.0 - snapshot.shot_age / 0.12).clamp(0.0, 1.0);
        let dip = if snapshot.reload_remaining > 0.0 {
            (std::f32::consts::PI * (0.8 - snapshot.reload_remaining).clamp(0.0, 0.8) / 0.8).sin()
        } else {
            0.0
        };
        let position = Vec3::new(0.2 + snapshot.bob.sin() * 0.012, -0.19 - snapshot.bob.cos().abs() * 0.012 - 0.18 * dip, -0.44 + 0.05 * kick);
        let rotation = Vec3::new(0.05 + 0.2 * kick - 0.9 * dip, 0.04, -0.06 + 0.4 * dip);
        let pose = snapshot.view.inverse() * mesh::transform(position, rotation);
        vertices.extend(pistol.iter().map(|vertex| Vertex {
            position: pose.transform_point3(Vec3::from_array(vertex.position)).to_array(),
            normal: pose.transform_vector3(Vec3::from_array(vertex.normal)).to_array(),
            material: vertex.material,
        }));
        if snapshot.shot_age < 0.05 {
            let mut flash = mesh::part([0.0, 0.03, -0.23], [0.07; 3], 0, [0.0; 3]);
            flash.primitive = Primitive::Octahedron;
            flash.emissive = 2.5;
            mesh::object(vertices, &flash, pose, true);
        }
    }
    world_count
}
