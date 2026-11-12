//! Pixel mode's pointer and keys: drawing strokes and shapes, the bucket and the eyedropper,
//! dragging the mirror axes, dragging out a new sprite, and the tool keys.

use egui::{Event, Key, Pos2};
use figcraft_pixel::canvas;
use figcraft_pixel::raster::Pt;
use serde_json::{Value, json};

use super::{Gesture, Mode, Tool, art_at, art_to_page, art_to_screen, decode, enter, ink, is_pixel_layer, leave, settle, step_brush};
use crate::FigApp;

/// Screen points within which a press takes a mirror axis's handle.
const AXIS_GRAB: f32 = 7.0;

/// Pixel mode's share of the canvas pointer at `pos`. True when it took the pointer (the canvas's
/// own tools then leave it alone). Off, it only listens for a double-click on a pixel layer.
pub(crate) fn input(app: &mut FigApp, ui: &egui::Ui, resp: &egui::Response, pos: Pos2) -> bool {
    settle(app);
    app.pixel.pointer = Some(pos);
    match app.pixel.mode {
        Mode::Off => {
            if resp.double_clicked()
                && let [id] = app.session.selection().as_slice()
                && is_pixel_layer(app, *id)
                && under(app, *id, pos)
            {
                enter(app, *id);
                return true;
            }
            false
        }
        Mode::New => new_sprite(app, ui, resp, pos),
        Mode::Edit(id) => edit(app, ui, resp, pos, id),
    }
}

/// Whether screen point `pos` is over layer `id`.
fn under(app: &FigApp, id: figcraft_doc::NodeId, pos: Pos2) -> bool {
    let Ok(doc) = app.session.doc() else { return false };
    let Some(n) = doc.node(id) else { return false };
    let xf = doc.world_transform(id);
    if xf.determinant().abs() < 1e-12 {
        return false;
    }
    let p = xf.inverse() * app.to_page(pos);
    p.x >= 0.0 && p.y >= 0.0 && p.x <= n.width && p.y <= n.height
}

/// No sprite yet: a drag makes one of the dragged size (whole art pixels at the New sprite scale).
fn new_sprite(app: &mut FigApp, ui: &egui::Ui, resp: &egui::Response, pos: Pos2) -> bool {
    let (pressed, down) = ui.input(|i| (i.pointer.primary_pressed(), i.pointer.primary_down()));
    let page = app.to_page(pos);
    if pressed && resp.hovered() {
        app.pixel.gesture = Some(Gesture::NewSprite { from: page, to: page });
    }
    if let Some(Gesture::NewSprite { from, .. }) = app.pixel.gesture {
        if down {
            app.pixel.gesture = Some(Gesture::NewSprite { from, to: page });
        } else {
            app.pixel.gesture = None;
            let scale = f64::from(app.pixel.new_size[2].max(1));
            let (x0, y0) = (from.x.min(page.x).round(), from.y.min(page.y).round());
            let w = ((from.x - page.x).abs() / scale).round();
            let h = ((from.y - page.y).abs() / scale).round();
            // A click (no real drag) makes one of the New sprite size there.
            let (w, h) = if w < 1.0 || h < 1.0 { (f64::from(app.pixel.new_size[0]), f64::from(app.pixel.new_size[1])) } else { (w, h) };
            let made = app.run("pixel.create", json!({"x": x0, "y": y0, "width": w.min(1024.0), "height": h.min(1024.0), "scale": scale}));
            if let Some(id) = made.as_ref().and_then(|r| figcraft_engine::params::id(r, "id")) {
                enter(app, id);
            }
        }
    }
    resp.hovered() || app.pixel.gesture.is_some()
}

