//! Graybox: corner styles. The corner size (`cornerRadius`, one value per corner) draws one of
//! several well-known corner shapes, so a panel can be cut like a sci-fi HUD or a ticket and still
//! grow and shrink its corners from the same field:
//!
//! - **Round**: upstream's circular corner (with corner smoothing).
//! - **Chamfer**: a straight diagonal cut (CSS `corner-shape: bevel`).
//! - **Scoop**: a concave quarter circle around the corner point, like a ticket (CSS `scoop`).
//! - **Notch**: a square step cut into the corner (CSS `notch`).
//! - **V-notch**: a V-shaped cut pointing into the layer: it opens half the size along each
//!   edge and reaches the full size, corner to tip, along the diagonal.
//!
//! Each corner fits in the size × size square at its corner, which is clamped like round corners
//! are (at most half the shorter side), so every style takes the same room. Corner smoothing only
//! shapes round corners.

use figcraft_geom::kurbo::{Affine, Arc, BezPath, PathEl, Point, Vec2};
use serde::{Deserialize, Serialize};

use crate::StrokeAlign;

/// How a layer's corners are shaped (see the module docs).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum CornerStyle {
    #[default]
    Round,
    Chamfer,
    Scoop,
    Notch,
    VNotch,
}

impl CornerStyle {
    pub const ALL: [CornerStyle; 5] = [CornerStyle::Round, CornerStyle::Chamfer, CornerStyle::Scoop, CornerStyle::Notch, CornerStyle::VNotch];

    /// The id used by `node.set` (`cornerStyle`) and in files.
    pub fn id(self) -> &'static str {
        match self {
            CornerStyle::Round => "round",
            CornerStyle::Chamfer => "chamfer",
            CornerStyle::Scoop => "scoop",
            CornerStyle::Notch => "notch",
            CornerStyle::VNotch => "vNotch",
        }
    }

    /// The name the Design panel shows.
    pub fn label(self) -> &'static str {
        match self {
            CornerStyle::Round => "Round",
            CornerStyle::Chamfer => "Chamfer",
            CornerStyle::Scoop => "Scoop",
            CornerStyle::Notch => "Notch",
            CornerStyle::VNotch => "V-notch",
        }
    }

    /// The style named `s`: its id or a common name for it (bevel, inverse round, inset…), any
    /// case, spaces, dashes and underscores ignored.
    pub fn parse(s: &str) -> Option<CornerStyle> {
        let key: String = s.chars().filter(|c| !matches!(c, ' ' | '-' | '_')).flat_map(char::to_lowercase).collect();
        Some(match key.as_str() {
            "round" | "rounded" | "radius" => CornerStyle::Round,
            "chamfer" | "chamfered" | "bevel" | "beveled" | "bevelled" | "cut" | "diagonal" => CornerStyle::Chamfer,
            "scoop" | "scooped" | "inverseround" | "invertedround" | "concave" | "ticket" => CornerStyle::Scoop,
            "notch" | "notched" | "inset" | "step" | "squarenotch" => CornerStyle::Notch,
            "vnotch" | "v" | "invertedchamfer" | "inversechamfer" => CornerStyle::VNotch,
            _ => return None,
        })
    }

    pub fn is_round(&self) -> bool {
        *self == CornerStyle::Round
    }
}

/// The outline of a `w` × `h` box with corner sizes `radii` (`[tl, tr, br, bl]`) in `style`;
/// round corners are upstream's [`figcraft_geom::rounded_rect`] with `smoothing`.
pub fn outline(w: f64, h: f64, radii: [f64; 4], smoothing: f64, style: CornerStyle) -> BezPath {
    if style.is_round() { figcraft_geom::rounded_rect(w, h, radii, smoothing) } else { offset_outline(w, h, radii, style, [0.0; 4]) }
}

/// The outline of a `w` × `h` box with corners in `style` (not smoothed), moved inward by `inset`
/// on each side (`[top, right, bottom, left]`, negative moves it outward), in the box's own
/// coordinates. Straight cuts stay parallel to themselves (mitred, like a stroke's outer edge);
/// a scoop keeps its centre on the box corner; a cut that the offset closes up becomes a square
/// corner. Round corners keep their radius minus the inset (a CSS-like inner edge).
pub fn offset_outline(w: f64, h: f64, radii: [f64; 4], style: CornerStyle, inset: [f64; 4]) -> BezPath {
    let finite = |v: f64| if v.is_finite() { v } else { 0.0 };
    let (w, h) = (finite(w).max(0.0), finite(h).max(0.0));
    let mut path = BezPath::new();
    if w <= 0.0 && h <= 0.0 {
        return path;
    }
    let max_r = w.min(h) / 2.0;
    let radii = radii.map(|r| finite(r).clamp(0.0, max_r));
    let [t, r, b, l] = inset.map(finite);
    // Each corner: its size, the insets of its vertical and horizontal sides, where it sits, and
    // whether the outline (clockwise) reaches it along its vertical side (then its local corner,
    // drawn from the horizontal side to the vertical one, is walked backwards).
    let corners = [
        (radii[0], l, t, Affine::IDENTITY, true),
        (radii[1], r, t, Affine::new([-1.0, 0.0, 0.0, 1.0, w, 0.0]), false),
        (radii[2], r, b, Affine::new([-1.0, 0.0, 0.0, -1.0, w, h]), true),
        (radii[3], l, b, Affine::new([1.0, 0.0, 0.0, -1.0, 0.0, h]), false),
    ];
    for (size, a, bb, at, backwards) in corners {
        let mut piece = at * corner(style, size, a, bb);
        if backwards {
            piece = piece.reverse_subpaths();
        }
        for el in piece.elements() {
            match *el {
                PathEl::MoveTo(p) if path.elements().is_empty() => path.move_to(p),
                PathEl::MoveTo(p) => path.line_to(p),
                other => path.push(other),
            }
        }
    }
    path.close_path();
    path
}

