//! Pixel mode's pointer and keys: drawing strokes and shapes, the bucket and the eyedropper,
//! dragging the mirror axes, dragging out a new sprite, and the tool keys.

use base64::Engine as _;
use egui::{Event, Key, Pos2};
use figcraft_pixel::canvas;
use figcraft_pixel::raster::Pt;
use figcraft_pixel::select::Op;
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
    // A press that closes the colour picker only closes it.
    let swallow = std::mem::take(&mut app.pixel.closed_picker) || app.pixel.picking.is_some();
    // Alt holds the eyedropper with any drawing tool (with a selection tool it subtracts).
    let tool = if mods.alt && app.pixel.gesture.is_none() && !app.pixel.tool.selects() { Tool::Eyedropper } else { app.pixel.tool };
    if (pressed || pressed2) && resp.hovered() && app.pixel.gesture.is_none() && !swallow {
        let secondary = pressed2 && !pressed;
        if let Some(axis) = axis_under(app, id, &d.sprite, pos) {
            app.pixel.gesture = Some(Gesture::Axis(axis));
        } else {
            press(app, id, &d, tool, at, secondary, mods);
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
        Some(Gesture::Select { tool, mut points, op }) => {
            if tool == Tool::Marquee {
                points.truncate(1);
                points.push(at);
            } else if points.last() != Some(&at) && points.len() < 65_536 {
                points.push(at);
            }
            if down {
                app.pixel.gesture = Some(Gesture::Select { tool, points, op });
            } else {
                app.pixel.gesture = None;
                commit_select(app, id, tool, &points, op);
            }
        }
        Some(Gesture::Move { from, .. }) => {
            if down {
                app.pixel.gesture = Some(Gesture::Move { from, to: at });
            } else {
                app.pixel.gesture = None;
                let (dx, dy) = (at.0 - from.0, at.1 - from.1);
                if (dx, dy) != (0, 0) {
                    on_cel(app, "pixel.transform", json!({"op": "move", "dx": dx, "dy": dy}));
                }
            }
        }
        Some(Gesture::NewSprite { .. }) | None => {}
    }
    resp.hovered() || app.pixel.gesture.is_some()
}

/// How a new selection combines with the one there is: Shift adds, Alt takes away, both keep only
/// the overlap.
fn op_of(mods: egui::Modifiers) -> Op {
    match (mods.shift, mods.alt) {
        (true, true) => Op::Intersect,
        (true, false) => Op::Add,
        (false, true) => Op::Subtract,
        (false, false) => Op::Replace,
    }
}

fn op_name(op: Op) -> &'static str {
    match op {
        Op::Replace => "replace",
        Op::Add => "add",
        Op::Subtract => "subtract",
        Op::Intersect => "intersect",
    }
}

/// A press with `tool` at art pixel `at`.
fn press(app: &mut FigApp, id: figcraft_doc::NodeId, d: &super::Decoded, tool: Tool, at: Pt, secondary: bool, mods: egui::Modifiers) {
    // Playing stops on the frame shown, so the press draws there (G16.4).
    app.pixel.playing = None;
    match tool {
        Tool::Pencil | Tool::Eraser | Tool::Replace => {
            // Shift-click draws a straight line on from the last point drawn (as pixel editors do).
            let points = match app.pixel.last.filter(|_| mods.shift) {
                Some(last) => vec![last, at],
                None => vec![at],
            };
            app.pixel.gesture = Some(Gesture::Stroke { points, secondary, erase: tool == Tool::Eraser });
        }
        Tool::Line | Tool::Rect | Tool::Ellipse => app.pixel.gesture = Some(Gesture::Shape { from: at, to: at, secondary }),
        Tool::Marquee | Tool::Lasso => app.pixel.gesture = Some(Gesture::Select { tool, points: vec![at], op: op_of(mods) }),
        Tool::Wand => {
            let mut p = cel_params(id, d);
            if let Some(o) = p.as_object_mut() {
                o.insert("kind".into(), json!("wand"));
                o.insert("x".into(), json!(at.0));
                o.insert("y".into(), json!(at.1));
                o.insert("op".into(), json!(op_name(op_of(mods))));
                o.insert("contiguous".into(), json!(app.pixel.contiguous));
            }
            app.run("pixel.select", p);
        }
        Tool::Move => app.pixel.gesture = Some(Gesture::Move { from: at, to: at }),
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
        if let Some(r) = i.replace {
            o.insert("replace".into(), json!(canvas::to_hex(r)));
        }
    }
}

