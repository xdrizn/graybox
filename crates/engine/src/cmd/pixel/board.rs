//! Graybox pixel art (G16.7): a pixel layer is a board, like a frame. Its size is its drawing area
//! in art pixels: making it bigger or smaller (the W and H fields, or dragging its sides) adds or
//! cuts art pixels on the side moved and keeps the art where it is on the page. The scale stays.
//! A 9-sliced layer (a button's background) and an instance keep regrowing their art instead.

use figcraft_doc::nine_slice::NineSlice;
use figcraft_doc::{Document, NodeId, NodeKind};
use figcraft_geom::Affine;
use figcraft_pixel::{Canvas, Sprite, store};
use serde_json::{Value, json};

use super::{show, sprite_of};
use crate::automation::params::{get, req_id};
use crate::cmd::CommandSpec;
use crate::{Result, Session, bad, spec};

pub(super) fn specs() -> Vec<CommandSpec> {
    vec![spec!(
        "pixel.canvas",
        "Sprite canvas size",
        "{id, width?, height? (art pixels, 1–1024; default as it is), anchor? (tl|t|tr|l|c|r|bl|b|br: where the art stays, default tl), offset? ([dx, dy]: or how far the art moves inside, art pixels)}  makes the drawing area bigger or smaller, every layer and frame; the art keeps its place on the page and the scale stays → {width, height}",
        true,
        canvas_cmd
    )]
}

/// True when layer `id` is a pixel board: a pixel layer of its own (not an instance or inside one)
/// without 9-slice. Its size is its drawing area.
pub fn is_board(doc: &Document, id: NodeId) -> bool {
    let Some(n) = doc.node(id) else { return false };
    if !store::is_pixel(&n.plugin_data) || NineSlice::of(n).is_some() {
        return false;
    }
    let mut at = Some(n);
    while let Some(x) = at {
        if matches!(x.kind, NodeKind::Instance { .. }) {
            return false;
        }
        at = x.parent.and_then(|p| doc.node(p));
    }
    true
}

/// Where the art goes in a `w` × `h` area resized from `old`, by `anchor` (art pixels, x then y).
fn anchored(anchor: &str, old: (u32, u32), (w, h): (u32, u32)) -> Option<(i64, i64)> {
    let (dw, dh) = (i64::from(w) - i64::from(old.0), i64::from(h) - i64::from(old.1));
    let (ax, ay) = match anchor {
        "tl" => (0, 0),
        "t" => (1, 0),
        "tr" => (2, 0),
        "l" => (0, 1),
        "c" => (1, 1),
        "r" => (2, 1),
        "bl" => (0, 2),
        "b" => (1, 2),
        "br" => (2, 2),
        _ => return None,
    };
    let part = |d: i64, a: i64| match a {
        0 => 0,
        1 => d.div_euclid(2),
        _ => d,
    };
    Some((part(dw, ax), part(dh, ay)))
}

/// Fits layer `id` round the pixels it uses (G16.7): outside Pixel mode it shows only the box
/// round them (the whole board when it is empty), where they are on the page. A layer at the size
/// of the box it showed takes the new box's size and place; one resized in Design keeps its own
/// (its art regrows into it). A 9-sliced layer's base follows the box.
pub fn retrim(doc: &mut Document, id: NodeId, sprite: &Sprite) {
    let Some(n) = doc.node(id) else { return };
    let old = store::shown_box(&n.plugin_data, sprite);
    let used = store::used_box(&n.plugin_data, sprite);
    let new = used.unwrap_or([0, 0, sprite.width, sprite.height]);
    let k = f64::from(sprite.scale.max(1));
    let px = |v: u32| f64::from(v) * k;
    let natural = (n.width - px(old[2])).abs() < 0.01 && (n.height - px(old[3])).abs() < 0.01;
    let world = doc.world_transform(id);
    let Some(n) = doc.node_mut(id) else { return };
    store::put_trim(&mut n.plugin_data, used.filter(|u| *u != [0, 0, sprite.width, sprite.height]));
    if new == old || !natural {
        return;
    }
    if let Some(mut s) = NineSlice::of(n) {
        s.base = (px(new[2]), px(new[3]));
        n.plugin_data.insert(figcraft_doc::nine_slice::KEY.into(), s.clamped().encode());
    }
    let shift = (px(new[0]) - px(old[0]), px(new[1]) - px(old[1]));
    if doc.set_world_transform(id, world * Affine::translate(shift)).is_ok() {
        figcraft_layout::set_size(doc, id, px(new[2]), px(new[3]));
    }
}

