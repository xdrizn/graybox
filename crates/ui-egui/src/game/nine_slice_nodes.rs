//! Graybox: the four points where the 9-slice cuts cross, on the canvas (G9). Dragging one moves
//! the horizontal and the vertical cut through it together; with Shift the opposite cuts follow,
//! so the slice stays symmetric (Shift does the same on a single cut).

use egui::{CornerRadius, Id, Pos2, Rect, Stroke, StrokeKind, vec2};
use figcraft_doc::NodeId;
use figcraft_doc::nine_slice::NineSlice;
use figcraft_geom::Affine;
use serde_json::json;

use crate::FigApp;

/// A node's drawn size and how far from its middle a press still grabs it (screen points).
const SIZE: f32 = 7.0;
const GRAB: f32 = 6.0;

/// Each crossing as the two cuts through it: (horizontal side 0 top or 2 bottom, vertical side 3
/// left or 1 right). Top left, top right, bottom right, bottom left.
pub(crate) const CROSSINGS: [(usize, usize); 4] = [(0, 3), (0, 1), (2, 1), (2, 3)];

fn drag_id() -> Id {
    Id::unique("graybox_nine_slice_node_drag")
}

/// The four crossings on screen, in `CROSSINGS` order.
fn nodes(app: &FigApp, world: Affine, (w, h): (f64, f64), s: &NineSlice) -> [Pos2; 4] {
    let [t, r, b, l] = s.insets;
    let to = |x: f64, y: f64| {
        let p = world * figcraft_geom::Point::new(x, y);
        app.to_screen(p.x, p.y)
    };
    [to(l, t), to(w - r, t), to(w - r, h - b), to(l, h - b)]
}

/// `insets` with cut `side` at `v` (rounded, never negative); with `symmetric` the opposite cut
/// takes the same value.
pub(crate) fn set_cut(insets: &mut [f64; 4], side: usize, v: f64, symmetric: bool) {
    let v = if v.is_finite() && side < 4 { v.round().max(0.0) } else { return };
    if let Some(slot) = insets.get_mut(side) {
        *slot = v;
    }
    if symmetric && let Some(slot) = insets.get_mut((side + 2) % 4) {
        *slot = v;
    }
}

/// Where cut `side` sits for the layer-space point `local` in a layer `w` × `h`.
pub(crate) fn cut_at(side: usize, local: figcraft_geom::Point, (w, h): (f64, f64)) -> f64 {
    match side {
        0 => local.y,
        1 => w - local.x,
        2 => h - local.y,
        _ => local.x,
    }
}

fn cursor(k: usize) -> egui::CursorIcon {
    if k.is_multiple_of(2) { egui::CursorIcon::ResizeNwSe } else { egui::CursorIcon::ResizeNeSw }
}

/// Draw the crossings: small squares, filled while under the pointer or dragged.
pub(crate) fn draw(app: &FigApp, painter: &egui::Painter, world: Affine, size: (f64, f64), s: &NineSlice) {
    let ctx = painter.ctx();
    let dragged: Option<usize> = ctx.data(|d| d.get_temp(drag_id()));
    let pointer = ctx.input(|i| i.pointer.hover_pos());
    let col = crate::theme::ACCENT;
    for (k, at) in nodes(app, world, size, s).into_iter().enumerate() {
        let hot = dragged == Some(k) || (dragged.is_none() && pointer.is_some_and(|p| p.distance(at) <= GRAB));
        let r = Rect::from_center_size(at, vec2(SIZE, SIZE));
        painter.rect(r, CornerRadius::same(1), if hot { col } else { egui::Color32::WHITE }, Stroke::new(1.0, col), StrokeKind::Middle);
    }
}

/// Forget a drag whose layer is gone (or no longer the one 9-sliced selection).
pub(crate) fn cancel(app: &mut FigApp, ctx: &egui::Context) {
    if ctx.data(|d| d.get_temp::<usize>(drag_id())).is_some() {
        ctx.data_mut(|d| d.remove::<usize>(drag_id()));
        let _ = app.session.commit_interaction();
    }
}

