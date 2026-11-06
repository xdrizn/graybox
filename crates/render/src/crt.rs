//! Graybox: the CRT backdrop effect. What lies behind the layer, inside its shape, drawn as an old
//! tube screen: bulged, split into red and blue at the edges, glowing, crossed by scanlines and a
//! phosphor mask, darker at the edges. Everything is placed in the layer's own coordinates, so a
//! render cut into bands or tiles draws the same lines.

use figcraft_doc::{Crt, CrtDirection};
use figcraft_geom::kurbo::{Rect, Shape as _, Vec2};
use figcraft_geom::{Affine, Point};
use tiny_skia::Pixmap;

use crate::backdrop::EffectFrame;
use crate::fx;

/// How much the full curvature bulges a corner (the sample lands `1 + K × r²` times further out).
const BULGE: f64 = 0.6;

/// A premultiplied colour, 0–1 per channel.
type Rgba = [f32; 4];

/// One render's CRT: the settings and where the backdrop lies.
struct Screen<'a> {
    c: Crt,
    back: &'a Pixmap,
    /// The blurred backdrop the glow adds, at `1 / step` of the backdrop's size.
    glow: Option<(Pixmap, f64)>,
    /// Local → backdrop pixels.
    to_back: Affine,
    bb: Rect,
    /// One device pixel, in document units: the box each pixel's lines are averaged over.
    foot: f64,
}

/// The CRT look of `frame`'s backdrop through its shape, or `None` when nothing shows.
pub(crate) fn crt(c: &Crt, frame: EffectFrame<'_>) -> Option<Pixmap> {
    let EffectFrame { shape, back, pad, size: (w, h), xf, scale } = frame;
    let det = xf.determinant();
    if w == 0 || h == 0 || !(det.is_finite() && det.abs() > 1e-12 && scale.is_finite() && scale > 0.0) {
        return None;
    }
    let bb = shape.0.bounding_box();
    if !(bb.width() >= 1e-6 && bb.height() >= 1e-6 && bb.width().is_finite() && bb.height().is_finite()) {
        return None;
    }
    let c = c.sanitized();
    let glow = if c.glow > 0.0 { glow_of(back, c.glow_radius() / 2.0 * scale) } else { None };
    let to_back = Affine::translate((f64::from(pad), f64::from(pad))) * xf;
    let screen = Screen { c, back, glow, to_back, bb, foot: 1.0 / scale };
    let cover = crate::shadow::spread_alpha(std::slice::from_ref(shape), 0.0, xf, w, h)?;
    let inv = xf.inverse();
    let mut out = Pixmap::new(w, h)?;
    let row = (w as usize) * 4;
    let shade = |(y, (o, m)): (usize, (&mut [u8], &[u8]))| screen.row(inv, y, o, m);
    // Rows in parallel on the desktop (a full-screen CRT is millions of pixels).
    #[cfg(not(target_arch = "wasm32"))]
    {
        use rayon::prelude::*;
        out.data_mut().par_chunks_mut(row).zip(cover.data().par_chunks(row)).enumerate().for_each(shade);
    }
    #[cfg(target_arch = "wasm32")]
    out.data_mut().chunks_mut(row).zip(cover.data().chunks(row)).enumerate().for_each(shade);
    Some(out)
}

