//! Pixel mode on the canvas: the sprite over a checkerboard, sharp; the pixel grid when zoomed in;
//! the brush under the pointer (and its mirror images); the mirror axes with their handles; tile
//! mode's copies around the sprite; the box of a sprite being dragged out.

use egui::epaint::{Mesh, Vertex};
use egui::{Color32, ColorImage, Pos2, Rect, Shape, Stroke, TextureOptions, pos2};
use figcraft_geom::Affine;
use figcraft_pixel::paint::{covered, freehand, paint};
use figcraft_pixel::transform::{self, Change};
use figcraft_pixel::{Canvas, Rgba};

use super::{Gesture, Mode, Tool, art_to_page, art_to_screen, decode, ink, screen_quad, settle, zoom_of};
use crate::FigApp;
use crate::theme::Tokens;

/// Screen pixels an art pixel must span before the pixel grid shows.
const GRID_FROM: f32 = 6.0;

/// Pixel mode's drawing over the canvas.
pub(crate) fn draw(app: &mut FigApp, painter: &egui::Painter) {
    settle(app);
    // A copy goes on the system clipboard too (as a picture other apps can paste).
    if let Some(c) = app.pixel.to_system.take() {
        crate::services::egui_copy_png(painter.ctx(), c.width(), c.height(), &c.to_bytes());
    }
    match app.pixel.mode {
        Mode::Off => {}
        Mode::New => draw_new(app, painter),
        Mode::Edit(id) => draw_edit(app, painter, id),
    }
}

/// The box of a sprite being dragged out, in whole art pixels, with its size.
fn draw_new(app: &FigApp, painter: &egui::Painter) {
    let t = Tokens::get(painter.ctx());
    let Some(Gesture::NewSprite { from, to }) = app.pixel.gesture else { return };
    let scale = f64::from(app.pixel.new_size[2].max(1));
    let (w, h) = (((from.x - to.x).abs() / scale).round(), ((from.y - to.y).abs() / scale).round());
    let (x0, y0) = (from.x.min(to.x).round(), from.y.min(to.y).round());
    let r = Rect::from_two_pos(app.to_screen(x0, y0), app.to_screen(x0 + w * scale, y0 + h * scale));
    painter.rect_filled(r, 0.0, t.accent.gamma_multiply(0.08));
    painter.rect_stroke(r, 0.0, Stroke::new(1.0, t.accent), egui::StrokeKind::Inside);
    let label = format!("{}×{}", w as i64, h as i64);
    let at = pos2(r.center().x, r.max.y + 12.0);
    painter.text(at, egui::Align2::CENTER_CENTER, label, crate::theme::semibold(11.0), t.accent);
}

/// A textured quad: `q` (top left, top right, bottom right, bottom left) showing `uv`.
fn quad(tex: egui::TextureId, q: [Pos2; 4], uv: Rect, tint: Color32) -> Shape {
    let mut m = Mesh::with_texture(tex);
    let uvs = [uv.left_top(), uv.right_top(), uv.right_bottom(), uv.left_bottom()];
    for (pos, uv) in q.iter().zip(uvs) {
        m.vertices.push(Vertex { pos: *pos, uv, color: tint });
    }
    m.add_triangle(0, 1, 2);
    m.add_triangle(0, 2, 3);
    Shape::mesh(m)
}

/// The checkerboard texture (two greys, repeating).
fn checker(app: &mut FigApp, ctx: &egui::Context) -> egui::TextureId {
    if let Some(t) = &app.pixel.checker {
        return t.id();
    }
    let (a, b) = ([0xc8, 0xc8, 0xc8, 255], [0xa0, 0xa0, 0xa0, 255]);
    let bytes: Vec<u8> = [a, b, b, a].concat();
    let img = ColorImage::from_rgba_unmultiplied([2, 2], &bytes);
    let opts = TextureOptions { wrap_mode: egui::TextureWrapMode::Repeat, ..TextureOptions::NEAREST };
    let t = ctx.load_texture("graybox-pixel-checker", img, opts);
    let id = t.id();
    app.pixel.checker = Some(t);
    id
}

/// The sprite picture as a texture (uploaded again only when it changed).
fn texture(app: &mut FigApp, ctx: &egui::Context, picture: &Canvas, key: u64) -> egui::TextureId {
    if let Some((t, k)) = &app.pixel.texture
        && *k == key
    {
        return t.id();
    }
    let img = ColorImage::from_rgba_unmultiplied([picture.width() as usize, picture.height() as usize], &picture.to_bytes());
    match &mut app.pixel.texture {
        Some((t, k)) => {
            t.set(img, TextureOptions::NEAREST);
            *k = key;
            t.id()
        }
        None => {
            let t = ctx.load_texture("graybox-pixel-sprite", img, TextureOptions::NEAREST);
            let id = t.id();
            app.pixel.texture = Some((t, key));
            id
        }
    }
}

