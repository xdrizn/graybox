//! The third-person scene, drawn by the same renderer, theme, fog, key light and shadow as the
//! first-person one. The only thing that crosses from the simulation is [`figcraft_game::tps::Frame`].
//!
//! Who owns what: G11.2 (world) owns `world.rs`; G11.3 (hero and animation) owns `hero.rs`.
mod hero;
mod world;

use super::{Callback, SceneCallback, SceneTheme, Snapshot, Vertex};
use figcraft_game::tps::{HeroPose, Tps};
use glam::{Mat4, Vec3};

pub(super) use world::objects as world_objects;

/// What the third-person scene draws besides the world: the hero.
#[derive(Clone, Copy)]
pub(super) struct Draw {
    pub pose: HeroPose,
}

/// Add the third-person scene below any egui-designed HUD.
pub fn paint(ui: &egui::Ui, rect: egui::Rect, tps: &Tps, theme: &SceneTheme) {
    if !rect.is_finite() || rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }
    let frame = tps.frame();
    // The light follows the hero (in whole metres, so the shadow does not shimmer).
    let focus = (frame.hero.position / 2.0).round() * 2.0;
    let snapshot =
        Snapshot { focus, tps: Some(Draw { pose: frame.hero }), ..Snapshot::blank(Mat4::look_at_rh(frame.eye, frame.target, Vec3::Y), frame.eye) };
    // No first-person light shines from the far end of the arena.
    let theme = SceneTheme { portal_intensity: 0.0, ..*theme };
    ui.painter().add(Callback::new_paint_callback(rect, SceneCallback { snapshot, theme, rect }));
}

/// The hero, as vertices to draw this frame. Returns how many are world geometry (all of them).
pub(super) fn dynamic(vertices: &mut Vec<Vertex>, draw: &Draw) -> u32 {
    vertices.clear();
    hero::vertices(vertices, &draw.pose);
    vertices.len() as u32
}
