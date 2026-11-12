//! Pixel mode on the canvas: the sprite over a checkerboard, sharp; the pixel grid when zoomed in;
//! the brush under the pointer (and its mirror images); the mirror axes with their handles; tile
//! mode's copies around the sprite; the box of a sprite being dragged out.

use egui::epaint::{Mesh, Vertex};
use egui::{Color32, ColorImage, Pos2, Rect, Shape, Stroke, TextureOptions, pos2};
use figcraft_geom::Affine;
use figcraft_pixel::paint::{covered, freehand, paint};
use figcraft_pixel::{Canvas, Rgba};

use super::{Gesture, Mode, Tool, art_to_page, art_to_screen, decode, ink, screen_quad, settle, zoom_of};
use crate::FigApp;
use crate::theme::Tokens;

/// Screen pixels an art pixel must span before the pixel grid shows.
const GRID_FROM: f32 = 6.0;

/// Pixel mode's drawing over the canvas.
pub(crate) fn draw(app: &mut FigApp, painter: &egui::Painter) {
    settle(app);
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
    painter.add(quad(tex, q, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE));
    if app.pixel.grid && zoom >= GRID_FROM {
        grid(app, painter, xf, w, h, zoom);
    }
    let outline = Stroke::new(1.0, t.accent);
    painter.add(Shape::closed_line(q.to_vec(), outline));
    brush_preview(app, painter, xf, &d, zoom);
    axes(app, painter, xf, w, h, &t);
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
        Gesture::Shape { from, to, secondary } => (1u8, from, to, secondary, shift, e.filled, e.tool.label()).hash(&mut h),
        _ => return base,
    }
    (e.colors, e.brush.size, e.brush.shape == figcraft_pixel::BrushShape::Circle, e.pixel_perfect, e.mirror, e.axis, e.tile).hash(&mut h);
    h.finish()
}

/// The cel with the stroke or shape in progress painted on it.
fn preview_cel(app: &FigApp, d: &super::Decoded, shift: bool) -> Canvas {
    let mut c = d.cel.clone();
    match &app.pixel.gesture {
        Some(Gesture::Stroke { points, secondary, erase }) => {
            let perfect = app.pixel.pixel_perfect && app.pixel.brush.size == 1;
            paint(&mut c, &freehand(points, perfect), &ink(&app.pixel, *secondary, *erase));
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
        _ => {}
    }
    c
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
    let mut i = ink(&app.pixel, false, tool == Tool::Eraser);
    if single {
        i.brush = figcraft_pixel::Brush::default();
        i.symmetry = Default::default();
    }
    let cells = covered(&[at], &i, d.sprite.width, d.sprite.height);
    let fill: Rgba = i.color;
    let tint = if tool == Tool::Eraser || single {
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
        if single || zoom >= 4.0 && cells.len() <= 4 {
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
        let outward = if which == 0 { egui::vec2(0.0, -14.0) } else { egui::vec2(-14.0, 0.0) };
        painter.line_segment([a, a + outward], line);
        painter.circle(a + outward, 5.0, color, Stroke::new(1.5, t.panel));
    }
}