fn draw_edit(app: &mut FigApp, painter: &egui::Painter, id: figcraft_doc::NodeId) {
    let Some(d) = decode(app, id) else { return };
    let Some(xf) = art_to_page(app, id, &d.sprite) else { return };
    let ctx = painter.ctx().clone();
    let t = Tokens::get(&ctx);
    let (w, h) = (f64::from(d.sprite.width), f64::from(d.sprite.height));
    let shift = ctx.input(|i| i.modifiers.shift);
    // The picture is made again only when the cel or the stroke or shape in progress changed.
    let key = preview_key(app, d.key, shift);
    let tex = match app.pixel.texture.as_ref().filter(|(_, k)| *k == key) {
        Some((t, _)) => t.id(),
        None => {
            let picture = d.picture(&preview_cel(app, &d, shift));
            texture(app, &ctx, &picture, key)
        }
    };
    let check = checker(app, &ctx);
    let zoom = zoom_of(app, xf);
    // The checker's squares: one art pixel when they read well, else bigger.
    let cell = if zoom >= 8.0 { 1.0 } else { (8.0 / zoom.max(0.01)).ceil() };
    let q = screen_quad(app, xf, w, h);
    // Tile mode: the sprite repeated around itself, dimmed, to check it tiles.
    if app.pixel.tile {
        for (dx, dy) in [(-1.0, -1.0), (0.0, -1.0), (1.0, -1.0), (-1.0, 0.0), (1.0, 0.0), (-1.0, 1.0), (0.0, 1.0), (1.0, 1.0)] {
            let moved = xf * Affine::translate((dx * w, dy * h));
            let mq = screen_quad(app, moved, w, h);
            painter.add(quad(
                check,
                mq,
                Rect::from_min_max(pos2(0.0, 0.0), pos2((w / f64::from(cell)) as f32 / 2.0, (h / f64::from(cell)) as f32 / 2.0)),
                Color32::from_gray(150),
            ));
            painter.add(quad(tex, mq, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::from_white_alpha(170)));
        }
    }
    painter.add(quad(
        check,
        q,
        Rect::from_min_max(pos2(0.0, 0.0), pos2((w / f64::from(cell)) as f32 / 2.0, (h / f64::from(cell)) as f32 / 2.0)),
        Color32::WHITE,
    ));
    onion(app, &ctx, painter, id, &d, q);
    painter.add(quad(tex, q, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE));
    if app.pixel.grid && zoom >= GRID_FROM {
        grid(app, painter, xf, w, h, zoom);
    }
    // Only the first frame makes the font: the guide shows there.
    if d.frame == 0 {
        glyph_guide(app, painter, id, xf, (w, h), &t);
    }
    let outline = Stroke::new(1.0, t.accent);
    painter.add(Shape::closed_line(q.to_vec(), outline));
    brush_preview(app, painter, xf, &d, zoom);
    selection_outline(app, painter, xf, &d);
    axes(app, painter, xf, w, h, &t);
    super::edges::draw(app, painter, id, &d.sprite);
}

/// The onion skin (G16.4): the frame before (red) and after (blue), faint, under the one drawn on.
fn onion(app: &mut FigApp, ctx: &egui::Context, painter: &egui::Painter, id: figcraft_doc::NodeId, d: &super::Decoded, q: [Pos2; 4]) {
    if !app.pixel.onion || app.pixel.playing.is_some() {
        return;
    }
    let frames = d.sprite.frames.len();
    let around = [
        (d.frame.checked_sub(1), Color32::from_rgba_unmultiplied(255, 70, 70, 80)),
        ((d.frame + 1 < frames).then_some(d.frame + 1), Color32::from_rgba_unmultiplied(70, 130, 255, 80)),
    ];
    let mut keep = Vec::new();
    for (frame, tint) in around {
        let Some(f) = frame else { continue };
        let Some(n) = app.session.doc().ok().and_then(|doc| doc.node(id)) else { return };
        let key = figcraft_pixel::store::frame_hash(&n.plugin_data, &d.sprite, f);
        let tex = match app.pixel.onion_tex.iter().find(|(k, _)| *k == key) {
            Some((_, t)) => t.clone(),
            None => {
                let Some(pic) = figcraft_pixel::store::frame_image(&n.plugin_data, &d.sprite, f) else { continue };
                let img = ColorImage::from_rgba_unmultiplied([pic.width() as usize, pic.height() as usize], &pic.to_bytes());
                ctx.load_texture(format!("graybox-pixel-onion-{key}"), img, TextureOptions::NEAREST)
            }
        };
        painter.add(quad(tex.id(), q, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), tint));
        keep.push((key, tex));
    }
    app.pixel.onion_tex = keep;
}

