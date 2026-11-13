//! Graybox pixel art (G16.2): selecting, transforming and recolouring a sprite's pixels, its
//! layers, its palette, and copy and paste. The selection is part of the sprite (it is saved and
//! undone with it); drawing commands draw only inside it, and the transform commands change
//! what it holds.

use base64::Engine as _;
use figcraft_doc::NodeId;
use figcraft_pixel::select::{Mask, Op};
use figcraft_pixel::transform::{self, Change};
use figcraft_pixel::{Canvas, Rgba, Sprite, canvas, palette, store};
use serde_json::{Value, json};

use super::{cel_of, edit_cel, point_of, req_point, rgba_of, show, sprite_of};
use crate::automation::params::{boolean, get, num, opt_id, req_id};
use crate::cmd::CommandSpec;
use crate::{Result, Session, bad, spec};

/// The most lasso points one selection takes.
const MAX_LASSO_POINTS: usize = 65_536;
/// The largest picture a paste takes (bytes of PNG text).
const MAX_PASTE_BYTES: usize = 32 << 20;

pub(super) fn specs() -> Vec<CommandSpec> {
    vec![
        spec!(
            "pixel.select",
            "Select pixels",
            "{id, kind: rect|lasso|wand|all|none|invert, from?/to? [x, y] (rect), points? [[x, y], …] (lasso), x?/y? (wand), op? (replace|add|subtract|intersect), contiguous? (wand, default true), tolerance? (0–255), layer?, frame?} → {count}",
            true,
            select
        ),
        spec!(
            "pixel.transform",
            "Transform pixels",
            "{id, op: move|flipH|flipV|rotateCW|rotateCCW, dx?/dy? (move, art pixels), layer?, frame?}  changes the selected pixels (the whole cel when nothing is selected); the selection follows → {count}",
            true,
            transform_cmd
        ),
        spec!(
            "pixel.outline",
            "Outline pixels",
            "{id, color?, outside? (default true: round the art; false: on its edge), layer?, frame?}  within the selection when there is one → {changed}",
            true,
            outline
        ),
        spec!(
            "pixel.replaceColor",
            "Replace a pixel colour",
            "{id | ids (several pixel layers), from, to (colours), link? (style:<id> or var:<id> that `to` came from), tolerance? (0–255), everywhere? (default true: every layer and frame; false: the one cel, within the selection)} → {changed}",
            true,
            replace_color
        ),
        spec!(
            "pixel.copy",
            "Copy pixels",
            "{id, layer? (default what the frame shows), frame?}  the selected pixels (or the whole frame) → {png (base64), x, y, width, height}",
            false,
            copy
        ),
        spec!(
            "pixel.paste",
            "Paste pixels",
            "{id, png? (base64 PNG) | source? (id of any layer) | clipboard? (what Copy took: layers, vector or not), x?, y? (art pixels, default 0), threshold? (0–100 %, default 50: alpha under it is left out and the rest comes in solid, so edges stay sharp; 0 keeps the alpha as drawn), layer?, frame?}  a layer is drawn at the sprite's scale, made smaller if it wouldn't fit the board; the pasted pixels become the selection → {width, height}",
            true,
            paste
        ),
        spec!("pixel.layer.add", "New sprite layer", "{id, name?, index? (0 = bottom; default on top)} → {layer}", true, layer_add),
        spec!("pixel.layer.delete", "Delete sprite layer", "{id, layer}  (never the last one) → {layers}", true, layer_delete),
        spec!("pixel.layer.set", "Sprite layer settings", "{id, layer, name?, visible?, opacity? (0–100 %)} → {layer}", true, layer_set),
        spec!("pixel.layer.move", "Move sprite layer", "{id, layer, index (0 = bottom)} → {index}", true, layer_move),
        spec!("pixel.layer.merge", "Merge sprite layer down", "{id, layer}  into the layer below it, every frame → {layer}", true, layer_merge),
        spec!(
            "pixel.palette.set",
            "Sprite palette",
            "{id, colors? [colours] (replaces it), add? (a colour to add), remove? (a colour to take out), text? (a .gpl or .hex palette file's text)} → {palette}",
            true,
            palette_set
        ),
        spec!("pixel.palette.export", "Export sprite palette", "{id, format? (gpl|hex, default gpl)} → {text}", false, palette_export),
    ]
}