fn commit_stroke(app: &mut FigApp, id: figcraft_doc::NodeId, d: &super::Decoded, points: &[Pt], secondary: bool, erase: bool) {
    let mut p = cel_params(id, d);
    ink_params(app, &mut p, secondary, erase);
    if let Some(o) = p.as_object_mut() {
        o.insert("points".into(), json!(points.iter().map(|(x, y)| [*x, *y]).collect::<Vec<_>>()));
        o.insert("pixelPerfect".into(), json!(app.pixel.pixel_perfect && app.pixel.brush.size == 1));
        let step = super::shade_step(&app.pixel, secondary, erase);
        if step != 0 {
            o.insert("shade".into(), json!(step));
        }
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
    // The desktop shell sends Cmd/Ctrl+C, X and V as copy, cut and paste events, not keys; a
    // browser sends both (`command_key` acts once a frame).
    let editing = matches!(app.pixel.mode, Mode::Edit(_));
    match ev {
        Event::Copy if editing => return command_key(app, Key::C, false),
        Event::Cut if editing => return command_key(app, Key::X, false),
        Event::Paste(_) if editing => return command_key(app, Key::V, false),
        _ => {}
    }
    let Event::Key { key, pressed: true, repeat, modifiers, .. } = ev else { return false };
    // Esc with a colour picker open closes the picker only.
    if *key == Key::Escape && app.pixel.picking.is_some() {
        app.pixel.picking = None;
        return true;
    }
    if modifiers.command || modifiers.ctrl {
        return editing && !*repeat && command_key(app, *key, modifiers.shift);
    }
    let selected = editing && selection(app).is_some();
    // Arrows move the selected pixels (Shift: ten at a time); without a selection they nudge the layer.
    let step = if modifiers.shift { 10 } else { 1 };
    let arrow = match key {
        Key::ArrowLeft => Some((-step, 0)),
        Key::ArrowRight => Some((step, 0)),
        Key::ArrowUp => Some((0, -step)),
        Key::ArrowDown => Some((0, step)),
        _ => None,
    };
    if let Some((dx, dy)) = arrow {
        if !selected {
            return false;
        }
        on_cel(app, "pixel.transform", json!({"op": "move", "dx": dx, "dy": dy}));
        return true;
    }
    // Held down, only the brush size keys repeat (a held X would swap the colours back and forth).
    if *repeat && !matches!(key, Key::OpenBracket | Key::CloseBracket | Key::Comma | Key::Period) {
        return matches!(key, Key::B | Key::E | Key::G | Key::I | Key::L | Key::U | Key::X | Key::R | Key::M | Key::Q | Key::W | Key::V);
    }
    let tool = |t: Tool| Some(t);
    let pick = match key {
        Key::B => tool(Tool::Pencil),
        Key::E => tool(Tool::Eraser),
        Key::G => tool(Tool::Bucket),
        Key::I => tool(Tool::Eyedropper),
        Key::R => tool(Tool::Replace),
        Key::L => tool(Tool::Line),
        Key::U if modifiers.shift => tool(Tool::Ellipse),
        Key::U => tool(Tool::Rect),
        Key::M => tool(Tool::Marquee),
        Key::Q => tool(Tool::Lasso),
        Key::W => tool(Tool::Wand),
        Key::V => tool(Tool::Move),
        _ => None,
    };
    if let Some(t) = pick {
        app.pixel.tool = t;
        return true;
    }
    match key {
        Key::X => app.pixel.colors.swap(0, 1),
        // The timeline (G16.4): previous and next frame (Enter stays Done, as in vector edit).
        Key::Comma if editing => super::timeline::step(app, false),
        Key::Period if editing => super::timeline::step(app, true),
        Key::OpenBracket => step_brush(&mut app.pixel, false),
        Key::CloseBracket => step_brush(&mut app.pixel, true),
        Key::Delete | Key::Backspace if editing => on_cel(app, "pixel.clear", json!({})),
        Key::Escape if app.pixel.gesture.is_some() => app.pixel.gesture = None,
        // Esc lets go of the selection first, then leaves Pixel mode.
        Key::Escape if selected => on_cel(app, "pixel.select", json!({"kind": "none"})),
        Key::Escape | Key::Enter => leave(app),
        _ => return false,
    }
    true
}

/// The clipboard holds only a picture (as it does after copying pixels on the desktop, where Cmd/
/// Ctrl+V then comes as an image paste and not as a key): while a sprite is edited, the copied
/// pixels go into it, not onto the page as a new layer. What is left for the app's own paste.
pub(crate) fn paste_events(app: &mut FigApp, paste: crate::shortcuts::PasteEvents) -> crate::shortcuts::PasteEvents {
    if !paste.image || !matches!(app.pixel.mode, Mode::Edit(_)) || app.pixel.clipboard.is_none() {
        return paste;
    }
    if !app.pixel.pasted {
        command_key(app, Key::V, false);
    }
    crate::shortcuts::PasteEvents { image: false, ..paste }
}

/// The selection of the sprite being edited.
fn selection(app: &mut FigApp) -> Option<figcraft_pixel::Mask> {
    let Mode::Edit(id) = app.pixel.mode else { return None };
    decode(app, id).and_then(|d| d.selection.clone())
}

/// Cmd/Ctrl with `key` while a sprite is edited: select all, deselect, invert, copy, cut and
/// paste work on its pixels. True when it was one of those.
fn command_key(app: &mut FigApp, key: Key, shift: bool) -> bool {
    match key {
        Key::A if !shift => on_cel(app, "pixel.select", json!({"kind": "all"})),
        Key::D if !shift => on_cel(app, "pixel.select", json!({"kind": "none"})),
        Key::I if shift => on_cel(app, "pixel.select", json!({"kind": "invert"})),
        // Once a frame each (a browser sends the key and the clipboard event).
        Key::X if app.pixel.cut => {}
        Key::V if app.pixel.pasted => {}
        Key::C | Key::X if !shift => {
            copy(app);
            if key == Key::X {
                app.pixel.cut = true;
                on_cel(app, "pixel.clear", json!({}));
            }
        }
        Key::V if !shift => {
            let Some((picture, x, y)) = app.pixel.clipboard.clone() else { return false };
            let Some(png) = picture.to_png() else { return true };
            let png = base64::engine::general_purpose::STANDARD.encode(png);
            on_cel(app, "pixel.paste", json!({"png": png, "x": x, "y": y}));
            app.pixel.pasted = true;
            // The pasted pixels are selected: the Move tool takes them where they should go.
            app.pixel.tool = Tool::Move;
        }
        _ => return false,
    }
    true
}

/// Copies the selected pixels of the current layer (all of it when nothing is selected), for
/// Paste here and as a picture on the system clipboard.
fn copy(app: &mut FigApp) {
    let Mode::Edit(id) = app.pixel.mode else { return };
    let Some(d) = decode(app, id) else { return };
    let Some(piece) = figcraft_pixel::transform::copy(&d.cel, d.selection.as_ref()) else { return };
    let (x, y) = d.selection.as_ref().and_then(figcraft_pixel::Mask::bounds).map_or((0, 0), |b| (b.0, b.1));
    app.pixel.to_system = Some(piece.clone());
    app.pixel.clipboard = Some((piece, x, y));
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

/// A selection made with the marquee or the lasso. A click without a drag deselects (unless it
/// adds to or takes from the selection).
fn commit_select(app: &mut FigApp, id: figcraft_doc::NodeId, tool: Tool, points: &[Pt], op: Op) {
    let pts: Vec<[i64; 2]> = points.iter().map(|(x, y)| [*x, *y]).collect();
    let click = match tool {
        Tool::Marquee => points.first() == points.last(),
        _ => points.len() < 3,
    };
    let p = if click {
        if op != Op::Replace {
            return;
        }
        json!({"id": id.0, "kind": "none"})
    } else if tool == Tool::Marquee {
        json!({"id": id.0, "kind": "rect", "from": pts.first(), "to": pts.last(), "op": op_name(op)})
    } else {
        json!({"id": id.0, "kind": "lasso", "points": pts, "op": op_name(op)})
    };
    app.run("pixel.select", p);
}

/// Runs `command` with `params` on the sprite being edited (its current layer and frame).
pub(crate) fn on_cel(app: &mut FigApp, command: &str, params: Value) {
    let Mode::Edit(id) = app.pixel.mode else { return };
    let Some(d) = decode(app, id) else { return };
    let mut p = cel_params(id, &d);
    if let (Some(o), Some(extra)) = (p.as_object_mut(), params.as_object()) {
        o.extend(extra.clone());
    }
    app.run(command, p);
}