/// Editing layer `id`.
fn edit(app: &mut FigApp, ui: &egui::Ui, resp: &egui::Response, pos: Pos2, id: figcraft_doc::NodeId) -> bool {
    let Some(d) = decode(app, id) else {
        leave(app);
        return false;
    };
    let (pressed2, pressed, down, mods) =
        ui.input(|i| (i.pointer.secondary_pressed(), i.pointer.primary_pressed(), i.pointer.any_down(), i.modifiers));
    let at = art_at(app, id, &d.sprite, pos);
    app.pixel.hover = at.filter(|_| resp.hovered());
    let Some(at) = at else { return resp.hovered() };
    // Alt holds the eyedropper with any tool.
    // A press that closes the colour picker only closes it.
    let swallow = std::mem::take(&mut app.pixel.closed_picker) || app.pixel.picking.is_some();
    let tool = if mods.alt && app.pixel.gesture.is_none() { Tool::Eyedropper } else { app.pixel.tool };
    if (pressed || pressed2) && resp.hovered() && app.pixel.gesture.is_none() && !swallow {
        let secondary = pressed2 && !pressed;
        if let Some(axis) = axis_under(app, id, &d.sprite, pos) {
            app.pixel.gesture = Some(Gesture::Axis(axis));
        } else {
            press(app, id, &d, tool, at, secondary, mods.shift);
        }
    }
    match app.pixel.gesture.clone() {
        Some(Gesture::Stroke { mut points, secondary, erase }) => {
            if points.last() != Some(&at) {
                points.push(at);
            }
            if down {
                app.pixel.gesture = Some(Gesture::Stroke { points, secondary, erase });
            } else {
                app.pixel.gesture = None;
                commit_stroke(app, id, &d, &points, secondary, erase);
            }
        }
        Some(Gesture::Shape { from, secondary, .. }) => {
            if down {
                app.pixel.gesture = Some(Gesture::Shape { from, to: at, secondary });
            } else {
                app.pixel.gesture = None;
                commit_shape(app, id, &d, from, at, secondary, mods.shift);
            }
        }
        Some(Gesture::Axis(which)) => {
            if down {
                let xf = art_to_page(app, id, &d.sprite);
                if let Some(xf) = xf.filter(|x| x.determinant().abs() > 1e-12) {
                    let p = xf.inverse() * app.to_page(pos);
                    // Snapped to half pixels, kept inside the sprite.
                    let (v, max) = if which == 0 { (p.x, d.sprite.width) } else { (p.y, d.sprite.height) };
                    if v.is_finite() {
                        app.pixel.axis[which] = (v * 2.0).round().clamp(0.0, f64::from(max) * 2.0) as i64;
                    }
                }
            } else {
                app.pixel.gesture = None;
            }
        }
        Some(Gesture::NewSprite { .. }) | None => {}
    }
    resp.hovered() || app.pixel.gesture.is_some()
}

/// A press with `tool` at art pixel `at`.
fn press(app: &mut FigApp, id: figcraft_doc::NodeId, d: &super::Decoded, tool: Tool, at: Pt, secondary: bool, shift: bool) {
    match tool {
        Tool::Pencil | Tool::Eraser => {
            // Shift-click draws a straight line on from the last point drawn (as pixel editors do).
            let points = match app.pixel.last.filter(|_| shift) {
                Some(last) => vec![last, at],
                None => vec![at],
            };
            app.pixel.gesture = Some(Gesture::Stroke { points, secondary, erase: tool == Tool::Eraser });
        }
        Tool::Line | Tool::Rect | Tool::Ellipse => app.pixel.gesture = Some(Gesture::Shape { from: at, to: at, secondary }),
        Tool::Bucket => {
            let mut p = cel_params(id, d);
            if let Some(o) = p.as_object_mut() {
                o.insert("x".into(), json!(at.0));
                o.insert("y".into(), json!(at.1));
                o.insert("color".into(), json!(canvas::to_hex(app.pixel.colors[usize::from(secondary)])));
                o.insert("contiguous".into(), json!(app.pixel.contiguous));
                o.insert("tile".into(), json!(app.pixel.tile));
            }
            app.run("pixel.fill", p);
        }
        Tool::Eyedropper => {
            // What the frame shows there; a transparent pixel picks transparency.
            let picture = d.picture(&d.cel);
            if let Some(c) = picture.get(at.0, at.1) {
                app.pixel.colors[usize::from(secondary)] = c;
            }
        }
    }
}

/// `{id, layer, frame}` for the cel being drawn on.
fn cel_params(id: figcraft_doc::NodeId, d: &super::Decoded) -> Value {
    json!({"id": id.0, "layer": d.layer, "frame": d.frame})
}

/// The settings a stroke or shape command takes from the editor.
fn ink_params(app: &FigApp, p: &mut Value, secondary: bool, erase: bool) {
    let e = &app.pixel;
    let i = ink(e, secondary, erase);
    if let Some(o) = p.as_object_mut() {
        o.insert("color".into(), json!(canvas::to_hex(i.color)));
        o.insert("size".into(), json!(i.brush.size));
        o.insert("shape".into(), json!(if i.brush.shape == figcraft_pixel::BrushShape::Circle { "circle" } else { "square" }));
        o.insert("tile".into(), json!(i.tile));
        o.insert("erase".into(), json!(erase));
        let mut mirror = serde_json::Map::new();
        if let Some(x2) = i.symmetry.x2 {
            mirror.insert("x".into(), json!(x2 as f64 / 2.0));
        }
        if let Some(y2) = i.symmetry.y2 {
            mirror.insert("y".into(), json!(y2 as f64 / 2.0));
        }
        if !mirror.is_empty() {
            o.insert("mirror".into(), Value::Object(mirror));
        }
    }
}