/// The sprite's selection, or None.
fn selection_of(s: &Session, id: NodeId, sprite: &Sprite) -> Result<Option<Mask>> {
    Ok(store::selection(&s.doc()?.get(id)?.plugin_data, sprite))
}

/// Stores `mask` as the sprite's selection (an empty one deselects).
fn put_selection(s: &mut Session, id: NodeId, mask: Option<&Mask>) -> Result<()> {
    store::put_selection(&mut s.doc_mut()?.get_mut(id)?.plugin_data, mask);
    Ok(())
}

/// A whole-number param within ±1,000,000 (default `d`).
fn whole(cmd: &str, p: &Value, key: &str, d: i64) -> Result<i64> {
    match get(p, key) {
        None | Some(Value::Null) => Ok(d),
        Some(v) => v
            .as_f64()
            .filter(|f| f.is_finite())
            .map(|f| f.clamp(-1e6, 1e6).round() as i64)
            .ok_or_else(|| bad(cmd, format!("`{key}` must be a number"))),
    }
}

fn tolerance(p: &Value) -> u8 {
    num(p, "tolerance").filter(|f| f.is_finite()).map_or(0, |f| f.clamp(0.0, 255.0) as u8)
}

fn select(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "pixel.select";
    let id = req_id(cmd, p, "id")?;
    let sprite = sprite_of(cmd, s.doc()?, id)?;
    let (w, h) = (sprite.width, sprite.height);
    let op = match get(p, "op").and_then(Value::as_str) {
        None => Op::Replace,
        Some(o) => Op::parse(o).ok_or_else(|| bad(cmd, "`op` must be replace, add, subtract or intersect"))?,
    };
    let current = selection_of(s, id, &sprite)?;
    let none = || Mask::new(w, h).ok_or_else(|| bad(cmd, "that sprite size can't be selected"));
    let kind = get(p, "kind").and_then(Value::as_str).unwrap_or("");
    let made = match kind {
        "none" => none()?,
        "all" => Mask::all(w, h).ok_or_else(|| bad(cmd, "that sprite size can't be selected"))?,
        "invert" => {
            current.as_ref().map_or_else(|| Mask::all(w, h), |m| Some(m.inverted())).ok_or_else(|| bad(cmd, "that sprite size can't be selected"))?
        }
        "rect" => {
            Mask::rect(w, h, req_point(cmd, p, "from")?, req_point(cmd, p, "to")?).ok_or_else(|| bad(cmd, "that sprite size can't be selected"))?
        }
        "lasso" => {
            let points: Vec<_> = match get(p, "points") {
                Some(Value::Array(a)) if !a.is_empty() => {
                    a.iter().take(MAX_LASSO_POINTS).map(point_of).collect::<Option<_>>().ok_or_else(|| bad(cmd, "`points` must be [[x, y], …]"))?
                }
                _ => return Err(bad(cmd, "`points` must be [[x, y], …] with at least one point")),
            };
            Mask::polygon(w, h, &points).ok_or_else(|| bad(cmd, "that sprite size can't be selected"))?
        }
        "wand" => {
            let coord = |k: &str| {
                num(p, k).filter(|f| f.is_finite()).map(|f| f.clamp(-1e6, 1e6).floor() as i64).ok_or_else(|| bad(cmd, "the wand needs `x` and `y`"))
            };
            let at = (coord("x")?, coord("y")?);
            let (layer, frame) = cel_of(cmd, &sprite, p)?;
            let cel = store::cel(&s.doc()?.get(id)?.plugin_data, &sprite, layer, frame).ok_or_else(|| bad(cmd, "that cel can't be read"))?;
            Mask::wand(&cel, at, boolean(p, "contiguous").unwrap_or(true), tolerance(p))
                .ok_or_else(|| bad(cmd, "that sprite size can't be selected"))?
        }
        _ => return Err(bad(cmd, "`kind` must be rect, lasso, wand, all, none or invert")),
    };
    // All, none and invert replace the selection whatever `op` says.
    let result = match (current, op) {
        (Some(mut m), Op::Add | Op::Subtract | Op::Intersect) if !matches!(kind, "all" | "none" | "invert") => {
            m.combine(&made, op);
            m
        }
        (None, Op::Subtract | Op::Intersect) if !matches!(kind, "all" | "none" | "invert") => none()?,
        _ => made,
    };
    put_selection(s, id, Some(&result))?;
    Ok(json!({"count": result.count()}))
}

