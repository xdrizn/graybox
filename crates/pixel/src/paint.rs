//! Painting on a cel: a path of pixels stamped with a brush, mirrored, wrapped round the edges in
//! tile mode, then painted once each; and the bucket fill.

use crate::canvas::{CLEAR, Canvas, Rgba, over_pixel};
use crate::raster::{MAX_POINTS, Pt, pixel_perfect, polyline};
use serde::{Deserialize, Serialize};

/// The most brush stamps × brush pixels one path paints (the rest of the path is left out).
const MAX_WORK: usize = 16 << 20;

/// The brush's footprint.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BrushShape {
    #[default]
    Square,
    Circle,
}

/// A brush: its size in art pixels (1–64) and shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Brush {
    pub size: u32,
    pub shape: BrushShape,
}

impl Default for Brush {
    fn default() -> Self {
        Self { size: 1, shape: BrushShape::Square }
    }
}

impl Brush {
    pub const MAX_SIZE: u32 = 64;

    /// A brush of `size` (kept in 1–64).
    pub fn new(size: u32, shape: BrushShape) -> Self {
        Self { size: size.clamp(1, Self::MAX_SIZE), shape }
    }

    /// The pixels the brush covers around a point at (0, 0). An even size reaches one pixel
    /// further right and down than left and up.
    pub fn offsets(&self) -> Vec<Pt> {
        let n = i64::from(self.size.clamp(1, Self::MAX_SIZE));
        let lo = -((n - 1) / 2);
        let hi = lo + n - 1;
        // The brush's middle and its radius, slightly less than half its size so a 3-pixel round
        // brush is a plus and a 5-pixel one is round.
        let mid = lo as f64 + n as f64 / 2.0;
        let r = n as f64 / 2.0 - 0.25;
        let mut out = Vec::with_capacity((n * n) as usize);
        for dy in lo..=hi {
            for dx in lo..=hi {
                let inside = match self.shape {
                    BrushShape::Square => true,
                    BrushShape::Circle => {
                        let (cx, cy) = (dx as f64 + 0.5 - mid, dy as f64 + 0.5 - mid);
                        n <= 2 || cx * cx + cy * cy <= r * r
                    }
                };
                if inside {
                    out.push((dx, dy));
                }
            }
        }
        out
    }
}

/// Mirror axes, doubled so they can sit on a pixel's edge (even) or through its middle (odd): a
/// vertical axis at `x2 / 2` mirrors left and right, a horizontal one at `y2 / 2` up and down.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Symmetry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x2: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y2: Option<i64>,
}