impl Screen<'_> {
    /// Row `y` of the layer into `out` (premultiplied RGBA8), through the shape's coverage `cover`.
    fn row(&self, inv: Affine, y: usize, out: &mut [u8], cover: &[u8]) {
        for (x, (o, m)) in out.chunks_exact_mut(4).zip(cover.chunks_exact(4)).enumerate() {
            let a = f32::from(m.get(3).copied().unwrap_or(0)) / 255.0;
            if a <= 0.0 {
                continue;
            }
            let px = self.at(inv * Point::new(x as f64 + 0.5, y as f64 + 0.5));
            for (o, v) in o.iter_mut().zip(px) {
                *o = (v * a * 255.0 + 0.5).clamp(0.0, 255.0) as u8;
            }
        }
    }

    /// The colour at local point `p`.
    fn at(&self, p: Point) -> Rgba {
        let c = &self.c;
        let (bw, bh) = (self.bb.width(), self.bb.height());
        // Centred unit position: −0.5..0.5 across the layer; a corner lies at r² = 0.5.
        let d = Vec2::new((p.x - self.bb.x0) / bw - 0.5, (p.y - self.bb.y0) / bh - 0.5);
        let r2 = d.hypot2();
        let bent = d * (1.0 + c.curvature * BULGE * r2);
        let mut px = if bent.x.abs() > 0.5 || bent.y.abs() > 0.5 { [0.0, 0.0, 0.0, 1.0] } else { self.tube(d, bent) };
        let vig = 1.0 - c.vignette as f32 * smoothstep(0.25, 0.75, (r2 * 2.0) as f32);
        let gain = c.brightness as f32 * vig;
        let alpha = px[3].clamp(0.0, 1.0);
        for v in px.iter_mut().take(3) {
            *v = (*v * gain).clamp(0.0, alpha);
        }
        px[3] = alpha;
        px
    }

    /// The backdrop at bulged position `bent` (centred unit position `d` before bulging), split,
    /// glowing and crossed by lines and the phosphor mask.
    fn tube(&self, d: Vec2, bent: Vec2) -> Rgba {
        let c = &self.c;
        let (bw, bh) = (self.bb.width(), self.bb.height());
        let local = Point::new(self.bb.x0 + (bent.x + 0.5) * bw, self.bb.y0 + (bent.y + 0.5) * bh);
        let mut s = self.sample(self.back, local);
        let r = d.hypot();
        if c.rgb_split > 0.0 && r > 1e-6 {
            // Red outwards, blue inwards, further the further out.
            let dir = Vec2::new(d.x * bw, d.y * bh);
            let dir = dir / dir.hypot().max(1e-9) * (c.max_split() * r / std::f64::consts::FRAC_1_SQRT_2);
            s[0] = self.channel(local + dir, 0);
            s[2] = self.channel(local - dir, 2);
        }
        if let Some((g, step)) = self.glow.as_ref() {
            let q = self.to_back * local;
            let gl = sample(g, q.x / step, q.y / step);
            let lift = c.glow as f32 * ((luma(gl) - 0.5).max(0.0) * 2.0);
            for (v, g) in s.iter_mut().zip(gl).take(3) {
                *v += g * lift;
            }
        }
        // Lines follow the bulged screen; the phosphor mask lies on the glass (unbent), in whole
        // device pixels, so it never beats with the pixel grid.
        let (along, across) = match c.direction {
            CrtDirection::Horizontal => ((bent.y + 0.5) * bh, (d.x + 0.5) * bw),
            CrtDirection::Vertical => ((bent.x + 0.5) * bw, (d.y + 0.5) * bh),
        };
        let lined = dark_share(along, self.foot, c.line_size, c.thickness * c.line_size) as f32;
        let darken = lined * c.line_opacity as f32 * (1.0 - c.bleed as f32 * luma(s).min(1.0));
        // Phosphor columns: a third of the line period, rounded to whole device pixels (at least one).
        let cell_px = (c.line_size / 3.0 / self.foot).round().max(1.0);
        let col = (across / self.foot / cell_px).floor().rem_euclid(3.0) as usize;
        for (ch, v) in s.iter_mut().take(3).enumerate() {
            let mask = if ch == col { 1.0 } else { 1.0 - c.phosphor as f32 };
            *v *= (1.0 - darken) * mask;
        }
        s
    }

    fn sample(&self, p: &Pixmap, local: Point) -> Rgba {
        let q = self.to_back * local;
        sample(p, q.x, q.y)
    }

    /// One channel of the backdrop at `local` (the colour split reads red and blue apart).
    fn channel(&self, local: Point, ch: usize) -> f32 {
        let q = self.to_back * local;
        sample_channel(self.back, q.x, q.y, ch)
    }
}

/// Whether node `n` has a visible CRT.
pub(crate) fn on(n: &figcraft_doc::Node) -> bool {
    n.effects.iter().any(|e| e.visible && matches!(e.kind, figcraft_doc::EffectKind::Crt(_)))
}