fn transform_cmd(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "pixel.transform";
    let id = req_id(cmd, p, "id")?;
    let sprite = sprite_of(cmd, s.doc()?, id)?;
    let change = match get(p, "op").and_then(Value::as_str).unwrap_or("") {
        "move" => Change::Move { dx: whole(cmd, p, "dx", 0)?, dy: whole(cmd, p, "dy", 0)? },
        "flipH" => Change::Flip { horizontal: true },
        "flipV" => Change::Flip { horizontal: false },
        "rotateCW" => Change::Rotate { clockwise: true },
        "rotateCCW" => Change::Rotate { clockwise: false },
        _ => return Err(bad(cmd, "`op` must be move, flipH, flipV, rotateCW or rotateCCW")),
    };
    let (layer, frame) = cel_of(cmd, &sprite, p)?;
    let selected = selection_of(s, id, &sprite)?;
    let doc = s.doc_mut()?;
    let data = &doc.get(id)?.plugin_data;
    let mut cel = store::cel(data, &sprite, layer, frame).ok_or_else(|| bad(cmd, "that cel can't be read"))?;
    let Some(moved) = transform::apply(&mut cel, selected.as_ref(), change) else {
        return Ok(json!({"count": 0}));
    };
    let n = doc.get_mut(id)?;
    store::put_cel(&mut n.plugin_data, layer, frame, &cel);
    if selected.is_some() {
        store::put_selection(&mut n.plugin_data, Some(&moved));
    }
    show(doc, id, &sprite);
    Ok(json!({"count": moved.count()}))
}

fn outline(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "pixel.outline";
    let id = req_id(cmd, p, "id")?;
    let sprite = sprite_of(cmd, s.doc()?, id)?;
    let cel = cel_of(cmd, &sprite, p)?;
    let color = match get(p, "color") {
        None | Some(Value::Null) => [0, 0, 0, 255],
        Some(v) => rgba_of(v).ok_or_else(|| bad(cmd, "`color` must be a colour"))?,
    };
    let outside = boolean(p, "outside").unwrap_or(true);
    let changed = edit_cel(s, id, &sprite, cel, |c| transform::outline(c, None, color, outside))?;
    Ok(json!({"changed": changed}))
}

fn replace_color(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "pixel.replaceColor";
    let colour = |key: &str| get(p, key).and_then(rgba_of).ok_or_else(|| bad(cmd, format!("`{key}` must be a colour")));
    let (from, to) = (colour("from")?, colour("to")?);
    let tol = tolerance(p);
    // The colour style or variable `to` came from (G16.6): it replaces `from`'s link.
    let link = super::tools::link_param(cmd, s.doc()?, p)?;
    // Several pixel layers at once (Selected colors): `ids`, every layer and frame of each.
    if let Some(list) = get(p, "ids").filter(|v| !v.is_null()) {
        let ids: Vec<NodeId> = list
            .as_array()
            .ok_or_else(|| bad(cmd, "`ids` must be a list of layer ids"))?
            .iter()
            .take(1024)
            .map(|v| v.as_u64().map(NodeId))
            .collect::<Option<_>>()
            .ok_or_else(|| bad(cmd, "`ids` must be a list of layer ids"))?;
        let mut changed = 0;
        for id in ids {
            let sprite = sprite_of(cmd, s.doc()?, id)?;
            changed += replace_everywhere(s, id, &sprite, from, to, tol)?;
            super::tools::relink(s.doc_mut()?, id, from, to, link.as_deref());
        }
        return Ok(json!({"changed": changed}));
    }
    let id = req_id(cmd, p, "id")?;
    let sprite = sprite_of(cmd, s.doc()?, id)?;
    if !boolean(p, "everywhere").unwrap_or(true) {
        let cel = cel_of(cmd, &sprite, p)?;
        let changed = edit_cel(s, id, &sprite, cel, |c| transform::replace_color(c, None, from, to, tol))?;
        return Ok(json!({"changed": changed}));
    }
    let changed = replace_everywhere(s, id, &sprite, from, to, tol)?;
    super::tools::relink(s.doc_mut()?, id, from, to, link.as_deref());
    Ok(json!({"changed": changed}))
}