fn commit_stroke(app: &mut FigApp, id: figcraft_doc::NodeId, d: &super::Decoded, points: &[Pt], secondary: bool, erase: bool) {
    let mut p = cel_params(id, d);
    ink_params(app, &mut p, secondary, erase);
    if let Some(o) = p.as_object_mut() {
        o.insert("points".into(), json!(points.iter().map(|(x, y)| [*x, *y]).collect::<Vec<_>>()));
        o.insert("pixelPerfect".into(), json!(app.pixel.pixel_perfect && app.pixel.brush.size == 1));
    }
    app.pixel.last = points.last().copied();
    app.run("pixel.stroke", p);
}

fn commit_shape(app: &mut FigApp, id: figcraft_doc::NodeId, d: &super::Decoded, from: Pt, to: Pt, secondary: bool, shift: bool) {
    let mut p = cel_params(id, d);
    ink_params(app, &mut p, secondary, false);
    let kind = match app.pixel.tool {
        Tool::Line => "line",
        Tool::Ellipse => "ellipse",
        _ => "rect",
    };
    if let Some(o) = p.as_object_mut() {
        o.insert("kind".into(), json!(kind));
        o.insert("from".into(), json!([from.0, from.1]));
        o.insert("to".into(), json!([to.0, to.1]));
        o.insert("filled".into(), json!(app.pixel.filled));
        o.insert("square".into(), json!(shift));
    }
    app.run("pixel.shape", p);
}

/// The mirror axis whose handle (past the sprite's top or left edge, or along the line itself) is
/// under `pos`.
fn axis_under(app: &FigApp, id: figcraft_doc::NodeId, sprite: &figcraft_pixel::Sprite, pos: Pos2) -> Option<usize> {
    let xf = art_to_page(app, id, sprite)?;
    for which in 0..2 {
        if !app.pixel.mirror.get(which).copied().unwrap_or(false) {
            continue;
        }
        let v = app.pixel.axis.get(which).copied().unwrap_or(0) as f64 / 2.0;
        let handle = if which == 0 { art_to_screen(app, xf, v, 0.0) } else { art_to_screen(app, xf, 0.0, v) };
        let outward = if which == 0 { egui::vec2(0.0, -14.0) } else { egui::vec2(-14.0, 0.0) };
        if pos.distance(handle + outward) <= AXIS_GRAB + 3.0 {
            return Some(which);
        }
    }
    None
}

/// A key pressed in Pixel mode: true when it was Pixel mode's (the app's shortcuts leave it alone).
pub(crate) fn key(app: &mut FigApp, ev: &Event) -> bool {
    if app.pixel.mode == Mode::Off {
        return false;
    }
    let Event::Key { key, pressed: true, repeat, modifiers, .. } = ev else { return false };
    // Esc with a colour picker open closes the picker only.
    if *key == Key::Escape && app.pixel.picking.is_some() {
        app.pixel.picking = None;
        return true;
    }
    if modifiers.command || modifiers.ctrl {
        return false;
    }
    // Held down, only the brush size keys repeat (a held X would swap the colours back and forth).
    if *repeat && !matches!(key, Key::OpenBracket | Key::CloseBracket) {
        return matches!(key, Key::B | Key::E | Key::G | Key::I | Key::L | Key::U | Key::X);
    }
    let tool = |t: Tool| Some(t);
    let pick = match key {
        Key::B => tool(Tool::Pencil),
        Key::E => tool(Tool::Eraser),
        Key::G => tool(Tool::Bucket),
        Key::I => tool(Tool::Eyedropper),
        Key::L => tool(Tool::Line),
        Key::U if modifiers.shift => tool(Tool::Ellipse),
        Key::U => tool(Tool::Rect),
        _ => None,
    };
    if let Some(t) = pick {
        app.pixel.tool = t;
        return true;
    }
    match key {
        Key::X => app.pixel.colors.swap(0, 1),
        Key::OpenBracket => step_brush(&mut app.pixel, false),
        Key::CloseBracket => step_brush(&mut app.pixel, true),
        Key::Escape if app.pixel.gesture.is_some() => app.pixel.gesture = None,
        Key::Escape | Key::Enter => leave(app),
        _ => return false,
    }
    true
}

/// The pointer is off the canvas (over a panel, or out of the window): a drag on the sprite goes on
/// where it is, and one released there ends where the pointer was last seen.
pub(crate) fn off_canvas(app: &mut FigApp, ui: &egui::Ui, resp: &egui::Response, hover: Option<Pos2>) {
    if app.pixel.gesture.is_none() {
        return;
    }
    if let Some(pos) = hover.or(app.pixel.pointer) {
        input(app, ui, resp, pos);
    } else {
        app.pixel.gesture = None;
    }
}