/// Clear `pix` under the CRT's `layer` (placed at `at`, its shape through `lxf`) as far as the layer
/// will not cover it: the CRT draws the whole picture inside its shape, so the unbent layers under
/// it must not show where the bulged picture leaves the backdrop's transparency (in Play, where
/// the game shows through). With coverage `a` and the layer's alpha `l`, what lies under is kept
/// by `(1 − a·op) / (1 − l·op)`: the layer then composites over it to `layer·op + under·(1 − a·op)`,
/// so anti-aliased edges blend without thinning and content outside the shape keeps its backdrop.
pub(crate) fn clear_under(pix: &mut Pixmap, at: (i32, i32), layer: &Pixmap, shape: &crate::shadow::Outline, lxf: Affine, op: f32) {
    let Some(cover) = crate::shadow::spread_alpha(std::slice::from_ref(shape), 0.0, lxf, layer.width(), layer.height()) else { return };
    let op = op.clamp(0.0, 1.0);
    let (pw, ph) = (i64::from(pix.width()), i64::from(pix.height()));
    let lw = i64::from(layer.width().max(1));
    let dst = pix.data_mut();
    for (i, (l, m)) in layer.data().chunks_exact(4).zip(cover.data().chunks_exact(4)).enumerate() {
        let a = f32::from(m.get(3).copied().unwrap_or(0)) / 255.0;
        if a <= 0.0 {
            continue;
        }
        let (x, y) = (i64::from(at.0) + i as i64 % lw, i64::from(at.1) + i as i64 / lw);
        if x < 0 || y < 0 || x >= pw || y >= ph {
            continue;
        }
        let la = f32::from(l.get(3).copied().unwrap_or(0)) / 255.0;
        let rest = 1.0 - la * op;
        let k = if rest <= 1e-6 { 1.0 } else { ((1.0 - a * op) / rest).clamp(0.0, 1.0) };
        if k >= 1.0 {
            continue;
        }
        let j = ((y * pw + x) * 4) as usize;
        if let Some(p) = dst.get_mut(j..j + 4) {
            for v in p.iter_mut() {
                *v = (f32::from(*v) * k + 0.5) as u8;
            }
        }
    }
}

/// Fill `back` (a backdrop read at `origin` on a surface of `size`) past that surface's edges with
/// its nearest pixel inside: the bulge reads past a view's edge, where nothing was drawn, and a
/// hole there would show through the cleared layer.
pub(crate) fn extend_edges(back: &mut Pixmap, origin: (i32, i32), size: (u32, u32)) {
    let (bw, bh) = (i64::from(back.width()), i64::from(back.height()));
    let (ox, oy) = (i64::from(origin.0), i64::from(origin.1));
    let (x0, y0) = ((-ox).max(0), (-oy).max(0));
    let (x1, y1) = ((i64::from(size.0) - ox).min(bw), (i64::from(size.1) - oy).min(bh));
    if x0 >= x1 || y0 >= y1 || (x0 == 0 && y0 == 0 && x1 == bw && y1 == bh) {
        return;
    }
    let src = back.data().to_vec();
    let dst = back.data_mut();
    for y in 0..bh {
        let sy = y.clamp(y0, y1 - 1);
        for x in 0..bw {
            let sx = x.clamp(x0, x1 - 1);
            if (sx, sy) == (x, y) {
                continue;
            }
            let (i, j) = (((y * bw + x) * 4) as usize, ((sy * bw + sx) * 4) as usize);
            if let (Some(d), Some(s)) = (dst.get_mut(i..i + 4), src.get(j..j + 4)) {
                d.copy_from_slice(s);
            }
        }
    }
}

/// Whether a visible CRT on `page` lies in `view`. Its bulge reads the backdrop anywhere under its
/// layer, further than any band overlap: such a render is not cut into bands.
pub(crate) fn in_view(doc: &figcraft_doc::Document, page: figcraft_doc::NodeId, view: &crate::View) -> bool {
    let seen = view.page_rect();
    doc.subtree(page).into_iter().any(|id| {
        doc.node(id).is_some_and(|n| n.visible && on(n)) && crate::draw::paint_bounds(doc, id).is_some_and(|b| b.intersect(seen).area() > 0.0)
    })
}

/// The share of `[t − foot/2, t + foot/2]` lying in the dark part (`0..dark` of each `period`):
/// a box filter, eased into the lines' mean share as a pixel nears a whole period, so lines finer
/// than about two pixels fade to an even darkening instead of beating with the pixel grid.
fn dark_share(t: f64, foot: f64, period: f64, dark: f64) -> f64 {
    let boxed = boxed_share(t, foot, period, dark);
    let w = smoothstep(0.4, 0.75, (foot / period) as f32) as f64;
    boxed + (dark / period - boxed) * w
}

/// The plain box filter of [`dark_share`].
fn boxed_share(t: f64, foot: f64, period: f64, dark: f64) -> f64 {
    // Dark length from 0 up to `t`.
    let upto = |t: f64| {
        let n = (t / period).floor();
        n * dark + (t - n * period).min(dark)
    };
    ((upto(t + foot / 2.0) - upto(t - foot / 2.0)) / foot).clamp(0.0, 1.0)
}