/// Replaces `from` with `to` in every cel of pixel layer `id` (Selection colours does this).
/// How many pixels changed.
pub fn replace_everywhere(s: &mut Session, id: NodeId, sprite: &Sprite, from: Rgba, to: Rgba, tol: u8) -> Result<usize> {
    let doc = s.doc_mut()?;
    let mut changed = 0;
    for l in &sprite.layers {
        for f in &sprite.frames {
            let data = &doc.get(id)?.plugin_data;
            if !data.contains_key(&store::cel_key(l.id, f.id)) {
                continue;
            }
            let Some(mut cel) = store::cel(data, sprite, l.id, f.id) else { continue };
            let n = transform::replace_color(&mut cel, None, from, to, tol);
            if n > 0 {
                store::put_cel(&mut doc.get_mut(id)?.plugin_data, l.id, f.id, &cel);
                changed += n;
            }
        }
    }
    if changed > 0 {
        show(doc, id, sprite);
    }
    Ok(changed)
}

/// The picture copying reads: one layer's cel (`layer` given) or what the frame shows.
fn source_picture(cmd: &str, s: &Session, id: NodeId, sprite: &Sprite, p: &Value) -> Result<Canvas> {
    let data = &s.doc()?.get(id)?.plugin_data;
    match get(p, "layer") {
        None | Some(Value::Null) => {
            let frame = match get(p, "frame") {
                None | Some(Value::Null) => 0,
                Some(v) => v
                    .as_u64()
                    .and_then(|i| usize::try_from(i).ok())
                    .filter(|i| *i < sprite.frames.len())
                    .ok_or_else(|| bad(cmd, "`frame` must be a frame index"))?,
            };
            store::frame_image(data, sprite, frame).ok_or_else(|| bad(cmd, "that frame can't be read"))
        }
        Some(_) => {
            let (layer, frame) = cel_of(cmd, sprite, p)?;
            store::cel(data, sprite, layer, frame).ok_or_else(|| bad(cmd, "that cel can't be read"))
        }
    }
}

fn copy(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "pixel.copy";
    let id = req_id(cmd, p, "id")?;
    let sprite = sprite_of(cmd, s.doc()?, id)?;
    let picture = source_picture(cmd, s, id, &sprite, p)?;
    let selected = selection_of(s, id, &sprite)?;
    let (x, y) = selected.as_ref().and_then(Mask::bounds).map_or((0, 0), |b| (b.0, b.1));
    let piece = transform::copy(&picture, selected.as_ref()).ok_or_else(|| bad(cmd, "nothing to copy"))?;
    let png = piece.to_png().ok_or_else(|| bad(cmd, "the picture can't be written"))?;
    Ok(json!({
        "png": base64::engine::general_purpose::STANDARD.encode(png),
        "x": x,
        "y": y,
        "width": piece.width(),
        "height": piece.height(),
    }))
}

fn paste(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "pixel.paste";
    let id = req_id(cmd, p, "id")?;
    let sprite = sprite_of(cmd, s.doc()?, id)?;
    let cut = threshold_of(cmd, p)?;
    let picture = match (get(p, "png").and_then(Value::as_str), opt_id(cmd, p, "source")?) {
        (Some(text), _) => {
            if text.len() > MAX_PASTE_BYTES {
                return Err(bad(cmd, "that picture is too big to paste"));
            }
            let bytes = base64::engine::general_purpose::STANDARD.decode(text.trim()).map_err(|_| bad(cmd, "`png` must be a base64 PNG"))?;
            Canvas::from_png(&bytes).ok_or_else(|| bad(cmd, "`png` isn't a picture a sprite can take (PNG, at most 1024 a side)"))?
        }
        (None, Some(source)) => {
            let doc = s.doc()?;
            drawn_picture(doc, &[source], &sprite, cut).ok_or_else(|| bad(cmd, "that layer can't be drawn as pixels"))?
        }
        (None, None) if boolean(p, "clipboard").unwrap_or(false) => clipboard_picture(s, &sprite, cut)?,
        (None, None) => return Err(bad(cmd, "give `png`, `source` or `clipboard`")),
    };
    let (x, y) = (whole(cmd, p, "x", 0)?, whole(cmd, p, "y", 0)?);
    let (layer, frame) = cel_of(cmd, &sprite, p)?;
    let doc = s.doc_mut()?;
    let mut cel = store::cel(&doc.get(id)?.plugin_data, &sprite, layer, frame).ok_or_else(|| bad(cmd, "that cel can't be read"))?;
    let covered = transform::paste(&mut cel, &picture, x, y).ok_or_else(|| bad(cmd, "that cel can't be pasted on"))?;
    let n = doc.get_mut(id)?;
    store::put_cel(&mut n.plugin_data, layer, frame, &cel);
    store::put_selection(&mut n.plugin_data, Some(&covered));
    show(doc, id, &sprite);
    Ok(json!({"width": picture.width(), "height": picture.height()}))
}

