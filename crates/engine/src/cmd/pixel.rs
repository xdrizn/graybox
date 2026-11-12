//! Graybox pixel art (G16): the `pixel.*` commands. A pixel layer is a layer whose plugin data
//! holds a sprite ([`figcraft_pixel::store`]); its fills show the sprite's first frame as an image
//! (kept up to date here) so every exporter and older FigCraft still see the art, while the
//! renderer draws the sprite itself, sharp, at a whole-number pixel scale.
//!
//! Every drawing command is one undo step and runs the same code the Pixel mode editor previews
//! with, so the stored pixels are what was shown while drawing.

use figcraft_color::{ImageScaleMode, Paint, PaintKind};
use figcraft_doc::{Document, NodeId};
use figcraft_pixel::paint::{BrushShape, Ink, Symmetry};
use figcraft_pixel::raster::{self, Pt};
use figcraft_pixel::resize::{Resize, SliceFill};
use figcraft_pixel::{Brush, Canvas, Rgba, Sprite, canvas, store};
use serde_json::{Value, json};

use super::CommandSpec;
use crate::automation::params::{boolean, color_of, get, num, opt_id, req_id};
use crate::{Result, Session, bad, spec};

mod anim;
mod edit;
pub(crate) mod kit;

/// The most pointer positions one stroke takes.
const MAX_STROKE_POINTS: usize = 100_000;
/// How far outside the sprite a point may be given (art pixels); further is brought in.
const MAX_COORD: f64 = 1_000_000.0;

pub fn specs() -> Vec<CommandSpec> {
    let mut list = vec![
        spec!(
            "pixel.create",
            "New sprite",
            "{x?, y?, width? (art pixels, default 32), height? (default 32), scale? (canvas pixels per art pixel, default 4), parent?, name?, background? (colour: the first layer starts filled), select? (default true)}  a pixel layer: a sprite drawn sharp at a whole-number scale → {id}",
            true,
            create
        ),
        spec!(
            "pixel.stroke",
            "Draw pixels",
            "{id, points: [[x, y], …] (art pixels, joined by lines), color? (default black), size? (1–64), shape? (square|circle), pixelPerfect? (one-pixel lines without L corners), mirror? {x?, y?} (axes in art pixels, .5 = through a pixel's middle), tile? (wrap round the edges), erase?, layer? (layer id; default the top one), frame? (index, default 0)} → {changed}",
            true,
            stroke
        ),
        spec!(
            "pixel.shape",
            "Draw a pixel shape",
            "{id, kind: line|rect|ellipse, from: [x, y], to: [x, y], filled?, square? (a square or circle box), color?, size?, shape?, mirror?, tile?, erase?, layer?, frame?} → {changed}",
            true,
            shape
        ),
        spec!(
            "pixel.fill",
            "Fill pixels",
            "{id, x, y, color?, contiguous? (default true: only what touches), tolerance? (0–255), tile?, layer?, frame?} → {changed}",
            true,
            fill
        ),
        spec!(
            "pixel.clear",
            "Clear pixels",
            "{id, layer?, frame?}  makes the selected pixels (the whole cel when nothing is selected) transparent → {changed}",
            true,
            clear
        ),
        spec!(
            "pixel.pick",
            "Pick a pixel colour",
            "{id, x, y, layer? (that layer only; default what the frame shows), frame?} → {color (#rrggbb[aa]) | null}",
            false,
            pick
        ),
        spec!(
            "pixel.get",
            "Read a sprite",
            "{id, pixels? (true: each frame's picture as rows of colours, '.' transparent), frame?} → {width, height, scale, layers, frames, tags, palette, rows?}",
            false,
            read
        ),
        spec!(
            "pixel.set",
            "Sprite settings",
            "{id, scale? (1–64: the layer takes width × scale), resize? (middle|edges|tile: how the art regrows when the layer is resized without 9-slice), sliceFill? (tile|stretch: the 9-slice edges and middle), slicePadding? (a frame with auto layout takes the 9-slice cuts as its padding), play? (the tag it plays in Prototype and Play; empty for every frame)} → {scale, resize, sliceFill, slicePadding, play}",
            true,
            settings
        ),
        spec!(
            "pixel.mode",
            "Pixel mode",
            "{id? (a pixel layer; default the selected one), on? (default true)}  edits the sprite in place on the canvas (the editor's own tools)",
            false,
            mode
        ),
    ];
    list.extend(edit::specs());
    list.extend(anim::specs());
    list.extend(kit::specs());
    list
}

