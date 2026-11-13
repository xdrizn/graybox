//! Graybox: 3D rotation of a layer (G17). A layer's tilt is a rotation about its X and Y axes plus a
//! perspective distance; it is drawn flat and then warped by the projective transform (a 3×3
//! homography) worked out here. Pure math: no document, no rendering.
//!
//! Conventions match Unity's numbers (`Quaternion.Euler`): **+X tips the top away from the viewer,
//! +Y brings the right side nearer**. Coordinates are page-like (y down); the depth `z` points to
//! the viewer. The layer's own 2D transform (its Z rotation) comes first, then the tilt about its
//! centre in its parent's space, as Unity applies Z, then X, then Y.
//! See `docs/game/specs/2026-11-13-frames-3d-design.md`.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub use figcraft_geom::{Affine, Point, Rect};

/// The plugin data key a layer's tilt is stored under: `"x,y,perspective"`.
pub const KEY: &str = "graybox.rotation3d";
/// The steepest tilt either way, in degrees: past it a layer is a sliver seen edge on.
pub const MAX_ANGLE: f64 = 80.0;
/// The perspective a new tilt gets (page units from the eye to the layer).
pub const DEFAULT_PERSPECTIVE: f64 = 1000.0;
pub const MIN_PERSPECTIVE: f64 = 50.0;
pub const MAX_PERSPECTIVE: f64 = 100_000.0;
/// No corner of a layer's box comes nearer than this share of the eye's distance: the perspective
/// is raised as needed (no point behind the eye, no blow-up at ±80°).
const NEAREST: f64 = 0.8;
/// What is drawn of a layer's overflow (effects, layers past its box) stops this close to the eye
/// (the homogeneous `w` of a point, 1 on the plane through the layer's centre).
pub const NEAR_W: f64 = 0.05;

/// A layer's rotation about X and Y (degrees) and its perspective (page units).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tilt {
    pub x: f64,
    pub y: f64,
    pub perspective: f64,
}

impl Default for Tilt {
    fn default() -> Self {
        Self { x: 0.0, y: 0.0, perspective: DEFAULT_PERSPECTIVE }
    }
}

impl Tilt {
    /// A tilt with its angles within ±[`MAX_ANGLE`] and its perspective within
    /// [`MIN_PERSPECTIVE`]…[`MAX_PERSPECTIVE`]; what isn't a finite number is left at its default.
    pub fn new(x: f64, y: f64, perspective: f64) -> Self {
        let angle = |a: f64| if a.is_finite() { a.clamp(-MAX_ANGLE, MAX_ANGLE) } else { 0.0 };
        let d = if perspective.is_finite() { perspective.clamp(MIN_PERSPECTIVE, MAX_PERSPECTIVE) } else { DEFAULT_PERSPECTIVE };
        Self { x: angle(x), y: angle(y), perspective: d }
    }

    /// No rotation about X or Y: drawn as usual.
    pub fn is_flat(&self) -> bool {
        self.x.abs() < 1e-9 && self.y.abs() < 1e-9
    }

    /// The stored form, `x,y,perspective`.
    pub fn encode(&self) -> String {
        format!("{},{},{}", self.x, self.y, self.perspective)
    }

    /// The tilt in its stored form (clamped), or `None` when it isn't one.
    pub fn decode(s: &str) -> Option<Self> {
        let mut it = s.split(',').map(|p| p.trim().parse::<f64>());
        let (Some(Ok(x)), Some(Ok(y)), Some(Ok(d)), None) = (it.next(), it.next(), it.next(), it.next()) else { return None };
        Some(Self::new(x, y, d))
    }

    /// The tilt `t` of the way from `a` to `b` (smart animate).
    pub fn lerp(a: Self, b: Self, t: f64) -> Self {
        let t = if t.is_finite() { t } else { 1.0 };
        let l = |p: f64, q: f64| p + (q - p) * t;
        Self::new(l(a.x, b.x), l(a.y, b.y), l(a.perspective, b.perspective))
    }