/// The `threshold` param as an alpha 0–255: a pixel at least this opaque comes in solid, a fainter
/// one not at all. 0 keeps the alpha as it was drawn.
fn threshold_of(cmd: &str, p: &Value) -> Result<u8> {
    match get(p, "threshold").filter(|v| !v.is_null()) {
        None => Ok(128),
        Some(v) => {
            let pct = v.as_f64().filter(|f| f.is_finite() && *f >= 0.0 && *f <= 100.0).ok_or_else(|| bad(cmd, "`threshold` must be 0–100"))?;
            Ok((pct * 2.55).round().clamp(0.0, 255.0) as u8)
        }
    }
}

/// Layers `ids` drawn as the pixels of `sprite` (G16.8): one art pixel per `scale` page units, so a
/// drawing comes in at its own size in art pixels, made smaller (keeping its shape) when that
/// wouldn't fit the board. With `cut` above 0 the alpha is cut at it instead of feathered: a pixel
/// that opaque comes in solid and a fainter one not at all, so the edges stay sharp pixel art.
fn drawn_picture(doc: &figcraft_doc::Document, ids: &[NodeId], sprite: &Sprite, cut: u8) -> Option<Canvas> {
    let b = ids.iter().filter_map(|id| figcraft_render::export_bounds(doc, *id)).reduce(|a, b| a.union(b))?;
    if !(b.width().is_finite() && b.height().is_finite()) || b.width() <= 0.0 || b.height() <= 0.0 {
        return None;
    }
    let mut k = 1.0 / f64::from(sprite.scale.max(1));
    // Fitted to the board: a drawing wider or taller than the sprite comes in at the size that fits.
    let room = (f64::from(sprite.width) / (b.width() * k)).min(f64::from(sprite.height) / (b.height() * k));
    if room.is_finite() && room < 1.0 {
        k *= room;
    }
    // A sprite holds 1024 art pixels a side at most, whatever the board says.
    let side = f64::from(Canvas::MAX_SIDE);
    let widest = (b.width().max(b.height()) * k).max(1.0);
    if widest > side {
        k *= side / widest;
    }
    let pm = figcraft_render::render_area(doc, ids, b, k)?;
    let mut px: Vec<Rgba> = pm
        .pixels()
        .iter()
        .map(|c| {
            let c = c.demultiply();
            match cut {
                0 => [c.red(), c.green(), c.blue(), c.alpha()],
                _ if c.alpha() >= cut => [c.red(), c.green(), c.blue(), 255],
                _ => [0, 0, 0, 0],
            }
        })
        .collect();
    if cut > 0 {
        snap_near_colours(&mut px);
    }
    Canvas::from_pixels(pm.width(), pm.height(), px)
}

/// Rounding where the drawing was anti-aliased leaves an edge pixel a shade off its own colour
/// (`#1f88e4` beside `#1e88e5`), which would read as four colours in Selected colors (G16.8). Each
/// pixel takes the most used colour within two steps a channel of it, so one flat colour stays one
/// colour; a real step between two shades is further apart and is left alone, and a drawing of many
/// colours (a photo, a gradient) is not touched at all.
fn snap_near_colours(px: &mut [Rgba]) {
    let mut count: std::collections::BTreeMap<Rgba, usize> = std::collections::BTreeMap::new();
    for c in px.iter().filter(|c| c[3] > 0) {
        *count.entry(*c).or_default() += 1;
        if count.len() > 256 {
            return;
        }
    }
    let near = |a: Rgba, b: Rgba| (0..4).all(|i| a[i].abs_diff(b[i]) <= 2);
    let mut to: std::collections::BTreeMap<Rgba, Rgba> = std::collections::BTreeMap::new();
    for (c, n) in &count {
        // The most used colour nearby (itself when nothing nearby is used more).
        let best = count.iter().filter(|(o, _)| near(*c, **o)).max_by_key(|(o, m)| (**m, std::cmp::Reverse(**o)));
        match best {
            Some((o, m)) if *m > *n => to.insert(*c, *o),
            _ => to.insert(*c, *c),
        };
    }
    for c in px.iter_mut() {
        if let Some(o) = to.get(c) {
            *c = *o;
        }
    }
}