/// The pixel layer `id` names and its sprite.
fn sprite_of(cmd: &str, doc: &Document, id: NodeId) -> Result<Sprite> {
    let n = doc.get(id)?;
    // An instance shows its main component's pixels (G16.3): those are the ones to edit.
    if matches!(n.kind, figcraft_doc::NodeKind::Instance { .. }) {
        return Err(bad(cmd, format!("\"{}\" is an instance: draw on its main component", n.name)));
    }
    if !store::is_pixel(&n.plugin_data) {
        return Err(bad(cmd, format!("\"{}\" is not a pixel layer", n.name)));
    }
    store::load(&n.plugin_data).ok_or_else(|| bad(cmd, format!("\"{}\" has a sprite that can't be read", n.name)))
}

/// The sprite's layer id (`layer`, default the top layer) and frame id (`frame` index, default 0).
fn cel_of(cmd: &str, sprite: &Sprite, p: &Value) -> Result<(u32, u32)> {
    let layer = match get(p, "layer") {
        None | Some(Value::Null) => sprite.layers.last().map(|l| l.id),
        Some(v) => v.as_u64().and_then(|i| u32::try_from(i).ok()).filter(|i| sprite.layer_index(*i).is_some()),
    }
    .ok_or_else(|| bad(cmd, "`layer` must be a layer id of this sprite"))?;
    let frame = match get(p, "frame") {
        None | Some(Value::Null) => sprite.frames.first().map(|f| f.id),
        Some(v) => v.as_u64().and_then(|i| usize::try_from(i).ok()).and_then(|i| sprite.frames.get(i)).map(|f| f.id),
    }
    .ok_or_else(|| bad(cmd, format!("`frame` must be a frame index (0–{})", sprite.frames.len().saturating_sub(1))))?;
    Ok((layer, frame))
}

/// The frame index `frame` names (default 0).
fn frame_index(cmd: &str, sprite: &Sprite, p: &Value) -> Result<usize> {
    match get(p, "frame") {
        None | Some(Value::Null) => Ok(0),
        Some(v) => v
            .as_u64()
            .and_then(|i| usize::try_from(i).ok())
            .filter(|i| *i < sprite.frames.len())
            .ok_or_else(|| bad(cmd, format!("`frame` must be a frame index (0–{})", sprite.frames.len().saturating_sub(1)))),
    }
}

/// A point `[x, y]` in art pixels (rounded down, brought within reach).
fn point_of(v: &Value) -> Option<Pt> {
    let a = v.as_array()?;
    let c = |v: Option<&Value>| v.and_then(Value::as_f64).filter(|f| f.is_finite()).map(|f| f.clamp(-MAX_COORD, MAX_COORD).floor() as i64);
    match a.as_slice() {
        [x, y] => Some((c(Some(x))?, c(Some(y))?)),
        _ => None,
    }
}

fn req_point(cmd: &str, p: &Value, key: &str) -> Result<Pt> {
    get(p, key).and_then(point_of).ok_or_else(|| bad(cmd, format!("`{key}` must be [x, y]")))
}

/// The ink `p` asks for: colour, brush, mirror, tile mode, eraser.
fn ink_of(cmd: &str, p: &Value) -> Result<Ink> {
    let color = match get(p, "color") {
        None | Some(Value::Null) => [0, 0, 0, 255],
        Some(v) => rgba_of(v).ok_or_else(|| bad(cmd, "`color` must be a colour (#rrggbb[aa], rgb(…) or {r, g, b, a})"))?,
    };
    let size = match get(p, "size") {
        None | Some(Value::Null) => 1,
        Some(v) => v
            .as_f64()
            .filter(|f| f.is_finite())
            .map(|f| f.clamp(1.0, f64::from(Brush::MAX_SIZE)) as u32)
            .ok_or_else(|| bad(cmd, "`size` must be 1–64"))?,
    };
    let shape = match get(p, "shape").and_then(Value::as_str) {
        None => BrushShape::Square,
        Some("square") => BrushShape::Square,
        Some("circle" | "round") => BrushShape::Circle,
        Some(_) => return Err(bad(cmd, "`shape` must be square or circle")),
    };
    let replace = match get(p, "replace") {
        None | Some(Value::Null) => None,
        Some(v) => Some(rgba_of(v).ok_or_else(|| bad(cmd, "`replace` must be a colour"))?),
    };
    let mut symmetry = Symmetry::default();
    match get(p, "mirror") {
        None | Some(Value::Null) => {}
        Some(Value::Object(m)) => {
            let axis = |k: &str| -> Result<Option<i64>> {
                match m.get(k) {
                    None | Some(Value::Null) => Ok(None),
                    Some(v) => v
                        .as_f64()
                        .filter(|f| f.is_finite())
                        .map(|f| Some((f * 2.0).round().clamp(-8192.0, 8192.0) as i64))
                        .ok_or_else(|| bad(cmd, format!("`mirror.{k}` must be a number"))),
                }
            };
            symmetry.x2 = axis("x")?;
            symmetry.y2 = axis("y")?;
        }
        Some(_) => return Err(bad(cmd, "`mirror` must be {x?, y?}")),
    }
    Ok(Ink {
        color,
        brush: Brush::new(size, shape),
        symmetry,
        tile: boolean(p, "tile").unwrap_or(false),
        erase: boolean(p, "erase").unwrap_or(false),
        replace,
    })
}