fn smoothstep(lo: f32, hi: f32, v: f32) -> f32 {
    let t = ((v - lo) / (hi - lo)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn luma(c: Rgba) -> f32 {
    0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2]
}

/// The backdrop blurred by `sigma` (device pixels) for the glow, at a half or a quarter of its size
/// when the blur is wide enough not to show it (the glow is soft), and that step.
fn glow_of(back: &Pixmap, sigma: f64) -> Option<(Pixmap, f64)> {
    let step: u32 = if sigma >= 4.0 {
        4
    } else if sigma >= 1.0 {
        2
    } else {
        1
    };
    let mut g = if step == 1 { back.clone() } else { shrink(back, step)? };
    fx::blur(&mut g, sigma / f64::from(step));
    Some((g, f64::from(step)))
}

/// `p` at `1 / step` of its size, each pixel the mean of the block it covers.
fn shrink(p: &Pixmap, step: u32) -> Option<Pixmap> {
    let (w, h) = (p.width(), p.height());
    let (sw, sh) = (w.div_ceil(step), h.div_ceil(step));
    let mut out = Pixmap::new(sw, sh)?;
    let (src, w) = (p.data(), w as usize);
    for (i, o) in out.data_mut().chunks_exact_mut(4).enumerate() {
        let (bx, by) = ((i % sw as usize) * step as usize, (i / sw as usize) * step as usize);
        let mut sum = [0u32; 4];
        let mut n = 0u32;
        for y in by..(by + step as usize).min(h as usize) {
            for x in bx..(bx + step as usize).min(w) {
                if let Some(q) = src.get((y * w + x) * 4..(y * w + x) * 4 + 4) {
                    for (s, v) in sum.iter_mut().zip(q) {
                        *s += u32::from(*v);
                    }
                    n += 1;
                }
            }
        }
        for (o, s) in o.iter_mut().zip(sum) {
            *o = ((s + n / 2) / n.max(1)) as u8;
        }
    }
    Some(out)
}

/// Channel `ch` of `p` at pixel position `(x, y)`, bilinear, clamped to its edges like [`sample`].
fn sample_channel(p: &Pixmap, x: f64, y: f64, ch: usize) -> f32 {
    if !(x.is_finite() && y.is_finite()) {
        return 0.0;
    }
    let (x, y) = (x.clamp(0.5, f64::from(p.width()) - 0.5) - 0.5, y.clamp(0.5, f64::from(p.height()) - 0.5) - 0.5);
    let (x0, y0) = (x.floor(), y.floor());
    let (tx, ty) = ((x - x0) as f32, (y - y0) as f32);
    let (w, h) = (i64::from(p.width()), i64::from(p.height()));
    let data = p.data();
    let get = |xi: f64, yi: f64| -> f32 {
        let (xi, yi) = ((xi as i64).min(w - 1), (yi as i64).min(h - 1));
        data.get(((yi * w + xi) * 4) as usize + ch).map_or(0.0, |v| f32::from(*v) / 255.0)
    };
    let top = get(x0, y0) + (get(x0 + 1.0, y0) - get(x0, y0)) * tx;
    let bottom = get(x0, y0 + 1.0) + (get(x0 + 1.0, y0 + 1.0) - get(x0, y0 + 1.0)) * tx;
    top + (bottom - top) * ty
}

/// `p` at pixel position `(x, y)` (pixel centres at .5), bilinear, premultiplied; past its edges,
/// its edge pixels (a render cut by the view holds only part of the layer's backdrop).
fn sample(p: &Pixmap, x: f64, y: f64) -> Rgba {
    if !(x.is_finite() && y.is_finite()) {
        return [0.0; 4];
    }
    let (x, y) = (x.clamp(0.5, f64::from(p.width()) - 0.5), y.clamp(0.5, f64::from(p.height()) - 0.5));
    let (fx, fy) = (x - 0.5, y - 0.5);
    let (x0, y0) = (fx.floor(), fy.floor());
    let (tx, ty) = ((fx - x0) as f32, (fy - y0) as f32);
    let (w, h) = (i64::from(p.width()), i64::from(p.height()));
    let data = p.data();
    let get = |xi: f64, yi: f64| -> Rgba {
        let (xi, yi) = (xi as i64, yi as i64);
        if xi < 0 || yi < 0 || xi >= w || yi >= h {
            return [0.0; 4];
        }
        let i = ((yi * w + xi) * 4) as usize;
        match data.get(i..i + 4) {
            Some(&[r, g, b, a]) => [r, g, b, a].map(|v| f32::from(v) / 255.0),
            _ => [0.0; 4],
        }
    };
    let (a, b, c, d) = (get(x0, y0), get(x0 + 1.0, y0), get(x0, y0 + 1.0), get(x0 + 1.0, y0 + 1.0));
    let mut out = [0.0; 4];
    for ch in 0..4 {
        let top = a[ch] + (b[ch] - a[ch]) * tx;
        let bottom = c[ch] + (d[ch] - c[ch]) * tx;
        out[ch] = top + (bottom - top) * ty;
    }
    out
}

#[cfg(test)]
#[path = "crt_tests.rs"]
mod tests;