/// What Copy took, drawn as pixels (G16.8): the copied layers are put on the page, drawn, and taken
/// straight back out, so vector work copied anywhere in the file can be pasted into a sprite and
/// then edited pixel by pixel. One undo step, like any other paste.
fn clipboard_picture(s: &mut Session, sprite: &Sprite, cut: u8) -> Result<Canvas> {
    let cmd = "pixel.paste";
    if s.clipboard.nodes.is_empty() {
        return Err(bad(cmd, "nothing was copied to paste"));
    }
    let page = s.page()?;
    let ids = crate::cmd::edit::paste_into(s, page, None)?;
    let picture = drawn_picture(s.doc()?, &ids, sprite, cut);
    let doc = s.doc_mut()?;
    for id in &ids {
        let _ = doc.remove(*id);
    }
    picture.ok_or_else(|| bad(cmd, "what was copied can't be drawn as pixels"))
}

/// The sprite layer `layer` names (an id of this sprite).
fn layer_id(cmd: &str, sprite: &Sprite, p: &Value) -> Result<u32> {
    get(p, "layer")
        .and_then(Value::as_u64)
        .and_then(|i| u32::try_from(i).ok())
        .filter(|i| sprite.layer_index(*i).is_some())
        .ok_or_else(|| bad(cmd, "`layer` must be a layer id of this sprite"))
}

/// Saves `sprite` in layer `id` and shows its picture again.
fn save(s: &mut Session, id: NodeId, sprite: &Sprite) -> Result<()> {
    let doc = s.doc_mut()?;
    store::save(&mut doc.get_mut(id)?.plugin_data, sprite);
    show(doc, id, sprite);
    Ok(())
}

fn layer_add(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "pixel.layer.add";
    let id = req_id(cmd, p, "id")?;
    let mut sprite = sprite_of(cmd, s.doc()?, id)?;
    let index = whole(cmd, p, "index", i64::MAX / 2)?.max(0) as usize;
    let name = get(p, "name").and_then(Value::as_str);
    let layer =
        sprite.add_layer(name, index).ok_or_else(|| bad(cmd, format!("a sprite has at most {} layers", figcraft_pixel::sprite::MAX_LAYERS)))?;
    save(s, id, &sprite)?;
    Ok(json!({"layer": layer}))
}

fn layer_delete(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "pixel.layer.delete";
    let id = req_id(cmd, p, "id")?;
    let mut sprite = sprite_of(cmd, s.doc()?, id)?;
    let layer = layer_id(cmd, &sprite, p)?;
    if !sprite.remove_layer(layer) {
        return Err(bad(cmd, "a sprite keeps at least one layer"));
    }
    save(s, id, &sprite)?;
    Ok(json!({"layers": sprite.layers.len()}))
}

fn layer_set(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "pixel.layer.set";
    let id = req_id(cmd, p, "id")?;
    let mut sprite = sprite_of(cmd, s.doc()?, id)?;
    let layer = layer_id(cmd, &sprite, p)?;
    let opacity = match num(p, "opacity") {
        None => None,
        Some(o) if o.is_finite() => Some((o.clamp(0.0, 100.0) * 2.55).round() as u8),
        Some(_) => return Err(bad(cmd, "`opacity` must be 0–100")),
    };
    let Some(l) = sprite.layers.iter_mut().find(|l| l.id == layer) else { return Err(bad(cmd, "no such layer")) };
    if let Some(name) = get(p, "name").and_then(Value::as_str) {
        l.name = name.chars().take(200).collect();
    }
    if let Some(v) = boolean(p, "visible") {
        l.visible = v;
    }
    if let Some(o) = opacity {
        l.opacity = o;
    }
    save(s, id, &sprite)?;
    Ok(json!({"layer": layer}))
}

fn layer_move(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "pixel.layer.move";
    let id = req_id(cmd, p, "id")?;
    let mut sprite = sprite_of(cmd, s.doc()?, id)?;
    let layer = layer_id(cmd, &sprite, p)?;
    let index = whole(cmd, p, "index", -1)?;
    if index < 0 {
        return Err(bad(cmd, "`index` must be 0 or more"));
    }
    sprite.move_layer(layer, index as usize);
    save(s, id, &sprite)?;
    Ok(json!({"index": sprite.layer_index(layer)}))
}

