//! Graybox: stretch brush shapes with a taper you can turn off or size, and organic brushes made
//! in code (no brush images): a brush pen whose width varies like pen pressure, a dry brush that
//! breaks into strands, chalk with rough edges and grain, and a chisel marker.
//!
//! A stretch shape spans x 0..1 along a subpath and y across it in stroke weights (y within
//! [`figcraft_doc::Brush::SHAPE_REACH`]). Its texture is laid out by the subpath's length in
//! stroke weights (`aspect`), so a long stroke gets more texture, not a stretched one.

use crate::width::Polyline;
use figcraft_doc::Brush;
use figcraft_geom::kurbo::{BezPath, Point};

/// The most texture laid out along one subpath, in stroke weights (longer ones repeat coarser).
const MAX_ASPECT: f64 = 2_000.0;

/// A preset's own taper: the parts of the length its start and end thin over.
pub fn default_taper(id: &str) -> (f64, f64) {
    match id {
        "taper" => (0.5, 0.5),
        "ink" => (0.04, 0.3),
        "bristle" => (0.08, 0.12),
        "brushpen" => (0.08, 0.2),
        "dry" => (0.05, 0.2),
        "chalk" => (0.01, 0.03),
        _ => (0.0, 0.0),
    }
}

/// The taper `brush` draws with: the parts of the length its start and end thin over, at most the
/// whole length together. Taper off: none.
pub fn taper_of(brush: &Brush) -> (f64, f64) {
    if !brush.taper {
        return (0.0, 0.0);
    }
    let (ds, de) = default_taper(&brush.preset);
    let pick = |v: Option<f64>, d: f64| v.filter(|v| v.is_finite()).unwrap_or(d).clamp(0.0, 1.0);
    let (s, e) = (pick(brush.taper_start, ds), pick(brush.taper_end, de));
    if s + e > 1.0 { (s / (s + e), e / (s + e)) } else { (s, e) }
}

/// How wide the brush is at `x` (0..1 along the subpath) for a taper `(start, end)`: 0 at a tapered
/// end, 1 past the tapers.
pub fn envelope(x: f64, (start, end): (f64, f64)) -> f64 {
    let ramp = |u: f64| {
        let u = u.clamp(0.0, 1.0);
        1.0 - (1.0 - u) * (1.0 - u)
    };
    let a = if start > 1e-9 { ramp(x / start) } else { 1.0 };
    let b = if end > 1e-9 { ramp((1.0 - x) / end) } else { 1.0 };
    a * b
}

/// A closed subpath that starts halfway along its longest segment, so a stretch brush's two ends
/// meet on a straight stretch, not at a corner. Open subpaths are returned as they are.
pub fn seam_on_a_side(line: &Polyline) -> Polyline {
    if !line.closed || line.points.len() < 3 {
        return line.clone();
    }
    let longest = line
        .points
        .windows(2)
        .enumerate()
        .map(|(i, w)| (i, (w[1] - w[0]).hypot()))
        .fold((0, 0.0), |best, (i, l)| if l > best.1 { (i, l) } else { best });
    let i = longest.0;
    let (Some(a), Some(b)) = (line.points.get(i), line.points.get(i + 1)) else { return line.clone() };
    let mid = a.midpoint(*b);
    // The points after the seam to the end (the end is the start again), then on from the second.
    let n = line.points.len();
    let mut points = vec![mid];
    points.extend(line.points.iter().skip(i + 1).take(n - 1 - i).copied());
    points.extend(line.points.iter().skip(1).take(i).copied());
    points.push(mid);
    let mut out = Polyline { points: Vec::with_capacity(points.len()), lengths: Vec::with_capacity(points.len()), closed: true };
    for p in points {
        match out.points.last() {
            None => {
                out.points.push(p);
                out.lengths.push(0.0);
            }
            Some(last) => {
                let d = (p - *last).hypot();
                if d > 1e-9 {
                    out.lengths.push(out.length() + d);
                    out.points.push(p);
                }
            }
        }
    }
    out
}

