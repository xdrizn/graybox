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
    glow: Option<Pixmap>,
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
    let glow = (c.glow > 0.0).then(|| {
        let mut g = back.clone();
        fx::blur(&mut g, c.glow_radius() / 2.0 * scale);
        g
    });
    let to_back = Affine::translate((f64::from(pad), f64::from(pad))) * xf;
    let screen = Screen { c, back, glow, to_back, bb, foot: 1.0 / scale };
    let cover = crate::shadow::spread_alpha(std::slice::from_ref(shape), 0.0, xf, w, h)?;
    let inv = xf.inverse();
    let mut out = Pixmap::new(w, h)?;
    for (i, (o, m)) in out.data_mut().chunks_exact_mut(4).zip(cover.data().chunks_exact(4)).enumerate() {
        let a = f32::from(m.get(3).copied().unwrap_or(0)) / 255.0;
        if a <= 0.0 {
            continue;
        }
        let (x, y) = (i % w as usize, i / w as usize);
        let px = screen.at(inv * Point::new(x as f64 + 0.5, y as f64 + 0.5));
        for (o, v) in o.iter_mut().zip(px) {
            *o = (v * a * 255.0 + 0.5).clamp(0.0, 255.0) as u8;
        }
    }
    Some(out)
}

impl Screen<'_> {
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
            s[0] = self.sample(self.back, local + dir)[0];
            s[2] = self.sample(self.back, local - dir)[2];
        }
        if let Some(g) = self.glow.as_ref() {
            let gl = self.sample(g, local);
            let lift = c.glow as f32 * ((luma(gl) - 0.5).max(0.0) * 2.0);
            for (v, g) in s.iter_mut().zip(gl).take(3) {
                *v += g * lift;
            }
        }
        // Lines and phosphor follow the bulged screen.
        let (along, across) = match c.direction {
            CrtDirection::Horizontal => ((bent.y + 0.5) * bh, (bent.x + 0.5) * bw),
            CrtDirection::Vertical => ((bent.x + 0.5) * bw, (bent.y + 0.5) * bh),
        };
        let lined = dark_share(along, self.foot, c.line_size, c.thickness * c.line_size) as f32;
        let darken = lined * c.line_opacity as f32 * (1.0 - c.bleed as f32 * luma(s).min(1.0));
        // Phosphor columns: a third of the line period, at least a device pixel.
        let cell = (c.line_size / 3.0).max(self.foot);
        let col = (across / cell).floor().rem_euclid(3.0) as usize;
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
}

/// The share of `[t − foot/2, t + foot/2]` lying in the dark part (`0..dark` of each `period`):
/// a box filter, so lines finer than a pixel fade to an even darkening.
fn dark_share(t: f64, foot: f64, period: f64, dark: f64) -> f64 {
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

/// `p` at pixel position `(x, y)` (pixel centres at .5), bilinear, premultiplied; transparent
/// outside.
fn sample(p: &Pixmap, x: f64, y: f64) -> Rgba {
    if !(x.is_finite() && y.is_finite()) {
        return [0.0; 4];
    }
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