fn layer_merge(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "pixel.layer.merge";
    let id = req_id(cmd, p, "id")?;
    let mut sprite = sprite_of(cmd, s.doc()?, id)?;
    let layer = layer_id(cmd, &sprite, p)?;
    let index = sprite.layer_index(layer).ok_or_else(|| bad(cmd, "no such layer"))?;
    let below = index.checked_sub(1).and_then(|i| sprite.layers.get(i)).cloned().ok_or_else(|| bad(cmd, "the bottom layer has nothing below it"))?;
    let top = sprite.layers.get(index).cloned().ok_or_else(|| bad(cmd, "no such layer"))?;
    let doc = s.doc_mut()?;
    for f in &sprite.frames {
        let data = &doc.get(id)?.plugin_data;
        let (Some(mut under), Some(over)) = (store::cel(data, &sprite, below.id, f.id), store::cel(data, &sprite, top.id, f.id)) else { continue };
        if top.visible {
            under.draw_over(&over, top.opacity);
        }
        store::put_cel(&mut doc.get_mut(id)?.plugin_data, below.id, f.id, &under);
    }
    sprite.remove_layer(layer);
    save(s, id, &sprite)?;
    Ok(json!({"layer": below.id}))
}

fn palette_set(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "pixel.palette.set";
    let id = req_id(cmd, p, "id")?;
    let mut sprite = sprite_of(cmd, s.doc()?, id)?;
    let max = figcraft_pixel::sprite::MAX_PALETTE;
    if let Some(v) = get(p, "colors").filter(|v| !v.is_null()) {
        let a = v.as_array().ok_or_else(|| bad(cmd, "`colors` must be a list of colours"))?;
        sprite.palette = a
            .iter()
            .take(max)
            .map(|c| rgba_of(c).map(canvas::to_hex))
            .collect::<Option<_>>()
            .ok_or_else(|| bad(cmd, "`colors` must be a list of colours"))?;
    }
    if let Some(text) = get(p, "text").and_then(Value::as_str) {
        sprite.palette = palette::parse(text).into_iter().map(canvas::to_hex).collect();
    }
    if let Some(v) = get(p, "add").filter(|v| !v.is_null()) {
        let c = canvas::to_hex(rgba_of(v).ok_or_else(|| bad(cmd, "`add` must be a colour"))?);
        if !sprite.palette.contains(&c) && sprite.palette.len() < max {
            sprite.palette.push(c);
        }
    }
    if let Some(v) = get(p, "remove").filter(|v| !v.is_null()) {
        let c = canvas::to_hex(rgba_of(v).ok_or_else(|| bad(cmd, "`remove` must be a colour"))?);
        sprite.palette.retain(|x| *x != c);
    }
    save(s, id, &sprite)?;
    Ok(json!({"palette": sprite.palette}))
}

fn palette_export(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "pixel.palette.export";
    let id = req_id(cmd, p, "id")?;
    let sprite = sprite_of(cmd, s.doc()?, id)?;
    let mut colors: Vec<Rgba> = sprite.palette.iter().filter_map(|c| canvas::parse_hex(c)).collect();
    if colors.is_empty() {
        // No palette of its own (colours come from the file's styles, G16.6): the colours it is
        // drawn with, in the order they first appear.
        colors = used_colors(s.doc()?.get(id)?, &sprite);
    }
    let name = s.doc()?.get(id)?.name.clone();
    let text = match get(p, "format").and_then(Value::as_str).unwrap_or("gpl") {
        "gpl" => palette::to_gpl(&name, &colors),
        "hex" => palette::to_hex_list(&colors),
        _ => return Err(bad(cmd, "`format` must be gpl or hex")),
    };
    Ok(json!({"text": text}))
}

/// The distinct colours of every cel of `n`'s sprite, in the order they first appear (at most 256).
fn used_colors(n: &figcraft_doc::Node, sprite: &Sprite) -> Vec<Rgba> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for l in &sprite.layers {
        for f in &sprite.frames {
            if !n.plugin_data.contains_key(&store::cel_key(l.id, f.id)) {
                continue;
            }
            let Some(cel) = store::cel(&n.plugin_data, sprite, l.id, f.id) else { continue };
            for p in cel.pixels().iter().filter(|p| p[3] > 0) {
                if out.len() >= 256 {
                    return out;
                }
                if seen.insert(*p) {
                    out.push(*p);
                }
            }
        }
    }
    out
}