/// Resizes board `id` to `w` × `h` art pixels, its art moved by `(dx, dy)` art pixels inside it.
/// Every cel of every layer and frame is redrawn; the layer stays round its art on the page (its
/// shown box moves with the art). The selection is dropped.
pub fn resize(doc: &mut Document, id: NodeId, (w, h): (u32, u32), (dx, dy): (i64, i64)) -> Result<()> {
    let cmd = "pixel.canvas";
    let n = doc.get(id)?;
    let mut sprite: Sprite = store::load(&n.plugin_data).ok_or_else(|| bad(cmd, "the sprite can't be read"))?;
    if Canvas::new(w, h).is_none() {
        return Err(bad(cmd, format!("a sprite is 1–{} art pixels a side", Canvas::MAX_SIDE)));
    }
    let [tx, ty, tw, th] = store::shown_box(&n.plugin_data, &sprite);
    let k = f64::from(sprite.scale.max(1));
    let natural = (n.width - f64::from(tw) * k).abs() < 0.01 && (n.height - f64::from(th) * k).abs() < 0.01;
    // The shown box moves with the art, cut to the new board (or the whole board when nothing of
    // it is left).
    let (x0, y0) = (i64::from(tx) + dx, i64::from(ty) + dy);
    let cut = |a: i64, len: u32, max: u32| (a.clamp(0, i64::from(max)), (a + i64::from(len)).clamp(0, i64::from(max)));
    let ((left, right), (top, bottom)) = (cut(x0, tw, w), cut(y0, th, h));
    let shown = if right > left && bottom > top { [left, top, right - left, bottom - top] } else { [0, 0, i64::from(w), i64::from(h)] };
    let mut cels = Vec::new();
    for l in &sprite.layers {
        for f in &sprite.frames {
            let Some(cel) = store::cel(&n.plugin_data, &sprite, l.id, f.id).and_then(|old| old.resized(w, h, dx, dy)) else { continue };
            cels.push((l.id, f.id, cel));
        }
    }
    sprite.width = w;
    sprite.height = h;
    let world = doc.world_transform(id);
    let node = doc.get_mut(id)?;
    store::save(&mut node.plugin_data, &sprite);
    for (l, f, cel) in &cels {
        store::put_cel(&mut node.plugin_data, *l, *f, cel);
    }
    store::put_selection(&mut node.plugin_data, None);
    let as_u32 = |v: i64| u32::try_from(v).unwrap_or(0);
    store::put_trim(&mut node.plugin_data, Some(shown.map(as_u32)));
    if natural {
        // The layer keeps showing the same art pixels where they were on the page.
        let shift = ((shown[0] - x0) as f64 * k, (shown[1] - y0) as f64 * k);
        doc.set_world_transform(id, world * Affine::translate(shift))?;
        figcraft_layout::set_size(doc, id, shown[2] as f64 * k, shown[3] as f64 * k);
    }
    show(doc, id, &sprite);
    Ok(())
}

fn canvas_cmd(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "pixel.canvas";
    let id = req_id(cmd, p, "id")?;
    let sprite = sprite_of(cmd, s.doc()?, id)?;
    let side = |key: &str, now: u32| -> Result<u32> {
        match get(p, key) {
            None | Some(Value::Null) => Ok(now),
            Some(v) => v
                .as_f64()
                .filter(|f| f.is_finite() && *f >= 1.0 && *f <= f64::from(Canvas::MAX_SIDE))
                .map(|f| f.round() as u32)
                .ok_or_else(|| bad(cmd, format!("`{key}` must be 1–{} art pixels", Canvas::MAX_SIDE))),
        }
    };
    let (w, h) = (side("width", sprite.width)?, side("height", sprite.height)?);
    let max = i64::from(Canvas::MAX_SIDE);
    let at = match get(p, "offset") {
        None | Some(Value::Null) => {
            let anchor = match get(p, "anchor") {
                None | Some(Value::Null) => "tl",
                Some(v) => v.as_str().unwrap_or_default(),
            };
            anchored(anchor, (sprite.width, sprite.height), (w, h)).ok_or_else(|| bad(cmd, "`anchor` must be tl, t, tr, l, c, r, bl, b or br"))?
        }
        Some(v) => {
            let part = |i: usize| v.get(i).and_then(Value::as_f64).filter(|f| f.is_finite()).map(|f| (f.round() as i64).clamp(-max, max));
            match (v.as_array().map(Vec::len), part(0), part(1)) {
                (Some(2), Some(x), Some(y)) => (x, y),
                _ => return Err(bad(cmd, "`offset` must be [dx, dy] (art pixels)")),
            }
        }
    };
    resize(s.doc_mut()?, id, (w, h), at)?;
    Ok(json!({"width": w, "height": h}))
}