impl Symmetry {
    /// `p` and its mirror images (one, two or four pixels, no repeats).
    pub fn images(&self, p: Pt) -> Vec<Pt> {
        let mut out = vec![p];
        if let Some(x2) = self.x2 {
            out.push((x2 - 1 - p.0, p.1));
        }
        if let Some(y2) = self.y2 {
            let n = out.len();
            for i in 0..n {
                if let Some(&(x, y)) = out.get(i) {
                    out.push((x, y2 - 1 - y));
                }
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }
}

/// How a path is painted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ink {
    pub color: Rgba,
    pub brush: Brush,
    pub symmetry: Symmetry,
    /// Tile mode: what goes past an edge comes back in from the opposite one.
    pub tile: bool,
    /// Paint transparency instead of the colour.
    pub erase: bool,
    /// Replace colour: paint only over pixels of this colour (the others stay).
    pub replace: Option<Rgba>,
}

impl Default for Ink {
    fn default() -> Self {
        Self { color: [0, 0, 0, 255], brush: Brush::default(), symmetry: Symmetry::default(), tile: false, erase: false, replace: None }
    }
}

/// A freehand stroke's pixels: the pointer's positions joined by lines, with L-corners taken out
/// when `perfect` (pixel-perfect).
pub fn freehand(points: &[Pt], perfect: bool) -> Vec<Pt> {
    let path = polyline(points);
    if perfect { pixel_perfect(&path) } else { path }
}

/// The canvas pixels `path` covers with `ink` on a `w` × `h` cel: each point stamped with the
/// brush, mirrored, wrapped (tile mode) or cut at the edges; each pixel once, in a stable order.
pub fn covered(path: &[Pt], ink: &Ink, w: u32, h: u32) -> Vec<Pt> {
    let offsets = ink.brush.offsets();
    let (w, h) = (i64::from(w), i64::from(h));
    let mut seen = vec![false; (w.max(0) * h.max(0)) as usize];
    let mut out = Vec::new();
    // Stamps that can't reach the cel (nor can their mirror images) are skipped; a path is cut
    // short past a bounded amount of work.
    let reach = i64::from(ink.brush.size);
    let near = |p: Pt| ink.symmetry.images(p).iter().any(|&(x, y)| x > -reach && y > -reach && x < w + reach && y < h + reach);
    let stamps = (MAX_WORK / offsets.len().max(1)).max(1);
    for &(px, py) in path.iter().filter(|p| ink.tile || near(**p)).take(stamps.min(MAX_POINTS)) {
        for &(dx, dy) in &offsets {
            for (x, y) in ink.symmetry.images((px.saturating_add(dx), py.saturating_add(dy))) {
                let (x, y) = if ink.tile { (x.rem_euclid(w.max(1)), y.rem_euclid(h.max(1))) } else { (x, y) };
                if x < 0 || y < 0 || x >= w || y >= h {
                    continue;
                }
                if let Some(s) = seen.get_mut((y * w + x) as usize)
                    && !*s
                {
                    *s = true;
                    out.push((x, y));
                }
            }
        }
    }
    out
}

/// Paints `path` on `cel` with `ink`: each covered pixel once (a half-transparent colour is laid
/// over what was there once, not built up where the stroke crosses itself). How many changed.
pub fn paint(cel: &mut Canvas, path: &[Pt], ink: &Ink) -> usize {
    let mut changed = 0;
    for (x, y) in covered(path, ink, cel.width(), cel.height()) {
        let Some(old) = cel.get(x, y) else { continue };
        if ink.replace.is_some_and(|r| !similar(old, r, 0)) {
            continue;
        }
        let new = if ink.erase {
            CLEAR
        } else if ink.color[3] == 255 {
            ink.color
        } else {
            over_pixel(old, ink.color, 255)
        };
        if new != old {
            cel.set(x, y, new);
            changed += 1;
        }
    }
    changed
}

/// Shade (G16.6): each pixel `path` covers whose colour is on `ramp` moves `step` places along it
/// (stopping at its ends); other colours and transparent pixels stay. A pixel moves once per
/// stroke however often the path crosses it.
pub fn shade(cel: &mut Canvas, path: &[Pt], ink: &Ink, ramp: &[Rgba], step: i32) -> usize {
    let Some(last) = ramp.len().checked_sub(1) else { return 0 };
    let mut changed = 0;
    for (x, y) in covered(path, ink, cel.width(), cel.height()) {
        let Some(old) = cel.get(x, y).filter(|p| p[3] > 0) else { continue };
        let Some(i) = ramp.iter().position(|c| *c == old) else { continue };
        let j = (i as i64).saturating_add(i64::from(step)).clamp(0, last as i64) as usize;
        if let Some(&new) = ramp.get(j)
            && new != old
        {
            cel.set(x, y, new);
            changed += 1;
        }
    }
    changed
}

/// The colours shading steps through: the sprite's own `palette` in its order, or without one the
/// colours of `art` from dark to light.
pub fn shade_ramp(palette: &[String], art: &Canvas) -> Vec<Rgba> {
    let own: Vec<Rgba> = palette.iter().filter_map(|h| crate::canvas::parse_hex(h)).collect();
    if !own.is_empty() {
        return own;
    }
    // The art's colours (the 256 most used when it has more), by how many pixels have each.
    let mut counts: std::collections::HashMap<Rgba, usize> = std::collections::HashMap::new();
    for p in art.to_bytes().chunks_exact(4).filter(|p| p[3] > 0) {
        *counts.entry([p[0], p[1], p[2], p[3]]).or_default() += 1;
    }
    let mut used: Vec<(Rgba, usize)> = counts.into_iter().collect();
    used.sort_unstable_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let mut used: Vec<Rgba> = used.into_iter().take(256).map(|(c, _)| c).collect();
    used.sort_by_key(|p| 299 * u32::from(p[0]) + 587 * u32::from(p[1]) + 114 * u32::from(p[2]));
    used
}

/// Whether two pixels count as the same colour within `tolerance` (0: exactly; every transparent
/// pixel is the same).
pub fn similar(a: Rgba, b: Rgba, tolerance: u8) -> bool {
    if a[3] == 0 && b[3] == 0 {
        return true;
    }
    a.iter().zip(b.iter()).all(|(x, y)| x.abs_diff(*y) <= tolerance)
}

/// The bucket: from `x`, `y`, every pixel of the same colour (within `tolerance`) is painted `color`.
/// `contiguous`: only those joined to it by a side (wrapping round the edges in `tile` mode);
/// otherwise every such pixel of the cel. How many changed.
pub fn fill(cel: &mut Canvas, (x, y): Pt, color: Rgba, contiguous: bool, tolerance: u8, tile: bool) -> usize {
    let Some(target) = cel.get(x, y) else { return 0 };
    // A see-through colour is laid over each pixel, as the pencil does; transparency (erasing) replaces.
    let ink = |old: Rgba| if color[3] == 255 || color[3] == 0 { color } else { over_pixel(old, color, 255) };
    let (w, h) = (i64::from(cel.width()), i64::from(cel.height()));
    let mut changed = 0;
    if !contiguous {
        for yy in 0..h {
            for xx in 0..w {
                if let Some(c) = cel.get(xx, yy).filter(|c| similar(*c, target, tolerance))
                    && ink(c) != c
                {
                    cel.set(xx, yy, ink(c));
                    changed += 1;
                }
            }
        }
        return changed;
    }
    let mut seen = vec![false; (w * h) as usize];
    let mut todo = vec![(x, y)];
    while let Some((cx, cy)) = todo.pop() {
        let Some(s) = seen.get_mut((cy * w + cx) as usize) else { continue };
        if *s {
            continue;
        }
        *s = true;
        let Some(c) = cel.get(cx, cy) else { continue };
        if !similar(c, target, tolerance) {
            continue;
        }
        if ink(c) != c {
            cel.set(cx, cy, ink(c));
            changed += 1;
        }
        for (nx, ny) in [(cx + 1, cy), (cx - 1, cy), (cx, cy + 1), (cx, cy - 1)] {
            let (nx, ny) = if tile { (nx.rem_euclid(w), ny.rem_euclid(h)) } else { (nx, ny) };
            if nx >= 0 && ny >= 0 && nx < w && ny < h {
                todo.push((nx, ny));
            }
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    const INK: Rgba = [20, 30, 40, 255];

    fn picture(c: &Canvas) -> Vec<String> {
        (0..i64::from(c.height()))
            .map(|y| (0..i64::from(c.width())).map(|x| if c.get(x, y).is_some_and(|p| p[3] > 0) { '#' } else { '.' }).collect())
            .collect()
    }

    fn brush_picture(b: Brush) -> Vec<String> {
        let mut c = Canvas::new(b.size, b.size).unwrap();
        let o = -((i64::from(b.size) - 1) / 2);
        for (x, y) in b.offsets() {
            c.set(x - o, y - o, INK);
        }
        picture(&c)
    }

    #[test]
    fn round_brushes_look_round() {
        assert_eq!(brush_picture(Brush::new(3, BrushShape::Circle)), [".#.", "###", ".#."]);
        assert_eq!(brush_picture(Brush::new(4, BrushShape::Circle)), [".##.", "####", "####", ".##."]);
        assert_eq!(brush_picture(Brush::new(2, BrushShape::Circle)), ["##", "##"]);
        assert_eq!(Brush::new(5, BrushShape::Square).offsets().len(), 25);
        assert_eq!(Brush::new(0, BrushShape::Square).size, 1);
        assert_eq!(Brush::new(500, BrushShape::Square).size, 64);
    }

    #[test]
    fn a_mirror_axis_through_the_middle_of_a_pixel_keeps_it() {
        let s = Symmetry { x2: Some(5), y2: None };
        assert_eq!(s.images((2, 0)), vec![(2, 0)], "pixel 2 is on the axis 2.5");
        assert_eq!(s.images((0, 0)), vec![(0, 0), (4, 0)]);
        let both = Symmetry { x2: Some(4), y2: Some(4) };
        assert_eq!(both.images((0, 0)), vec![(0, 0), (0, 3), (3, 0), (3, 3)]);
    }

    #[test]
    fn a_mirrored_stroke_draws_both_sides() {
        let mut c = Canvas::new(6, 2).unwrap();
        let ink = Ink { color: INK, symmetry: Symmetry { x2: Some(6), y2: None }, ..Ink::default() };
        paint(&mut c, &freehand(&[(0, 0), (1, 1)], false), &ink);
        assert_eq!(picture(&c), ["#....#", ".#..#."]);
    }

    #[test]
    fn tile_mode_wraps_a_stroke_round_the_edges() {
        let mut c = Canvas::new(4, 3).unwrap();
        let ink = Ink { color: INK, tile: true, ..Ink::default() };
        paint(&mut c, &freehand(&[(2, 1), (5, 1)], false), &ink);
        assert_eq!(picture(&c), ["....", "####", "...."]);
        let mut plain = Canvas::new(4, 3).unwrap();
        paint(&mut plain, &freehand(&[(2, 1), (5, 1)], false), &Ink { color: INK, ..Ink::default() });
        assert_eq!(picture(&plain), ["....", "..##", "...."], "without tile mode the edge cuts it");
    }

    #[test]
    fn half_transparent_ink_is_laid_once_where_a_stroke_crosses_itself() {
        let mut c = Canvas::filled(3, 1, [255, 255, 255, 255]).unwrap();
        let ink = Ink { color: [0, 0, 0, 128], ..Ink::default() };
        paint(&mut c, &freehand(&[(0, 0), (2, 0), (0, 0)], false), &ink);
        assert_eq!(c.get(1, 0), Some([127, 127, 127, 255]));
    }

    #[test]
    fn erasing_clears_and_a_big_brush_reaches_round() {
        let mut c = Canvas::filled(5, 5, INK).unwrap();
        let ink = Ink { erase: true, brush: Brush::new(3, BrushShape::Circle), ..Ink::default() };
        assert_eq!(paint(&mut c, &[(2, 2)], &ink), 5);
        assert_eq!(picture(&c), ["#####", "##.##", "#...#", "##.##", "#####"]);
    }

    #[test]
    fn the_bucket_fills_what_touches_or_every_match() {
        // A ring with a hole in the middle and an outside.
        let mut c = Canvas::new(5, 5).unwrap();
        for p in crate::raster::rect_outline((1, 1), (3, 3)) {
            c.set(p.0, p.1, INK);
        }
        let red = [255, 0, 0, 255];
        assert_eq!(fill(&mut c, (0, 0), red, true, 0, false), 16, "the outside only");
        assert_eq!(c.get(2, 2), Some(CLEAR));
        let mut every = Canvas::new(5, 5).unwrap();
        for p in crate::raster::rect_outline((1, 1), (3, 3)) {
            every.set(p.0, p.1, INK);
        }
        assert_eq!(fill(&mut every, (0, 0), red, false, 0, false), 17, "outside and the hole");
        assert_eq!(fill(&mut every, (9, 9), red, true, 0, false), 0, "outside the cel does nothing");
    }

    #[test]
    fn the_bucket_with_tolerance_takes_near_colours() {
        let mut c = Canvas::from_pixels(3, 1, vec![[100, 100, 100, 255], [104, 100, 100, 255], [200, 0, 0, 255]]).unwrap();
        assert_eq!(fill(&mut c, (0, 0), INK, true, 4, false), 2);
        assert_eq!(c.get(2, 0), Some([200, 0, 0, 255]));
    }

    #[test]
    fn a_big_brush_on_a_long_path_stays_bounded() {
        let mut c = Canvas::new(16, 16).unwrap();
        let ink = Ink { brush: Brush::new(64, BrushShape::Circle), ..Ink::default() };
        let path = freehand(&[(-1000, -1000), (1000, 1000)], true);
        paint(&mut c, &path, &ink);
        assert!(!c.is_clear());
    }

    #[test]
    fn the_bucket_lays_a_see_through_colour_over_like_the_pencil() {
        let mut a = Canvas::filled(3, 1, [0, 0, 255, 255]).unwrap();
        let half_red = [255, 0, 0, 128];
        assert_eq!(fill(&mut a, (0, 0), half_red, true, 0, false), 3);
        let mut b = Canvas::filled(3, 1, [0, 0, 255, 255]).unwrap();
        paint(&mut b, &[(0, 0)], &Ink { color: half_red, ..Ink::default() });
        assert_eq!(a.get(0, 0), b.get(0, 0), "same colour as the pencil gives");
        assert_eq!(a.get(2, 0).map(|p| p[3]), Some(255));
        // Erasing with the bucket still clears.
        assert_eq!(fill(&mut a, (0, 0), CLEAR, true, 0, false), 3);
        assert_eq!(a.get(1, 0), Some(CLEAR));
    }

    #[test]
    fn replace_ink_paints_only_over_its_colour() {
        let mut c = Canvas::new(4, 1).unwrap();
        c.set(1, 0, [9, 9, 9, 255]);
        c.set(2, 0, [9, 9, 9, 255]);
        let ink = Ink { color: INK, replace: Some([9, 9, 9, 255]), ..Ink::default() };
        assert_eq!(paint(&mut c, &freehand(&[(0, 0), (3, 0)], false), &ink), 2);
        assert_eq!(picture(&c), [".##."]);
        assert_eq!(c.get(1, 0), Some(INK));
    }
}

#[cfg(test)]
mod shade_tests {
    use super::*;

    const DARK: Rgba = [30, 20, 60, 255];
    const MID: Rgba = [90, 60, 140, 255];
    const LIGHT: Rgba = [200, 170, 240, 255];
    const OTHER: Rgba = [10, 200, 10, 255];

    #[test]
    fn shading_moves_each_covered_pixel_one_step_along_the_ramp_once_per_stroke() {
        let mut c = Canvas::new(4, 1).unwrap();
        for (x, p) in [DARK, MID, LIGHT, OTHER].into_iter().enumerate() {
            c.set(x as i64, 0, p);
        }
        // The path crosses pixel 0 twice: it still moves one step.
        let path = [(0, 0), (1, 0), (2, 0), (3, 0), (0, 0)];
        let ramp = [DARK, MID, LIGHT];
        assert_eq!(shade(&mut c, &path, &Ink::default(), &ramp, 1), 2, "light stays at the end; other colours stay");
        assert_eq!([c.get(0, 0), c.get(1, 0), c.get(2, 0), c.get(3, 0)], [Some(MID), Some(LIGHT), Some(LIGHT), Some(OTHER)]);
        assert_eq!(shade(&mut c, &path, &Ink::default(), &ramp, -1), 3);
        assert_eq!([c.get(0, 0), c.get(1, 0), c.get(2, 0)], [Some(DARK), Some(MID), Some(MID)]);
        let mut empty = Canvas::new(2, 1).unwrap();
        assert_eq!(shade(&mut empty, &path, &Ink::default(), &ramp, 1), 0, "transparent pixels stay transparent");
    }

    #[test]
    fn without_a_palette_the_ramp_is_the_art_colours_from_dark_to_light() {
        let mut c = Canvas::new(3, 1).unwrap();
        for (x, p) in [LIGHT, DARK, MID].into_iter().enumerate() {
            c.set(x as i64, 0, p);
        }
        assert_eq!(shade_ramp(&[], &c), vec![DARK, MID, LIGHT]);
        assert_eq!(shade_ramp(&["#5a3c8c".into(), "zz".into(), "#1e143c".into()], &c), vec![MID, DARK], "the palette's own order");
    }
}