/// The pointer at `pos` over layer `id` (9-sliced by `s`): a press on a crossing drags it, the
/// drag moves both its cuts (with Shift their opposites too), the release ends it (one undo step).
/// True when the pointer belongs to a crossing.
pub(crate) fn input(app: &mut FigApp, ctx: &egui::Context, pos: Pos2, id: NodeId, world: Affine, size: (f64, f64), s: &NineSlice) -> bool {
    let (pressed, down, released, shift) = ctx.input(|i| {
        (
            i.pointer.button_pressed(egui::PointerButton::Primary),
            i.pointer.primary_down(),
            i.pointer.button_released(egui::PointerButton::Primary),
            i.modifiers.shift,
        )
    });
    if let Some(k) = ctx.data(|d| d.get_temp::<usize>(drag_id())) {
        ctx.set_cursor_icon(cursor(k));
        if down && let Some(&(hz, vt)) = CROSSINGS.get(k) {
            let local = world.inverse() * app.to_page(pos);
            let mut insets = s.insets;
            set_cut(&mut insets, hz, cut_at(hz, local, size), shift);
            set_cut(&mut insets, vt, cut_at(vt, local, size), shift);
            if insets != s.insets {
                let _ = app.session.execute("game.nineSlice.set", &json!({"ids": [id], "insets": insets}));
            }
        }
        if released || !down {
            ctx.data_mut(|d| d.remove::<usize>(drag_id()));
            let _ = app.session.commit_interaction();
        }
        return true;
    }
    let Some(k) = nodes(app, world, size, s).into_iter().position(|at| at.distance(pos) <= GRAB) else { return false };
    ctx.set_cursor_icon(cursor(k));
    if pressed {
        let _ = app.session.begin_interaction("9-slice");
        ctx.data_mut(|d| d.insert_temp(drag_id(), k));
    }
    true
}

#[cfg(test)]
mod tests {
    use egui::{Event, Modifiers, PointerButton, Pos2};
    use figcraft_doc::NodeId;
    use figcraft_doc::nine_slice::NineSlice;
    use figcraft_engine::Session;
    use serde_json::json;

    use crate::layers_drag_tests::Ui;

    /// A selected 200 × 200 frame at (100, 100) with every cut 20 in.
    fn scene() -> (Ui, NodeId) {
        let mut s = Session::new();
        let f = figcraft_engine::params::id_of(&s.execute("frame.create", &json!({"x": 100, "y": 100, "width": 200, "height": 200})).unwrap()["id"])
            .unwrap();
        s.execute("game.nineSlice.set", &json!({"ids": [f], "enabled": true, "insets": [20, 20, 20, 20]})).unwrap();
        s.execute("select.set", &json!({"ids": [f]})).unwrap();
        (Ui::new(s), f)
    }

    fn insets(u: &Ui, f: NodeId) -> [f64; 4] {
        NineSlice::of(u.app.session.doc().unwrap().node(f).unwrap()).unwrap().insets
    }

    /// Press at page point `from`, move to page point `to` in steps and release, `mods` held.
    fn drag(u: &mut Ui, from: (f64, f64), to: (f64, f64), mods: Modifiers) {
        let (a, z) = (u.app.to_screen(from.0, from.1), u.app.to_screen(to.0, to.1));
        let button = |p: Pos2, pressed: bool| Event::PointerButton { pos: p, button: PointerButton::Primary, pressed, modifiers: mods };
        u.frame_with(vec![Event::PointerMoved(a)], mods);
        u.frame_with(vec![Event::PointerMoved(a), button(a, true)], mods);
        for k in 1..=5 {
            u.frame_with(vec![Event::PointerMoved(a + (z - a) * (k as f32 / 5.0))], mods);
        }
        u.frame_with(vec![button(z, false)], mods);
        u.frame(vec![]);
    }

    #[test]
    fn dragging_a_crossing_moves_both_of_its_cuts_in_one_undo_step() {
        let (mut u, f) = scene();
        // The top left crossing is at page (120, 120).
        drag(&mut u, (120.0, 120.0), (140.0, 150.0), Modifiers::NONE);
        assert_eq!(insets(&u, f), [50.0, 20.0, 20.0, 40.0]);
        assert_eq!(u.app.session.ds().unwrap().selection, vec![f], "the frame stays selected, nothing moved");
        u.app.run("edit.undo", json!({}));
        assert_eq!(insets(&u, f), [20.0; 4]);
    }

    #[test]
    fn with_shift_a_crossing_keeps_the_slice_symmetric() {
        let (mut u, f) = scene();
        // The bottom right crossing is at page (280, 280); 10 left and 30 up.
        drag(&mut u, (280.0, 280.0), (270.0, 250.0), Modifiers::SHIFT);
        assert_eq!(insets(&u, f), [50.0, 30.0, 50.0, 30.0]);
    }

    #[test]
    fn with_shift_a_single_cut_moves_its_opposite_too() {
        let (mut u, f) = scene();
        // The left cut, halfway down.
        drag(&mut u, (120.0, 200.0), (135.0, 200.0), Modifiers::SHIFT);
        assert_eq!(insets(&u, f), [20.0, 35.0, 20.0, 35.0]);
    }

    #[test]
    fn set_cut_ignores_junk() {
        let mut i = [1.0, 2.0, 3.0, 4.0];
        super::set_cut(&mut i, 9, 5.0, true);
        super::set_cut(&mut i, 0, f64::NAN, true);
        assert_eq!(i, [1.0, 2.0, 3.0, 4.0]);
        super::set_cut(&mut i, 1, -7.0, false);
        assert_eq!(i, [1.0, 0.0, 3.0, 4.0]);
    }
}