/// A colour param as a pixel.
fn rgba_of(v: &Value) -> Option<Rgba> {
    if let Some(s) = v.as_str()
        && let Some(c) = canvas::parse_hex(s)
    {
        return Some(c);
    }
    color_of(v).map(|c| c.to_rgba8())
}

/// Runs `f` on the cel of layer `layer` in frame `frame` of pixel layer `id`, keeps what it did
/// inside the sprite's selection (all of it when nothing is selected), stores the cel and shows the
/// new picture. How many pixels changed.
fn edit_cel(s: &mut Session, id: NodeId, sprite: &Sprite, (layer, frame): (u32, u32), f: impl FnOnce(&mut Canvas) -> usize) -> Result<usize> {
    let doc = s.doc_mut()?;
    let n = doc.get(id)?;
    let Some(mut cel) = store::cel(&n.plugin_data, sprite, layer, frame) else { return Ok(0) };
    let mut changed = f(&mut cel);
    if changed > 0
        && let Some(mask) = store::selection(&n.plugin_data, sprite)
        && let Some(before) = store::cel(&n.plugin_data, sprite, layer, frame)
    {
        changed = figcraft_pixel::transform::clip_to(&mut cel, &before, &mask);
    }
    if changed > 0 {
        store::put_cel(&mut doc.get_mut(id)?.plugin_data, layer, frame, &cel);
        show(doc, id, sprite);
    }
    Ok(changed)
}

/// Puts the frame the sprite shows at rest (the first its played tag shows) in layer `id`'s image
/// fill (adding one if it has none), so the art shows wherever the sprite itself isn't drawn
/// (exports, older FigCraft). The picture it
/// replaces leaves the document when nothing else shows it (undo brings back the document that had it).
pub fn show(doc: &mut Document, id: NodeId, sprite: &Sprite) {
    let Some(n) = doc.node(id) else { return };
    let Some(png) = store::frame_image(&n.plugin_data, sprite, figcraft_pixel::anim::frame_at(sprite, 0.0).0).and_then(|c| c.to_png()) else {
        return;
    };
    let Ok((hash, _, _)) = figcraft_format::add_image(doc, png) else { return };
    let Some(n) = doc.node_mut(id) else { return };
    let old = match n.fills.iter_mut().rev().find_map(|f| match &mut f.kind {
        PaintKind::Image(img) => Some(img),
        _ => None,
    }) {
        Some(img) => Some(std::mem::replace(&mut img.image, hash.clone())),
        None => {
            n.fills.push(Paint::image(hash.clone(), ImageScaleMode::Fill));
            None
        }
    };
    if let Some(old) = old.filter(|o| *o != hash)
        && !figcraft_doc::used_images(doc).contains(&old)
    {
        doc.images.remove(&old);
    }
}