/// A repeatable pseudo-random number in 0..1 for `i`, channel `k` (splitmix64).
fn rand(seed: u32, i: u64, k: u64) -> f64 {
    let mut z = u64::from(seed) ^ i.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ k.wrapping_mul(0xD1B5_4A32_D192_ED03) ^ 0x6A09_E667_F3BC_C908;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 11) as f64 / (1u64 << 53) as f64
}

/// Smooth value noise in 0..1 at `x` (one new value per unit), channel `k`.
fn noise(seed: u32, k: u64, x: f64) -> f64 {
    let x = if x.is_finite() { x.clamp(0.0, 1e9) } else { 0.0 };
    let i = x.floor();
    let f = x - i;
    let u = f * f * (3.0 - 2.0 * f);
    let i = i as u64;
    let (a, b) = (rand(seed, i, k), rand(seed, i + 1, k));
    a + (b - a) * u
}

/// A closed band over x `x0..x1` in `n` steps between edges `f(x) = (top, bottom)`.
fn band(p: &mut BezPath, x0: f64, x1: f64, n: usize, f: impl Fn(f64) -> (f64, f64)) {
    let n = n.max(1);
    let xs = (0..=n).map(|i| x0 + (x1 - x0) * i as f64 / n as f64);
    for (i, x) in xs.clone().enumerate() {
        let q = (x, f(x).0);
        if i == 0 { p.move_to(q) } else { p.line_to(q) }
    }
    for x in xs.rev() {
        p.line_to((x, f(x).1));
    }
    p.close_path();
}

/// The stretch shape of preset `id` for a subpath `aspect` stroke weights long (`None`: not a
/// stretch preset). `closed`: the subpath is a loop, so a marker has no chisel cut at its seam.
pub fn stretch_unit(id: &str, brush: &Brush, aspect: f64, closed: bool) -> Option<BezPath> {
    let taper = taper_of(brush);
    let a = if aspect.is_finite() { aspect.clamp(1e-3, MAX_ASPECT) } else { 1.0 };
    // Samples along the length: about four per stroke weight.
    let n = ((a * 4.0).ceil() as usize).clamp(64, 4_096);
    let env = |x: f64| envelope(x, taper);
    let seed = brush.seed;
    let mut p = BezPath::new();
    match id {
        "taper" => band(&mut p, 0.0, 1.0, n, |x| (-0.375 * env(x), 0.375 * env(x))),
        // A blunt start, uneven edges and a long tail.
        "ink" => band(&mut p, 0.0, 1.0, n, |x| {
            let w = env(x);
            (-0.5 * w + 0.04 * (9.0 * std::f64::consts::PI * x).sin() * w, 0.45 * w + 0.04 * (13.0 * std::f64::consts::PI * x + 1.0).sin() * w)
        }),
        // Edges that cross three times: a twisting ribbon.
        "ribbon" => band(&mut p, 0.0, 1.0, n, |x| {
            let c = 0.5 * (3.0 * std::f64::consts::PI * x).cos() * env(x);
            (-c, c)
        }),
        // Four thin strands with ragged ends.
        "bristle" => {
            for k in 0..4 {
                let (kf, y) = (f64::from(k), -0.375 + 0.25 * f64::from(k));
                let own = (taper.0 * (0.6 + 0.4 * kf / 3.0), taper.1 * (0.6 + 0.4 * (3.0 - kf) / 3.0));
                band(&mut p, 0.0, 1.0, n, |x| {
                    let h = 0.09 * envelope(x, own).sqrt();
                    (y - h, y + h)
                });
            }
        }
        // Width that swells and thins along the stroke, like pressure on a brush pen.
        "brushpen" => band(&mut p, 0.0, 1.0, n, |x| {
            let w = env(x) * (0.62 + 0.38 * noise(seed, 1, x * a / 5.0));
            (-0.5 * w, (0.44 + 0.04 * noise(seed, 2, x * a / 3.0)) * w)
        }),
        "dry" => dry(&mut p, a, n, taper, seed),
        "chalk" => chalk(&mut p, a, n, taper, seed),
        // A chisel tip: flat, even and cut on a slant at both ends.
        "marker" => {
            let c = if closed { 0.0 } else { (0.5 / a).min(0.2) };
            let (top, bottom) = (|x: f64| -0.46 * env(x), |x: f64| 0.46 * env(x));
            let k = n.max(2);
            for i in 0..=k {
                let x = (1.0 - c) * i as f64 / k as f64;
                if i == 0 { p.move_to((x, top(x))) } else { p.line_to((x, top(x))) }
            }
            for i in (0..=k).rev() {
                let x = c + (1.0 - c) * i as f64 / k as f64;
                p.line_to((x, bottom(x)));
            }
            p.close_path();
        }
        _ => return None,
    }
    Some(p)
}