/// What the picture on screen depends on besides the cel: the stroke or shape in progress and the
/// settings it is drawn with (Shift squares a shape).
fn preview_key(app: &FigApp, base: u64, shift: bool) -> u64 {
    use std::hash::{Hash, Hasher};
    let e = &app.pixel;
    let Some(g) = &e.gesture else { return base };
    let mut h = std::collections::hash_map::DefaultHasher::new();
    base.hash(&mut h);
    match g {
        Gesture::Stroke { points, secondary, erase } => (0u8, points, secondary, erase).hash(&mut h),
        Gesture::Shape { from, to, secondary } => (1u8, from, to, secondary, shift, e.filled).hash(&mut h),
        Gesture::Move { from, to } => (2u8, from, to).hash(&mut h),
        _ => return base,
    }
    (e.tool, e.colors, e.brush.size, e.brush.shape == figcraft_pixel::BrushShape::Circle, e.pixel_perfect, e.shade, e.mirror, e.axis, e.tile)
        .hash(&mut h);
    h.finish()
}

/// The cel with the stroke or shape in progress painted on it.
fn preview_cel(app: &FigApp, d: &super::Decoded, shift: bool) -> Canvas {
    let mut c = d.cel.clone();
    match &app.pixel.gesture {
        Some(Gesture::Stroke { points, secondary, erase }) => {
            let perfect = app.pixel.pixel_perfect && app.pixel.brush.size == 1;
            let (path, ink) = (freehand(points, perfect), ink(&app.pixel, *secondary, *erase));
            match super::shade_step(&app.pixel, *secondary, *erase) {
                0 => paint(&mut c, &path, &ink),
                step => {
                    let ramp = figcraft_pixel::paint::shade_ramp(&d.sprite.palette, &c);
                    figcraft_pixel::paint::shade(&mut c, &path, &ink, &ramp, step)
                }
            };
        }
        Some(Gesture::Shape { from, to, secondary }) => {
            let kind = match app.pixel.tool {
                Tool::Line => "line",
                Tool::Ellipse => "ellipse",
                _ => "rect",
            };
            if let Some(path) = figcraft_engine::cmd::pixel::shape_path(kind, *from, *to, app.pixel.filled, shift) {
                paint(&mut c, &path, &ink(&app.pixel, *secondary, false));
            }
        }
        Some(Gesture::Move { from, to }) => {
            let change = Change::Move { dx: to.0 - from.0, dy: to.1 - from.1 };
            transform::apply(&mut c, d.selection.as_ref(), change);
            return c;
        }
        _ => return c,
    }
    // Drawing stays inside the selection, as the command keeps it.
    if let Some(mask) = &d.selection {
        transform::clip_to(&mut c, &d.cel, mask);
    }
    c
}

/// On a glyph sheet (G16.6): each cell's character in its corner and its baseline, so the artist
/// sees which letter goes where. Hidden while cells are too small on screen to read.
fn glyph_guide(app: &FigApp, painter: &egui::Painter, id: figcraft_doc::NodeId, xf: Affine, (w, h): (f64, f64), t: &Tokens) {
    use figcraft_pixel::font::{self, Font};
    let Ok(doc) = app.session.doc() else { return };
    let Some(f) = doc.node(id).and_then(|n| n.plugin_data.get(font::KEY)).and_then(|s| Font::load(s)) else { return };
    let (cw, ch) = (f64::from(f.cell_width), f64::from(f.cell_height));
    let cols = (w / cw).floor().max(0.0) as u32;
    let cell_px = art_to_screen(app, xf, 0.0, 0.0).distance(art_to_screen(app, xf, 0.0, ch));
    if cols == 0 || cell_px < 20.0 {
        return;
    }
    let size = (cell_px * 0.22).clamp(8.0, 13.0);
    let clip = painter.clip_rect();
    let base = Stroke::new(1.0, t.accent.gamma_multiply(0.45));
    // A small dark chip under each label, so it reads on the checkerboard and on the art.
    let chip = Color32::from_black_alpha(130);
    let ink = Color32::from_white_alpha(210);
    for (i, c) in f.chars.chars().enumerate().take(font::MAX_CHARS) {
        let i = i as u32;
        let (x0, y0) = (f64::from(i % cols) * cw, f64::from(i / cols) * ch);
        if y0 + ch > h {
            break;
        }
        let (a, b) = (art_to_screen(app, xf, x0, y0), art_to_screen(app, xf, x0 + cw, y0 + ch));
        if !Rect::from_two_pos(a, b).intersects(clip) {
            continue;
        }
        let by = y0 + f64::from(f.baseline);
        painter.line_segment([art_to_screen(app, xf, x0, by), art_to_screen(app, xf, x0 + cw, by)], base);
        if !c.is_whitespace() && !c.is_control() {
            let galley = painter.layout_no_wrap(c.to_string(), egui::FontId::proportional(size), ink);
            let at = a + egui::vec2(1.0, 1.0);
            painter.rect_filled(Rect::from_min_size(at, galley.size() + egui::vec2(4.0, 0.0)), egui::CornerRadius::same(2), chip);
            painter.galley(at + egui::vec2(2.0, 0.0), galley, ink);
        }
    }
}