fn create(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "pixel.create";
    let whole = |key: &str, d: u32, max: u32| -> Result<u32> {
        match get(p, key) {
            None | Some(Value::Null) => Ok(d),
            Some(v) => v
                .as_f64()
                .filter(|f| f.is_finite() && *f >= 1.0 && *f <= f64::from(max))
                .map(|f| f.round() as u32)
                .ok_or_else(|| bad(cmd, format!("`{key}` must be a whole number from 1 to {max}"))),
        }
    };
    let (w, h) = (whole("width", 32, Canvas::MAX_SIDE)?, whole("height", 32, Canvas::MAX_SIDE)?);
    let scale = whole("scale", 4, figcraft_pixel::sprite::MAX_SCALE)?;
    let background = match get(p, "background") {
        None | Some(Value::Null) => None,
        Some(v) => Some(rgba_of(v).ok_or_else(|| bad(cmd, "`background` must be a colour"))?),
    };
    let parent = opt_id(cmd, p, "parent")?;
    let sprite = Sprite::new(w, h, scale).ok_or_else(|| bad(cmd, "that sprite size can't be made"))?;
    let name = get(p, "name").and_then(Value::as_str).map_or_else(|| format!("Sprite {w}×{h}"), str::to_string);
    let mut params = json!({
        "type": "rectangle",
        "x": num(p, "x").filter(|f| f.is_finite()).unwrap_or(0.0).round(),
        "y": num(p, "y").filter(|f| f.is_finite()).unwrap_or(0.0).round(),
        "width": f64::from(w * scale),
        "height": f64::from(h * scale),
        "name": name,
        "select": boolean(p, "select").unwrap_or(true),
    });
    if let (Some(parent), Some(o)) = (parent, params.as_object_mut()) {
        o.insert("parent".into(), json!(parent.0));
    }
    let made = super::find_command("node.create").ok_or_else(|| bad(cmd, "can't make layers"))?;
    let r = (made.run)(s, &params)?;
    let id = crate::automation::params::id(&r, "id").ok_or_else(|| bad(cmd, "the layer wasn't made"))?;
    let doc = s.doc_mut()?;
    {
        let n = doc.get_mut(id)?;
        n.fills.clear();
        n.strokes.clear();
        store::save(&mut n.plugin_data, &sprite);
        if let (Some(bg), Some(layer), Some(frame)) = (background, sprite.layers.first(), sprite.frames.first())
            && let Some(cel) = Canvas::filled(w, h, bg)
        {
            store::put_cel(&mut n.plugin_data, layer.id, frame.id, &cel);
        }
    }
    show(doc, id, &sprite);
    Ok(json!({"id": id.0}))
}

fn stroke(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "pixel.stroke";
    let id = req_id(cmd, p, "id")?;
    let sprite = sprite_of(cmd, s.doc()?, id)?;
    let cel = cel_of(cmd, &sprite, p)?;
    let points: Vec<Pt> = match get(p, "points") {
        Some(Value::Array(a)) if !a.is_empty() => {
            a.iter().take(MAX_STROKE_POINTS).map(point_of).collect::<Option<_>>().ok_or_else(|| bad(cmd, "`points` must be [[x, y], …]"))?
        }
        _ => return Err(bad(cmd, "`points` must be [[x, y], …] with at least one point")),
    };
    let ink = ink_of(cmd, p)?;
    let perfect = boolean(p, "pixelPerfect").unwrap_or(false);
    let path = figcraft_pixel::paint::freehand(&points, perfect);
    let changed = edit_cel(s, id, &sprite, cel, |c| figcraft_pixel::paint::paint(c, &path, &ink))?;
    Ok(json!({"changed": changed}))
}

/// The pixels of shape `kind` in the box from `a` to `b`.
pub fn shape_path(kind: &str, a: Pt, b: Pt, filled: bool, square: bool) -> Option<Vec<Pt>> {
    let b = if square { raster::squared(a, b) } else { b };
    Some(match (kind, filled) {
        ("line", _) => raster::line(a, if square { snap_45(a, b) } else { b }),
        ("rect" | "rectangle", false) => raster::rect_outline(a, b),
        ("rect" | "rectangle", true) => raster::rect_filled(a, b),
        ("ellipse", false) => raster::ellipse_outline(a, b),
        ("ellipse", true) => raster::ellipse_filled(a, b),
        _ => return None,
    })
}

/// `b` moved onto the nearest 45° line through `a` (Shift with the line tool).
fn snap_45(a: Pt, b: Pt) -> Pt {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    if dx.abs() > 2 * dy.abs() {
        (b.0, a.1)
    } else if dy.abs() > 2 * dx.abs() {
        (a.0, b.1)
    } else {
        let d = dx.abs().max(dy.abs());
        (a.0 + d * dx.signum(), a.1 + d * dy.signum())
    }
}