/// Seven strands of uneven thickness that break up more and more towards the end, as a brush
/// running out of paint does.
fn dry(p: &mut BezPath, a: f64, n: usize, taper: (f64, f64), seed: u32) {
    const STRANDS: u64 = 7;
    for j in 0..STRANDS {
        let y = -0.42 + 0.84 * j as f64 / (STRANDS - 1) as f64;
        let h = 0.035 + 0.035 * rand(seed, j, 10);
        let wobble = 0.03 * (rand(seed, j, 11) - 0.5);
        // Walk along the strand in stroke weights: a run of paint, then a gap.
        let mut pos = rand(seed, j, 12) * 0.6;
        let mut k = 0u64;
        while pos < a && k < 4_000 {
            let t = pos / a;
            let run = (1.5 + 7.0 * rand(seed, j * 4_096 + k, 13)) * (1.0 - 0.65 * t);
            let end = (pos + run.max(0.3)).min(a);
            let (x0, x1) = (pos / a, end / a);
            let steps = (((x1 - x0) * n as f64).ceil() as usize).clamp(4, 256);
            // Each run thins to its own ends over a fifth of a weight.
            let fade = 0.2 / a;
            band(p, x0, x1, steps, |x| {
                let ends = ((x - x0) / fade).min((x1 - x) / fade).clamp(0.0, 1.0).sqrt();
                let w = h * envelope(x, taper).sqrt() * ends;
                (y + wobble - w, y + wobble + w)
            });
            let gap = (0.15 + 1.1 * rand(seed, j * 4_096 + k, 14)) * (0.4 + 1.6 * t);
            pos = end + gap;
            k += 1;
        }
    }
}

/// A band with rough edges and grain: small holes wound the other way, which the non-zero fill
/// leaves empty.
fn chalk(p: &mut BezPath, a: f64, n: usize, taper: (f64, f64), seed: u32) {
    let rough = |x: f64, k: u64| 0.05 * noise(seed, k, x * a * 2.5) + 0.03 * rand(seed, (x * n as f64) as u64, k + 2);
    let env = |x: f64| envelope(x, taper);
    band(p, 0.0, 1.0, n, |x| (-(0.44 + rough(x, 20)) * env(x), (0.44 + rough(x, 21)) * env(x)));
    // Holes on a jittered grid (columns 0.45 weights apart, four rows), never overlapping.
    let cols = ((a / 0.45) as u64).min(3_000);
    for c in 0..cols {
        for r in 0..4u64 {
            let cell = c * 4 + r;
            if rand(seed, cell, 30) > 0.35 {
                continue;
            }
            let x = (c as f64 + 0.25 + 0.5 * rand(seed, cell, 31)) * 0.45 / a;
            let y = -0.27 + 0.18 * r as f64 + 0.03 * (rand(seed, cell, 32) - 0.5);
            let s = 0.03 + 0.03 * rand(seed, cell, 33);
            // Only where the band is wide enough around it.
            if y.abs() + s > 0.36 * env(x) {
                continue;
            }
            let (hx, hy) = (s * (0.7 + 0.6 * rand(seed, cell, 34)) / a, s);
            p.move_to(Point::new(x - hx, y - hy));
            p.line_to(Point::new(x - hx, y + hy));
            p.line_to(Point::new(x + hx, y + hy));
            p.line_to(Point::new(x + hx, y - hy));
            p.close_path();
        }
    }
}