/// One corner in its own frame: the corner point at the origin, its horizontal side along +x
/// (moved to `y = b`), its vertical side along +y (moved to `x = a`), the layer towards +x +y. The
/// path starts on the horizontal side and ends on the vertical one.
fn corner(style: CornerStyle, r: f64, a: f64, b: f64) -> BezPath {
    let mut p = BezPath::new();
    let square = |p: &mut BezPath| p.move_to((a, b));
    if r <= 0.0 {
        square(&mut p);
        return p;
    }
    // Diagonal cuts move by the mean of the two sides' insets.
    let d = (a + b) / 2.0;
    match style {
        CornerStyle::Round => {
            // Radius less the inset, centred where the sides' inset lines meet it.
            let (rx, ry) = ((r - a).max(0.0), (r - b).max(0.0));
            if rx <= 0.0 || ry <= 0.0 {
                square(&mut p);
            } else {
                let c = Point::new(r, r);
                let arc = Arc::new(c, (rx, ry), -std::f64::consts::FRAC_PI_2, -std::f64::consts::FRAC_PI_2, 0.0);
                p.move_to((r, b));
                arc.to_cubic_beziers(0.1, |p1, p2, p3| p.curve_to(p1, p2, p3));
                p.line_to((a, r));
            }
        }
        CornerStyle::Chamfer => {
            // The cut x + y = r moved inward by d along its normal.
            let c = r + d * std::f64::consts::SQRT_2;
            if c <= a + b {
                square(&mut p);
            } else {
                p.move_to((c - b, b));
                p.line_to((a, c - a));
            }
        }
        CornerStyle::Scoop => {
            let rho = r + d;
            if rho <= 0.0 || a * a + b * b >= rho * rho {
                square(&mut p);
            } else {
                let x1 = (rho * rho - b * b).sqrt();
                let y2 = (rho * rho - a * a).sqrt();
                let (start, end) = (b.atan2(x1), y2.atan2(a));
                let arc = Arc::new(Point::ORIGIN, (rho, rho), start, end - start, 0.0);
                p.move_to((x1, b));
                arc.to_cubic_beziers(0.1, |p1, p2, p3| p.curve_to(p1, p2, p3));
                p.line_to((a, y2));
            }
        }
        CornerStyle::Notch => {
            p.move_to((r + a, b));
            p.line_to((r + a, r + b));
            p.line_to((a, r + b));
        }
        CornerStyle::VNotch => {
            // Mouth at r/2 on each side, tip at (r, r); each side of the V moved inward by d.
            let s5 = 5.0_f64.sqrt();
            let (u1, n1) = (Vec2::new(1.0, 2.0) / s5, Vec2::new(2.0, -1.0) / s5);
            let (u2, n2) = (Vec2::new(2.0, 1.0) / s5, Vec2::new(-1.0, 2.0) / s5);
            let p1 = Point::new(r / 2.0, 0.0) + n1 * d;
            let p2 = Point::new(0.0, r / 2.0) + n2 * d;
            let first = p1 + u1 * ((b - p1.y) / u1.y);
            let last = p2 + u2 * ((a - p2.x) / u2.x);
            match meet(p1, u1, p2, u2) {
                Some(tip) if first.x > a && last.y > b && tip.x > first.x && tip.y > last.y => {
                    p.move_to(first);
                    p.line_to(tip);
                    p.line_to(last);
                }
                _ => square(&mut p),
            }
        }
    }
    p
}

/// Where the lines through `p` along `u` and through `q` along `v` cross.
fn meet(p: Point, u: Vec2, q: Point, v: Vec2) -> Option<Point> {
    let den = u.cross(v);
    if den.abs() < 1e-12 {
        return None;
    }
    let t = (q - p).cross(v) / den;
    Some(p + u * t)
}

/// Per-side strokes (weights `[top, right, bottom, left]`) of a box with styled corners: the
/// ring between the outline grown by each side's outer share of its weight and the one shrunk by
/// its inner share, the inner edge running against the outer one (a ring under non-zero too).
pub fn side_strokes(w: f64, h: f64, radii: [f64; 4], style: CornerStyle, sides: [f64; 4], align: StrokeAlign) -> BezPath {
    let sides = sides.map(|s| if s.is_finite() { s.max(0.0) } else { 0.0 });
    if sides.iter().sum::<f64>() <= 0.0 {
        return BezPath::new();
    }
    let out = match align {
        StrokeAlign::Inside => 0.0,
        StrokeAlign::Center => 0.5,
        StrokeAlign::Outside => 1.0,
    };
    let mut ring = offset_outline(w, h, radii, style, sides.map(|s| -s * out));
    let inner = sides.map(|s| s * (1.0 - out));
    let [t, r, b, l] = inner;
    if w - l - r > 0.0 && h - t - b > 0.0 {
        ring.extend(offset_outline(w, h, radii, style, inner).reverse_subpaths().elements().iter().copied());
    }
    ring
}