fn shape(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "pixel.shape";
    let id = req_id(cmd, p, "id")?;
    let sprite = sprite_of(cmd, s.doc()?, id)?;
    let cel = cel_of(cmd, &sprite, p)?;
    let (a, b) = (req_point(cmd, p, "from")?, req_point(cmd, p, "to")?);
    let kind = get(p, "kind").and_then(Value::as_str).unwrap_or("");
    // A line's box is its own: Shift snaps it to 45° steps instead of a square.
    let square = boolean(p, "square").unwrap_or(false);
    let path =
        shape_path(kind, a, b, boolean(p, "filled").unwrap_or(false), square).ok_or_else(|| bad(cmd, "`kind` must be line, rect or ellipse"))?;
    let ink = ink_of(cmd, p)?;
    let changed = edit_cel(s, id, &sprite, cel, |c| figcraft_pixel::paint::paint(c, &path, &ink))?;
    Ok(json!({"changed": changed}))
}

fn fill(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "pixel.fill";
    let id = req_id(cmd, p, "id")?;
    let sprite = sprite_of(cmd, s.doc()?, id)?;
    let cel = cel_of(cmd, &sprite, p)?;
    let at = (
        num(p, "x").filter(|f| f.is_finite()).ok_or_else(|| bad(cmd, "`x` must be a number"))?.clamp(-MAX_COORD, MAX_COORD).floor() as i64,
        num(p, "y").filter(|f| f.is_finite()).ok_or_else(|| bad(cmd, "`y` must be a number"))?.clamp(-MAX_COORD, MAX_COORD).floor() as i64,
    );
    let ink = ink_of(cmd, p)?;
    let color = if ink.erase { figcraft_pixel::CLEAR } else { ink.color };
    let tolerance = num(p, "tolerance").filter(|f| f.is_finite()).map_or(0, |f| f.clamp(0.0, 255.0) as u8);
    let contiguous = boolean(p, "contiguous").unwrap_or(true);
    let changed = edit_cel(s, id, &sprite, cel, |c| figcraft_pixel::paint::fill(c, at, color, contiguous, tolerance, ink.tile))?;
    Ok(json!({"changed": changed}))
}

fn clear(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "pixel.clear";
    let id = req_id(cmd, p, "id")?;
    let sprite = sprite_of(cmd, s.doc()?, id)?;
    let cel = cel_of(cmd, &sprite, p)?;
    let changed = edit_cel(s, id, &sprite, cel, |c| {
        let n = c.pixels().iter().filter(|p| p[3] > 0).count();
        if let Some(clear) = Canvas::new(c.width(), c.height()) {
            *c = clear;
        }
        n
    })?;
    Ok(json!({"changed": changed}))
}

fn pick(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "pixel.pick";
    let id = req_id(cmd, p, "id")?;
    let doc = s.doc()?;
    let sprite = sprite_of(cmd, doc, id)?;
    let x = num(p, "x").filter(|f| f.is_finite()).ok_or_else(|| bad(cmd, "`x` must be a number"))?.clamp(-MAX_COORD, MAX_COORD).floor() as i64;
    let y = num(p, "y").filter(|f| f.is_finite()).ok_or_else(|| bad(cmd, "`y` must be a number"))?.clamp(-MAX_COORD, MAX_COORD).floor() as i64;
    let data = &doc.get(id)?.plugin_data;
    let picture = match get(p, "layer") {
        None | Some(Value::Null) => store::frame_image(data, &sprite, frame_index(cmd, &sprite, p)?),
        Some(_) => {
            let (layer, frame) = cel_of(cmd, &sprite, p)?;
            store::cel(data, &sprite, layer, frame)
        }
    };
    let color = picture.and_then(|c| c.get(x, y)).filter(|c| c[3] > 0).map(canvas::to_hex);
    Ok(json!({"color": color}))
}

fn read(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "pixel.get";
    let id = req_id(cmd, p, "id")?;
    let doc = s.doc()?;
    // An instance shows its main's pixels: those are read.
    let id = doc.node(id).and_then(|n| figcraft_render::pixel::source(doc, n)).map_or(id, |n| n.id);
    let sprite = sprite_of(cmd, doc, id)?;
    let mut out = serde_json::to_value(&sprite).map_err(|e| bad(cmd, e.to_string()))?;
    if boolean(p, "pixels").unwrap_or(false) {
        let data = &doc.get(id)?.plugin_data;
        let frames: Vec<usize> = match get(p, "frame") {
            None | Some(Value::Null) => (0..sprite.frames.len().min(16)).collect(),
            Some(_) => vec![frame_index(cmd, &sprite, p)?],
        };
        let mut pictures = Vec::new();
        for i in frames {
            let Some(c) = store::frame_image(data, &sprite, i) else { continue };
            pictures.push(json!({"frame": i, "rows": rows(&c)}));
        }
        if let Some(o) = out.as_object_mut() {
            o.insert("pictures".into(), Value::Array(pictures));
        }
    }
    Ok(out)
}

