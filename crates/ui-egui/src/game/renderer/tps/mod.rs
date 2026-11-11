//! The third-person scene, drawn by the same renderer, theme, fog, key light and shadow as the
//! first-person one. The only thing that crosses from the simulation is [`figcraft_game::tps::Frame`].
//!
//! Who owns what: G11.2 (world) owns `world.rs`, `props.rs`, `grass.rs`, `sky.rs`, `scene.rs`,
//! `shapes.rs`, `gpu.rs` and `tps.wgsl`; G11.3 (hero and animation) owns `hero.rs`.
mod gpu;
mod grass;
mod hero;
mod look;
mod props;
mod scene;
mod shapes;
mod sky;
mod world;

#[cfg(test)]
mod tests;

use super::{Callback, SceneCallback, SceneTheme, Snapshot, Vertex};
use figcraft_game::tps::{HeroPose, Tps};
use glam::{Mat4, Vec3};

pub(super) use gpu::{COLOR, Targets, TpsGpu};
use look::TpsLook;
pub(super) use scene::{SHADOW_SIZE, SceneData};

/// What the third-person scene draws besides the world: the hero, and what moves the grass and the sky.
#[derive(Clone, Copy)]
pub(super) struct Draw {
    pub pose: HeroPose,
    /// Where the camera is (the sky is drawn around it).
    pub eye: Vec3,
    /// Seconds since the app started (the wind and the water run on it).
    pub time: f32,
    /// How the picture looks (see [`TpsLook`]).
    pub look: TpsLook,
}

/// Add the third-person scene below any egui-designed HUD.
pub fn paint(ui: &egui::Ui, rect: egui::Rect, tps: &Tps, theme: &SceneTheme) {
    if !rect.is_finite() || rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }
    let frame = tps.frame();
    // The light follows the hero (in whole metres, so the shadow does not shimmer).
    let focus = (frame.hero.position / 2.0).round() * 2.0;
    let time = ui.input(|input| input.time) as f32;
    let look = TpsLook::default();
    let draw = Draw { pose: frame.hero, eye: frame.eye, time, look };
    let snapshot = Snapshot { focus, tps: Some(draw), ..Snapshot::blank(Mat4::look_at_rh(frame.eye, frame.target, Vec3::Y), frame.eye) };
    // The look decides the lights, the fog, the sky, the shadows and the edges; the orange point light
    // is the watchtower's beacon (no first-person light shines from the far end of the arena).
    let theme = look.apply(theme, tps.world().beacon());
    ui.painter().add(Callback::new_paint_callback(rect, SceneCallback { snapshot, theme, rect }));
}

/// The hero and the sky, as vertices to draw this frame. Returns how many are world geometry (all of them).
pub(super) fn dynamic(vertices: &mut Vec<Vertex>, draw: &Draw) -> u32 {
    vertices.clear();
    hero::vertices(vertices, &draw.pose);
    sky::vertices(vertices, draw.eye);
    vertices.len() as u32
}
