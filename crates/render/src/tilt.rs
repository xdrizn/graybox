//! Graybox: a tilted layer (3D rotation, G17; see [`figcraft_doc::tilt`]) is drawn flat into a
//! pixmap of its own, then warped onto the target: each device pixel it covers maps back through
//! the inverse homography and samples the flat drawing (bilinear, premultiplied). tiny-skia has only
//! affine transforms, hence the warp by hand.
//!
//! The flat drawing is kept ([`CACHE`]) while the layer and every layer inside it are unchanged
//! (the same `Arc`s), at the same scale: moving or panning a tilted frame only warps it again.

use std::sync::{Arc, Mutex, PoisonError};

use figcraft_doc::tilt::{self, Homography};
use figcraft_doc::{Document, Node};
use figcraft_geom::{Affine, Point, Rect};
use tiny_skia::{Pixmap, PixmapPaint, Transform};

use crate::draw::{Bounds, Ctx};

/// The largest side of the flat drawing (device pixels).
const MAX_SIDE: f64 = 4096.0;
/// Flat drawings kept, most recently used last.
const MAX_ENTRIES: usize = 8;
/// Pixels kept in all (4 bytes each): 64 MB.
const MAX_PIXELS: u64 = 16 * 1024 * 1024;

/// What a kept flat drawing was drawn from.
struct Key {
    /// The layer and the layers inside it, top first (held: never confused with others).
    nodes: Vec<Arc<Node>>,
    bits: [u64; 5],
    size: (u32, u32),
    flags: (bool, bool, bool, bool),
    fonts: u64,
}

impl Key {
    fn matches(&self, o: &Key) -> bool {
        self.bits == o.bits
            && self.size == o.size
            && self.flags == o.flags
            && self.fonts == o.fonts
            && self.nodes.len() == o.nodes.len()
            && self.nodes.iter().zip(&o.nodes).all(|(a, b)| Arc::ptr_eq(a, b))
    }
}

static CACHE: Mutex<Vec<(Key, Arc<Pixmap>)>> = Mutex::new(Vec::new());

/// The local paint bounds `r` of `n` grown to what its tilt draws ([`crate::draw::bounds_within`]).
pub(crate) fn grow(n: &Node, r: Rect) -> Rect {
    tilt::local(n).and_then(|h| h.bounds(r)).unwrap_or(r)
}

/// Page bounds `r` of layer `id` grown to its outermost tilted ancestor's paint bounds: a change
/// inside a tilted layer shows wherever the warp puts it ([`crate::draw::paint_bounds`]).
pub(crate) fn inside_tilted(doc: &Document, id: figcraft_doc::NodeId, r: Rect) -> Rect {
    let outer = doc.ancestors(id).into_iter().filter(|a| doc.node(*a).is_some_and(|n| tilt::of(n).is_some())).last();
    match outer.and_then(|a| crate::draw::paint_bounds(doc, a)) {
        Some(b) => r.union(b),
        None => r,
    }
}