/// A picture as rows of colours: each pixel `#rrggbb[aa]`, or `.` when transparent.
fn rows(c: &Canvas) -> Vec<Vec<String>> {
    (0..i64::from(c.height()))
        .map(|y| (0..i64::from(c.width())).map(|x| c.get(x, y).filter(|p| p[3] > 0).map_or_else(|| ".".to_string(), canvas::to_hex)).collect())
        .collect()
}

fn settings(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "pixel.set";
    let id = req_id(cmd, p, "id")?;
    let mut sprite = sprite_of(cmd, s.doc()?, id)?;
    if let Some(v) = get(p, "scale").filter(|v| !v.is_null()) {
        let scale = v
            .as_f64()
            .filter(|f| f.is_finite() && *f >= 1.0 && *f <= f64::from(figcraft_pixel::sprite::MAX_SCALE))
            .ok_or_else(|| bad(cmd, "`scale` must be a whole number from 1 to 64"))?
            .round() as u32;
        let old = sprite.scale.max(1);
        sprite.scale = scale;
        let doc = s.doc_mut()?;
        let n = doc.get(id)?;
        // The layer keeps how many art pixels it shows (a resized layer stays resized).
        let (aw, ah) = ((n.width / f64::from(old)).round().max(1.0), (n.height / f64::from(old)).round().max(1.0));
        store::save(&mut doc.get_mut(id)?.plugin_data, &sprite);
        kit::rescale(doc, id, &sprite, old)?;
        super::arrange::resize_cmd(s, &json!({"ids": [id.0], "width": aw * f64::from(scale), "height": ah * f64::from(scale)}))?;
    }
    let text = |key: &str| get(p, key).filter(|v| !v.is_null()).map(|v| v.as_str().unwrap_or_default().to_string());
    if let Some(v) = text("resize") {
        sprite.resize = Resize::parse(&v).ok_or_else(|| bad(cmd, "`resize` must be middle, edges or tile"))?;
    }
    if let Some(v) = text("sliceFill") {
        sprite.slice_fill = SliceFill::parse(&v).ok_or_else(|| bad(cmd, "`sliceFill` must be tile or stretch"))?;
    }
    let repad = match get(p, "slicePadding").filter(|v| !v.is_null()) {
        None => false,
        Some(v) => {
            sprite.slice_padding = v.as_bool().ok_or_else(|| bad(cmd, "`slicePadding` must be true or false"))?;
            sprite.slice_padding
        }
    };
    // What it plays in Prototype and Play (G16.4): a tag's name, or "" for every frame.
    if let Some(v) = get(p, "play").filter(|v| !v.is_null()) {
        let v = v.as_str().ok_or_else(|| bad(cmd, "`play` must be a tag's name, or empty for every frame"))?.to_string();
        if !v.is_empty() && sprite.tag(&v).is_none() {
            return Err(bad(cmd, format!("no tag called \"{v}\"")));
        }
        sprite.play = (!v.is_empty()).then_some(v);
    }
    let doc = s.doc_mut()?;
    store::save(&mut doc.get_mut(id)?.plugin_data, &sprite);
    show(doc, id, &sprite);
    if repad {
        kit::repad(doc, id, &sprite)?;
    }
    Ok(json!({
        "scale": sprite.scale,
        "resize": label(&sprite.resize),
        "sliceFill": label(&sprite.slice_fill),
        "slicePadding": sprite.slice_padding,
        "play": sprite.play,
    }))
}

/// A setting as its JSON name.
fn label<T: serde::Serialize>(v: &T) -> Value {
    serde_json::to_value(v).unwrap_or(Value::Null)
}

fn mode(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "pixel.mode";
    let on = boolean(p, "on").unwrap_or(true);
    let id = match opt_id(cmd, p, "id")? {
        Some(id) => Some(id),
        None => s.selection().first().copied(),
    };
    if on {
        let id = id.ok_or_else(|| bad(cmd, "Select a pixel layer"))?;
        sprite_of(cmd, s.doc()?, id)?;
        s.request_ui("pixel.mode", json!({"id": id.0, "on": true}));
    } else {
        s.request_ui("pixel.mode", json!({"on": false}));
    }
    Ok(json!({"on": on}))
}