    /// Centred point `(x, y)` turned: where it lands in the plane and its depth toward the eye.
    fn rotate(&self, x: f64, y: f64) -> (f64, f64, f64) {
        let (sx, cx) = self.x.to_radians().sin_cos();
        let (sy, cy) = self.y.to_radians().sin_cos();
        (x * cy - y * sx * sy, y * cx, x * sy + y * sx * cy)
    }

    /// The eye's distance for a layer whose box corners (centred) are `corners`: the perspective,
    /// raised so no corner comes nearer than [`NEAREST`] of it.
    fn eye(&self, corners: &[(f64, f64)]) -> f64 {
        let near = corners.iter().map(|(x, y)| self.rotate(*x, *y).2).filter(|z| z.is_finite()).fold(0.0, f64::max);
        let d = self.perspective.max(near / NEAREST);
        if d.is_finite() { d.min(1e15) } else { MAX_PERSPECTIVE }
    }

    /// The warp of a box centred on `center` with half sizes `half`, both in the same space.
    pub fn homography(&self, center: Point, half: (f64, f64)) -> Homography {
        let (hw, hh) = (half.0.abs(), half.1.abs());
        self.about(center, &[(-hw, -hh), (hw, -hh), (hw, hh), (-hw, hh)])
    }

    /// The warp about `center` of a layer whose box corners, relative to it, are `corners`.
    fn about(&self, center: Point, corners: &[(f64, f64)]) -> Homography {
        if self.is_flat() || !center.x.is_finite() || !center.y.is_finite() {
            return Homography::IDENTITY;
        }
        let d = self.eye(corners);
        let (sx, cx) = self.x.to_radians().sin_cos();
        let (sy, cy) = self.y.to_radians().sin_cos();
        // (x, y, 1) ↦ (x', y', 1 − z/d): the turned point over its depth.
        let m = Homography([cy, -sx * sy, 0.0, 0.0, cx, 0.0, -sy / d, -sx * cy / d, 1.0]);
        Homography::translate(center.x, center.y) * m * Homography::translate(-center.x, -center.y)
    }

    /// The warp of a layer in its own space: `a` maps it to its parent, its box is `w` × `h`. The
    /// tilt turns it about its centre in the parent's space (after its Z rotation); this is that
    /// warp brought back into the layer's space (`a⁻¹ · H · a`).
    pub fn local(&self, a: Affine, w: f64, h: f64) -> Homography {
        if self.is_flat() {
            return Homography::IDENTITY;
        }
        let c = a * Point::new(w / 2.0, h / 2.0);
        let corners = [(0.0, 0.0), (w, 0.0), (w, h), (0.0, h)].map(|(x, y)| {
            let p = a * Point::new(x, y);
            (p.x - c.x, p.y - c.y)
        });
        let hp = self.about(c, &corners);
        let fa = Homography::from_affine(a);
        match fa.inverse() {
            Some(inv) => inv * hp * fa,
            None => Homography::IDENTITY,
        }
    }
}

/// A projective transform of the plane, row-major 3×3 on `(x, y, 1)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Homography(pub [f64; 9]);

impl Homography {
    pub const IDENTITY: Self = Self([1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]);

    pub fn translate(x: f64, y: f64) -> Self {
        Self([1.0, 0.0, x, 0.0, 1.0, y, 0.0, 0.0, 1.0])
    }

    pub fn from_affine(a: Affine) -> Self {
        let [a0, b0, c0, d0, e0, f0] = a.as_coeffs();
        Self([a0, c0, e0, b0, d0, f0, 0.0, 0.0, 1.0])
    }

    pub fn is_identity(&self) -> bool {
        *self == Self::IDENTITY
    }