/// The pixel grid: a faint line between every art pixel (only lines on screen are drawn).
fn grid(app: &FigApp, painter: &egui::Painter, xf: Affine, w: f64, h: f64, zoom: f32) {
    let alpha = ((zoom - GRID_FROM) * 12.0).clamp(20.0, 60.0) as u8;
    let line = Stroke::new(1.0, Color32::from_rgba_unmultiplied(40, 40, 40, alpha));
    let clip = painter.clip_rect();
    for x in 1..(w as i64) {
        let (a, b) = (art_to_screen(app, xf, x as f64, 0.0), art_to_screen(app, xf, x as f64, h));
        if Rect::from_two_pos(a, b).intersects(clip) {
            painter.line_segment([a, b], line);
        }
    }
    for y in 1..(h as i64) {
        let (a, b) = (art_to_screen(app, xf, 0.0, y as f64), art_to_screen(app, xf, w, y as f64));
        if Rect::from_two_pos(a, b).intersects(clip) {
            painter.line_segment([a, b], line);
        }
    }
}

/// The pixels the brush would paint under the pointer (mirror images too), in the colour.
fn brush_preview(app: &FigApp, painter: &egui::Painter, xf: Affine, d: &super::Decoded, zoom: f32) {
    if app.pixel.gesture.is_some() {
        return;
    }
    let Some(at) = app.pixel.hover else { return };
    let alt = painter.ctx().input(|i| i.modifiers.alt);
    let tool = if alt { Tool::Eyedropper } else { app.pixel.tool };
    let single = matches!(tool, Tool::Bucket | Tool::Eyedropper);
    // Shade paints no colour of its own: the brush shows as outlines only.
    let shading = !alt && super::shade_step(&app.pixel, false, false) != 0;
    let mut i = ink(&app.pixel, false, tool == Tool::Eraser);
    if single {
        i.brush = figcraft_pixel::Brush::default();
        i.symmetry = Default::default();
    }
    let cells = covered(&[at], &i, d.sprite.width, d.sprite.height);
    let fill: Rgba = i.color;
    let tint = if tool == Tool::Eraser || single || shading {
        Color32::TRANSPARENT
    } else {
        Color32::from_rgba_unmultiplied(fill[0], fill[1], fill[2], (u16::from(fill[3]) * 3 / 4) as u8)
    };
    let edge = Stroke::new(1.0, Color32::from_white_alpha(200));
    let shadow = Stroke::new(1.0, Color32::from_black_alpha(160));
    for (x, y) in cells.iter().take(4096) {
        let r = [
            art_to_screen(app, xf, *x as f64, *y as f64),
            art_to_screen(app, xf, *x as f64 + 1.0, *y as f64),
            art_to_screen(app, xf, *x as f64 + 1.0, *y as f64 + 1.0),
            art_to_screen(app, xf, *x as f64, *y as f64 + 1.0),
        ];
        if tint != Color32::TRANSPARENT {
            painter.add(Shape::convex_polygon(r.to_vec(), tint, Stroke::NONE));
        }
        if single || shading || zoom >= 4.0 && cells.len() <= 4 {
            painter.add(Shape::closed_line(r.to_vec(), shadow));
            painter.add(Shape::closed_line(r.iter().map(|p| *p + egui::vec2(0.5, 0.5)).collect(), edge));
        }
    }
}