/// Draw `n` (at `xf`, its own transform included) tilted, if it is. False when it isn't: the
/// caller draws it as usual.
pub(crate) fn draw(ctx: &Ctx, pix: &mut Pixmap, n: &Node, xf: Affine, depth: usize) -> bool {
    let Some(h) = tilt::local(n) else { return false };
    let mut flat = n.clone();
    flat.plugin_data.remove(tilt::KEY);
    // Drawn in its own space: `xf` already places it.
    flat.transform = Affine::IDENTITY;
    let to_page = if ctx.scale.is_finite() && ctx.scale > 0.0 { Affine::scale(1.0 / ctx.scale) * xf } else { xf };
    // What the layer paints laid flat (its effects and overflow included), in its own units.
    let r = crate::draw::bounds_within(ctx.doc, &flat, to_page, 0, ctx.lod.then_some(ctx.scale), None).0;
    if !(r.width() > 0.0 && r.height() > 0.0 && r.x0.is_finite() && r.y0.is_finite() && r.x1.is_finite() && r.y1.is_finite()) {
        return true;
    }
    let to_device = Homography::from_affine(xf) * h;
    let Some(quad) = to_device.bounds(r) else { return true };
    let target = Rect::new(0.0, 0.0, f64::from(pix.width()), f64::from(pix.height()));
    let shown = quad.intersect(target);
    if shown.width() <= 0.0 || shown.height() <= 0.0 {
        return true;
    }
    // Device pixels per layer unit, times how much the near side is magnified.
    let [a, b, c, d, ..] = xf.as_coeffs();
    let base = (a * a + b * b).sqrt().max((c * c + d * d).sqrt());
    let near = [Point::new(r.x0, r.y0), Point::new(r.x1, r.y0), Point::new(r.x1, r.y1), Point::new(r.x0, r.y1)]
        .iter()
        .map(|p| 1.0 / h.apply_h(*p).2.max(tilt::NEAR_W))
        .fold(1.0, f64::max);
    let k = (base * near).min(MAX_SIDE / r.width().max(r.height()));
    if !(k.is_finite() && k > 0.0) {
        return true;
    }
    let (fw, fh) = ((r.width() * k).ceil().max(1.0) as u32, (r.height() * k).ceil().max(1.0) as u32);
    // The flat drawing is this much finer than the target (device pixels per page unit scale alike).
    let finer = if base > 1e-12 { k / base } else { 1.0 };
    let Some(off) = flat_drawing(ctx, n, &flat, r, (k, finer), (fw, fh), depth) else { return true };
    // Device pixel → flat drawing pixel.
    let to_flat = Homography::from_affine(Affine::scale(k) * Affine::translate((-r.x0, -r.y0)));
    let Some(back) = to_device.inverse().map(|inv| to_flat * inv) else { return true };
    let (x0, y0) = (shown.x0.floor().max(0.0) as u32, shown.y0.floor().max(0.0) as u32);
    let (x1, y1) = (shown.x1.ceil().min(target.x1) as u32, shown.y1.ceil().min(target.y1) as u32);
    let (w, hgt) = (x1.saturating_sub(x0), y1.saturating_sub(y0));
    let Some(_held) = crate::layer_budget::Held::take(w, hgt, 1) else { return true };
    let Some(mut out) = Pixmap::new(w, hgt) else { return true };
    warp(&off, &back, (x0, y0), &mut out);
    pix.draw_pixmap(x0 as i32, y0 as i32, out.as_ref(), &PixmapPaint::default(), Transform::identity(), None);
    true
}

/// `flat` (tilted layer `n` laid flat) drawn at `k` device pixels per unit over its local bounds
/// `r`, `finer` times as fine as the target: a kept drawing when nothing in it changed.
fn flat_drawing(ctx: &Ctx, n: &Node, flat: &Node, r: Rect, (k, finer): (f64, f64), size: (u32, u32), depth: usize) -> Option<Arc<Pixmap>> {
    // Keyed by the document's own nodes. The layer drawn may be a copy (a 9-slice base): one that
    // isn't the document's node is drawn afresh every time, and so is one with a layer inside left
    // out or outlined (text being edited, outlines for the selection).
    let own = ctx.doc.node_arc(n.id).filter(|o| std::ptr::eq(Arc::as_ptr(*o), n));
    let key = own.and_then(|root| {
        let ids = ctx.doc.subtree(flat.id);
        if ids.iter().any(|id| ctx.skip.contains(id) || ctx.outlined.contains(id)) {
            return None;
        }
        let mut nodes = vec![root.clone()];
        nodes.extend(ids.into_iter().skip(1).filter_map(|id| ctx.doc.node_arc(id).cloned()));
        // Animated GIFs and pixel layers inside: the frames they show, not the time (which the
        // presenter moves on every frame).
        let frames = if ctx.time == 0.0 { 0 } else { crate::images::animation_clock(ctx.doc, flat.id, ctx.time).map_or(0, |c| c.0) };
        Some(Key {
            nodes,
            bits: [k.to_bits(), r.x0.to_bits(), r.y0.to_bits(), ctx.scale.to_bits(), frames],
            size,
            flags: (ctx.lod, ctx.preview, ctx.magnified_image_pixels, ctx.outline_mode),
            fonts: figcraft_text::layout_generation(),
        })
    });
    if let Some(key) = &key {
        let mut cache = CACHE.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(i) = cache.iter().position(|(k, _)| k.matches(key)) {
            let hit = cache.remove(i);
            let pix = hit.1.clone();
            cache.push(hit);
            return Some(pix);
        }
    }
    let _held = crate::layer_budget::Held::take(size.0, size.1, 1)?;
    let mut off = Pixmap::new(size.0, size.1)?;
    {
        let bounds = Bounds::default();
        let sub = Ctx { bounds: &bounds, under: None, scale: ctx.scale * finer, ..*ctx };
        let xf = Affine::scale(k) * Affine::translate((-r.x0, -r.y0));
        crate::draw::draw_layer(&sub, &mut off, flat, xf, depth + 1);
    }
    let off = Arc::new(off);
    if let Some(key) = key {
        let mut cache = CACHE.lock().unwrap_or_else(PoisonError::into_inner);
        cache.push((key, off.clone()));
        let pixels = |c: &Vec<(Key, Arc<Pixmap>)>| c.iter().map(|(k, _)| u64::from(k.size.0) * u64::from(k.size.1)).sum::<u64>();
        while cache.len() > MAX_ENTRIES || (cache.len() > 1 && pixels(&cache) > MAX_PIXELS) {
            cache.remove(0);
        }
    }
    Some(off)
}