    /// `p` in homogeneous coordinates `(x, y, w)` (not divided).
    pub fn apply_h(&self, p: Point) -> (f64, f64, f64) {
        let [a, b, c, d, e, f, g, h, i] = self.0;
        (a * p.x + b * p.y + c, d * p.x + e * p.y + f, g * p.x + h * p.y + i)
    }

    /// Where `p` lands, or `None` when it is at or behind the eye or not finite.
    pub fn apply(&self, p: Point) -> Option<Point> {
        let (x, y, w) = self.apply_h(p);
        let q = Point::new(x / w, y / w);
        (w > 1e-12 && q.x.is_finite() && q.y.is_finite()).then_some(q)
    }

    /// The inverse warp, `None` when it has none. A point in front of the eye maps back with a
    /// positive `w` (the inverse of its depth), one behind it with a negative one.
    pub fn inverse(&self) -> Option<Self> {
        let [a, b, c, d, e, f, g, h, i] = self.0;
        let (ca, cb, cc) = (e * i - f * h, f * g - d * i, d * h - e * g);
        let det = a * ca + b * cb + c * cc;
        if !det.is_finite() || det.abs() < 1e-300 {
            return None;
        }
        let k = 1.0 / det;
        let m = [ca, c * h - b * i, b * f - c * e, cb, a * i - c * g, c * d - a * f, cc, b * g - a * h, a * e - b * d].map(|v| v * k);
        m.iter().all(|v| v.is_finite()).then_some(Self(m))
    }

    /// Whether `p` is drawn: in front of the eye by at least [`NEAR_W`].
    pub fn in_front(&self, p: Point) -> bool {
        self.apply_h(p).2 >= NEAR_W
    }

    /// The bounding box of `r` warped, what is nearer the eye than [`NEAR_W`] cut off; `None` when
    /// nothing of it is in front.
    pub fn bounds(&self, r: Rect) -> Option<Rect> {
        if self.is_identity() {
            return Some(r);
        }
        let corners = [Point::new(r.x0, r.y0), Point::new(r.x1, r.y0), Point::new(r.x1, r.y1), Point::new(r.x0, r.y1)];
        let w = |p: Point| self.apply_h(p).2 - NEAR_W;
        let mut out: Option<Rect> = None;
        let mut add = |p: Point| {
            if let Some(q) = self.apply(p) {
                out = Some(out.map_or(Rect::from_points(q, q), |o| o.union_pt(q)));
            }
        };
        for (a, b) in corners.iter().zip(corners.iter().cycle().skip(1)) {
            let (wa, wb) = (w(*a), w(*b));
            if wa >= 0.0 {
                add(*a);
            }
            if (wa >= 0.0) != (wb >= 0.0) && (wa - wb).abs() > 1e-300 {
                // Where the edge crosses the near plane.
                add(a.lerp(*b, wa / (wa - wb)));
            }
        }
        out.filter(|o| o.x0.is_finite() && o.y0.is_finite() && o.x1.is_finite() && o.y1.is_finite())
    }
}

impl std::ops::Mul for Homography {
    type Output = Self;
    fn mul(self, o: Self) -> Self {
        let ([a0, a1, a2, a3, a4, a5, a6, a7, a8], [b0, b1, b2, b3, b4, b5, b6, b7, b8]) = (self.0, o.0);
        Self([
            a0 * b0 + a1 * b3 + a2 * b6,
            a0 * b1 + a1 * b4 + a2 * b7,
            a0 * b2 + a1 * b5 + a2 * b8,
            a3 * b0 + a4 * b3 + a5 * b6,
            a3 * b1 + a4 * b4 + a5 * b7,
            a3 * b2 + a4 * b5 + a5 * b8,
            a6 * b0 + a7 * b3 + a8 * b6,
            a6 * b1 + a7 * b4 + a8 * b7,
            a6 * b2 + a7 * b5 + a8 * b8,
        ])
    }
}

#[cfg(test)]
mod tests;