/// The mirror axes across the sprite, each with a round handle past the sprite's edge to drag.
fn axes(app: &FigApp, painter: &egui::Painter, xf: Affine, w: f64, h: f64, t: &Tokens) {
    let color = Color32::from_rgb(0xff, 0x4f, 0xb8);
    let line = Stroke::new(1.5, color);
    for which in 0..2 {
        if !app.pixel.mirror[which] {
            continue;
        }
        let v = app.pixel.axis[which] as f64 / 2.0;
        let (a, b) = if which == 0 {
            (art_to_screen(app, xf, v, 0.0), art_to_screen(app, xf, v, h))
        } else {
            (art_to_screen(app, xf, 0.0, v), art_to_screen(app, xf, w, v))
        };
        painter.add(Shape::dashed_line(&[a, b], line, 6.0, 4.0));
        let knob = axis_knob(app, xf, which, v);
        painter.line_segment([a, knob], line);
        painter.circle(knob, 6.0, color, Stroke::new(1.5, t.panel));
    }
    // Where both cross, a ring to drag both at once (G16.7).
    if app.pixel.mirror == [true, true] {
        let cross = art_to_screen(app, xf, app.pixel.axis[0] as f64 / 2.0, app.pixel.axis[1] as f64 / 2.0);
        painter.circle(cross, 5.0, t.panel, Stroke::new(2.0, color));
    }
}

/// Where mirror axis `which` (at `v` art pixels) has its handle on screen: 14 points past the
/// sprite's top edge (the vertical axis) or left edge (the horizontal one).
pub(super) fn axis_knob(app: &FigApp, xf: Affine, which: usize, v: f64) -> Pos2 {
    if which == 0 { art_to_screen(app, xf, v, 0.0) + egui::vec2(0.0, -14.0) } else { art_to_screen(app, xf, 0.0, v) + egui::vec2(-14.0, 0.0) }
}

/// The selection's outline, marching (white and black dashes that move), where it is or where the
/// Move tool is taking it; and the marquee or lasso being drawn.
fn selection_outline(app: &FigApp, painter: &egui::Painter, xf: Affine, d: &super::Decoded) {
    let ctx = painter.ctx();
    let phase = (ctx.input(|i| i.time) * 8.0).rem_euclid(8.0) as f32;
    let white = Stroke::new(1.0, Color32::WHITE);
    let black = Stroke::new(1.0, Color32::BLACK);
    let ants = |a: Pos2, b: Pos2| {
        painter.line_segment([a, b], white);
        let len = a.distance(b);
        if len <= 0.0 {
            return;
        }
        let dir = (b - a) / len;
        // Black dashes 4 px long every 8 px, shifted along with time.
        let mut s = -phase;
        while s < len {
            let (s0, s1) = (s.max(0.0), (s + 4.0).min(len));
            if s1 > s0 {
                painter.line_segment([a + dir * s0, a + dir * s1], black);
            }
            s += 8.0;
        }
    };
    let (dx, dy) = match &app.pixel.gesture {
        Some(Gesture::Move { from, to }) if d.selection.is_some() => ((to.0 - from.0) as f64, (to.1 - from.1) as f64),
        _ => (0.0, 0.0),
    };
    if !d.edges.is_empty() {
        for ((x0, y0), (x1, y1)) in d.edges.iter().take(200_000) {
            ants(art_to_screen(app, xf, *x0 as f64 + dx, *y0 as f64 + dy), art_to_screen(app, xf, *x1 as f64 + dx, *y1 as f64 + dy));
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(80));
    }
    match &app.pixel.gesture {
        Some(Gesture::Select { tool: Tool::Marquee, points, .. }) => {
            if let (Some(a), Some(b)) = (points.first(), points.last()) {
                let (x0, y0, x1, y1) = (a.0.min(b.0) as f64, a.1.min(b.1) as f64, a.0.max(b.0) as f64 + 1.0, a.1.max(b.1) as f64 + 1.0);
                let q = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)].map(|(x, y)| art_to_screen(app, xf, x, y));
                for (a, b) in q.iter().zip(q.iter().cycle().skip(1)) {
                    ants(*a, *b);
                }
            }
        }
        Some(Gesture::Select { points, .. }) => {
            let at = |p: &(i64, i64)| art_to_screen(app, xf, p.0 as f64 + 0.5, p.1 as f64 + 0.5);
            for w in points.windows(2) {
                if let [a, b] = w {
                    ants(at(a), at(b));
                }
            }
        }
        _ => {}
    }
}