/// Fill `out` (whose top-left is device pixel `at`) from `src` through `back` (device → `src`
/// pixels): bilinear, premultiplied; what maps outside `src` or from behind the eye stays clear.
fn warp(src: &Pixmap, back: &Homography, at: (u32, u32), out: &mut Pixmap) {
    let (sw, sh) = (src.width() as usize, src.height() as usize);
    let data = src.data();
    let ow = out.width() as usize;
    let texel = |x: isize, y: isize| -> [f32; 4] {
        if x < 0 || y < 0 || x as usize >= sw || y as usize >= sh {
            return [0.0; 4];
        }
        let i = (y as usize * sw + x as usize) * 4;
        match data.get(i..i + 4) {
            Some(p) => [f32::from(p[0]), f32::from(p[1]), f32::from(p[2]), f32::from(p[3])],
            None => [0.0; 4],
        }
    };
    let [m0, m1, m2, m3, m4, m5, m6, m7, m8] = back.0;
    let max_lambda = 1.0 / tilt::NEAR_W;
    for (row, line) in out.data_mut().chunks_exact_mut(ow * 4).enumerate() {
        let y = f64::from(at.1) + row as f64 + 0.5;
        for (col, px) in line.chunks_exact_mut(4).enumerate() {
            let x = f64::from(at.0) + col as f64 + 0.5;
            let (u, v, w) = (m0 * x + m1 * y + m2, m3 * x + m4 * y + m5, m6 * x + m7 * y + m8);
            // `w` is 1 over the source point's depth: in front of the eye only.
            if !(w > 0.0 && w <= max_lambda) {
                continue;
            }
            let (sx, sy) = (u / w - 0.5, v / w - 0.5);
            if !(sx > -1.0 && sy > -1.0 && sx < sw as f64 && sy < sh as f64) {
                continue;
            }
            let (fx, fy) = (sx.floor(), sy.floor());
            let (tx, ty) = ((sx - fx) as f32, (sy - fy) as f32);
            let (ix, iy) = (fx as isize, fy as isize);
            let (p00, p10, p01, p11) = (texel(ix, iy), texel(ix + 1, iy), texel(ix, iy + 1), texel(ix + 1, iy + 1));
            for ch in 0..4 {
                let top = p00[ch] + (p10[ch] - p00[ch]) * tx;
                let bottom = p01[ch] + (p11[ch] - p01[ch]) * tx;
                px[ch] = (top + (bottom - top) * ty).round().clamp(0.0, 255.0) as u8;
            }
            // Premultiplied: no colour above its alpha.
            let alpha = px[3];
            for c in px.iter_mut().take(3) {
                *c = (*c).min(alpha);
            }
        }
    }
}

#[cfg(test)]
#[path = "tilt_tests.rs"]
mod tests;
