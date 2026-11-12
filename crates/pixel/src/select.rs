//! Selections on a cel: which art pixels are selected (a mask the cel's size), made with the
//! marquee (a box), the lasso (a polygon) or the magic wand (a colour region), and combined
//! (replace, add, subtract, intersect). Stored as text (packed bits, base64).

use base64::Engine as _;

use crate::canvas::{Canvas, Rgba};
use crate::paint::similar;
use crate::raster::{Pt, line};

/// How a new selection combines with the one there is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Op {
    #[default]
    Replace,
    Add,
    Subtract,
    Intersect,
}

impl Op {
    /// The op a name gives (`replace`, `add`, `subtract`, `intersect`).
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "replace" => Self::Replace,
            "add" => Self::Add,
            "subtract" => Self::Subtract,
            "intersect" => Self::Intersect,
            _ => return None,
        })
    }
}

/// The selected pixels of a `width` × `height` cel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mask {
    width: u32,
    height: u32,
    bits: Vec<bool>,
}

impl Mask {
    /// Nothing selected, or None for a size a canvas can't have.
    pub fn new(width: u32, height: u32) -> Option<Self> {
        let ok = width > 0 && height > 0 && width <= Canvas::MAX_SIDE && height <= Canvas::MAX_SIDE;
        ok.then(|| Self { width, height, bits: vec![false; width as usize * height as usize] })
    }

    /// Everything selected.
    pub fn all(width: u32, height: u32) -> Option<Self> {
        let mut m = Self::new(width, height)?;
        m.bits.fill(true);
        Some(m)
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    fn index(&self, x: i64, y: i64) -> Option<usize> {
        (x >= 0 && y >= 0 && x < i64::from(self.width) && y < i64::from(self.height)).then(|| y as usize * self.width as usize + x as usize)
    }

    /// Whether pixel `x`, `y` is selected (false outside).
    pub fn get(&self, x: i64, y: i64) -> bool {
        self.index(x, y).and_then(|i| self.bits.get(i).copied()).unwrap_or(false)
    }

    /// Selects or deselects pixel `x`, `y` (nothing outside).
    pub fn set(&mut self, x: i64, y: i64, on: bool) {
        if let Some(b) = self.index(x, y).and_then(|i| self.bits.get_mut(i)) {
            *b = on;
        }
    }

    /// Whether nothing is selected.
    pub fn is_empty(&self) -> bool {
        !self.bits.iter().any(|b| *b)
    }

    /// How many pixels are selected.
    pub fn count(&self) -> usize {
        self.bits.iter().filter(|b| **b).count()
    }

    /// The smallest box holding every selected pixel: (x0, y0, x1, y1), both corners included.
    pub fn bounds(&self) -> Option<(i64, i64, i64, i64)> {
        let mut b: Option<(i64, i64, i64, i64)> = None;
        for y in 0..i64::from(self.height) {
            for x in 0..i64::from(self.width) {
                if self.get(x, y) {
                    b = Some(match b {
                        None => (x, y, x, y),
                        Some((x0, y0, x1, y1)) => (x0.min(x), y0.min(y), x1.max(x), y1.max(y)),
                    });
                }
            }
        }
        b
    }

    /// The pixels not selected.
    pub fn inverted(&self) -> Self {
        Self { width: self.width, height: self.height, bits: self.bits.iter().map(|b| !b).collect() }
    }

    /// The box with corners `a` and `b` (both included, cut at the edges).
    pub fn rect(width: u32, height: u32, a: Pt, b: Pt) -> Option<Self> {
        let mut m = Self::new(width, height)?;
        let (w, h) = (i64::from(width), i64::from(height));
        let (x0, x1) = (a.0.min(b.0).max(0), a.0.max(b.0).min(w - 1));
        let (y0, y1) = (a.1.min(b.1).max(0), a.1.max(b.1).min(h - 1));
        for y in y0..=y1 {
            for x in x0..=x1 {
                m.set(x, y, true);
            }
        }
        Some(m)
    }

    /// The polygon through `points` (a lasso, closed back to its first point): the pixels whose
    /// middle is inside it, and the pixels its edges pass through.
    pub fn polygon(width: u32, height: u32, points: &[Pt]) -> Option<Self> {
        let mut m = Self::new(width, height)?;
        let (w, h) = (i64::from(width), i64::from(height));
        // Far points are brought in (a lasso that goes off the cel still closes along its edge).
        let pts: Vec<(f64, f64)> = points.iter().take(1 << 16).map(|&(x, y)| (x.clamp(-1, w) as f64 + 0.5, y.clamp(-1, h) as f64 + 0.5)).collect();
        if pts.is_empty() {
            return Some(m);
        }
        for y in 0..h {
            let cy = y as f64 + 0.5;
            let mut xs: Vec<f64> = Vec::new();
            for i in 0..pts.len() {
                let (Some(&(ax, ay)), Some(&(bx, by))) = (pts.get(i), pts.get((i + 1) % pts.len())) else { continue };
                if (ay <= cy && by > cy) || (by <= cy && ay > cy) {
                    xs.push(ax + (cy - ay) / (by - ay) * (bx - ax));
                }
            }
            xs.sort_by(f64::total_cmp);
            for pair in xs.chunks(2) {
                if let [from, to] = pair {
                    for x in 0..w {
                        let cx = x as f64 + 0.5;
                        if cx >= *from && cx <= *to {
                            m.set(x, y, true);
                        }
                    }
                }
            }
        }
        let clamped: Vec<Pt> = points.iter().take(1 << 16).map(|&(x, y)| (x.clamp(-1, w), y.clamp(-1, h))).collect();
        for i in 0..clamped.len() {
            if let (Some(&a), Some(&b)) = (clamped.get(i), clamped.get((i + 1) % clamped.len())) {
                for (x, y) in line(a, b) {
                    m.set(x, y, true);
                }
            }
        }
        Some(m)
    }

    /// The magic wand at `at`: the pixels of the same colour (within `tolerance`), touching it by a
    /// side when `contiguous`, else anywhere on the cel. Empty outside the cel.
    pub fn wand(cel: &Canvas, at: Pt, contiguous: bool, tolerance: u8) -> Option<Self> {
        let mut m = Self::new(cel.width(), cel.height())?;
        let Some(target) = cel.get(at.0, at.1) else { return Some(m) };
        let same = |c: Rgba| similar(c, target, tolerance);
        if !contiguous {
            for y in 0..i64::from(cel.height()) {
                for x in 0..i64::from(cel.width()) {
                    if cel.get(x, y).is_some_and(same) {
                        m.set(x, y, true);
                    }
                }
            }
            return Some(m);
        }
        let mut todo = vec![at];
        while let Some((x, y)) = todo.pop() {
            if m.get(x, y) || !cel.get(x, y).is_some_and(same) {
                continue;
            }
            m.set(x, y, true);
            todo.extend([(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)]);
        }
        Some(m)
    }

    /// This selection combined with `other` (the same size; another size changes nothing).
    pub fn combine(&mut self, other: &Mask, op: Op) {
        if (self.width, self.height) != (other.width, other.height) {
            return;
        }
        for (a, b) in self.bits.iter_mut().zip(&other.bits) {
            *a = match op {
                Op::Replace => *b,
                Op::Add => *a || *b,
                Op::Subtract => *a && !*b,
                Op::Intersect => *a && *b,
            };
        }
    }

    /// The selection moved by `dx`, `dy` (what goes past the edges is lost).
    pub fn moved(&self, dx: i64, dy: i64) -> Self {
        let mut m = Self { width: self.width, height: self.height, bits: vec![false; self.bits.len()] };
        for y in 0..i64::from(self.height) {
            for x in 0..i64::from(self.width) {
                if self.get(x, y) {
                    m.set(x.saturating_add(dx), y.saturating_add(dy), true);
                }
            }
        }
        m
    }

    /// The selection as text: its pixels packed eight to a byte, row by row, in base64.
    pub fn encode(&self) -> String {
        let mut bytes = vec![0u8; self.bits.len().div_ceil(8)];
        for (i, b) in self.bits.iter().enumerate() {
            if *b && let Some(byte) = bytes.get_mut(i / 8) {
                *byte |= 1 << (i % 8);
            }
        }
        base64::engine::general_purpose::STANDARD.encode(bytes)
    }

    /// A selection of `width` × `height` from [`Self::encode`]'s text (None when it doesn't fit).
    pub fn decode(text: &str, width: u32, height: u32) -> Option<Self> {
        let bytes = base64::engine::general_purpose::STANDARD.decode(text.trim()).ok()?;
        let mut m = Self::new(width, height)?;
        if bytes.len() != m.bits.len().div_ceil(8) {
            return None;
        }
        for (i, b) in m.bits.iter_mut().enumerate() {
            *b = bytes.get(i / 8).is_some_and(|byte| byte & (1 << (i % 8)) != 0);
        }
        Some(m)
    }

    /// The edges between selected and unselected pixels (for drawing the selection's outline), in
    /// art-pixel corner coordinates, each straight run of them as one segment `((x0, y0), (x1, y1))`.
    pub fn edges(&self) -> Vec<(Pt, Pt)> {
        let (w, h) = (i64::from(self.width), i64::from(self.height));
        let mut out = Vec::new();
        // Across: the boundary above row `y`; down: the boundary left of column `x`.
        for y in 0..=h {
            let mut start = None;
            for x in 0..=w {
                let edge = x < w && self.get(x, y - 1) != self.get(x, y);
                match (edge, start) {
                    (true, None) => start = Some(x),
                    (false, Some(s)) => {
                        out.push(((s, y), (x, y)));
                        start = None;
                    }
                    _ => {}
                }
            }
        }
        for x in 0..=w {
            let mut start = None;
            for y in 0..=h {
                let edge = y < h && self.get(x - 1, y) != self.get(x, y);
                match (edge, start) {
                    (true, None) => start = Some(y),
                    (false, Some(s)) => {
                        out.push(((x, s), (x, y)));
                        start = None;
                    }
                    _ => {}
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(m: &Mask) -> Vec<String> {
        (0..i64::from(m.height())).map(|y| (0..i64::from(m.width())).map(|x| if m.get(x, y) { '#' } else { '.' }).collect()).collect()
    }

    #[test]
    fn a_box_is_cut_at_the_edges_and_both_corners_count() {
        let m = Mask::rect(5, 3, (3, 1), (9, -4)).unwrap();
        assert_eq!(rows(&m), ["...##", "...##", "....."]);
        assert_eq!(m.bounds(), Some((3, 0, 4, 1)));
        assert_eq!(m.count(), 4);
    }

    #[test]
    fn a_lasso_takes_what_it_goes_round_and_its_own_line() {
        let m = Mask::polygon(7, 7, &[(1, 1), (5, 1), (5, 5), (1, 5)]).unwrap();
        assert_eq!(rows(&m), [".......", ".#####.", ".#####.", ".#####.", ".#####.", ".#####.", "......."]);
        // A thin lasso (a line there and back) still takes its pixels.
        let thin = Mask::polygon(5, 1, &[(0, 0), (4, 0)]).unwrap();
        assert_eq!(rows(&thin), ["#####"]);
    }

    #[test]
    fn the_wand_takes_one_colour_region_or_every_pixel_of_it() {
        let mut c = Canvas::new(5, 1).unwrap();
        let red = [255, 0, 0, 255];
        for x in [0, 1, 3] {
            c.set(x, 0, red);
        }
        assert_eq!(rows(&Mask::wand(&c, (0, 0), true, 0).unwrap()), ["##..."]);
        assert_eq!(rows(&Mask::wand(&c, (0, 0), false, 0).unwrap()), ["##.#."]);
        assert_eq!(rows(&Mask::wand(&c, (2, 0), true, 0).unwrap()), ["..#.."]);
        assert!(Mask::wand(&c, (9, 0), true, 0).unwrap().is_empty());
    }

    #[test]
    fn selections_add_subtract_and_intersect() {
        let a = Mask::rect(4, 1, (0, 0), (2, 0)).unwrap();
        let b = Mask::rect(4, 1, (1, 0), (3, 0)).unwrap();
        let with = |op| {
            let mut m = a.clone();
            m.combine(&b, op);
            rows(&m)
        };
        assert_eq!(with(Op::Add), ["####"]);
        assert_eq!(with(Op::Subtract), ["#..."]);
        assert_eq!(with(Op::Intersect), [".##."]);
        assert_eq!(with(Op::Replace), [".###"]);
        assert_eq!(rows(&a.inverted()), ["...#"]);
    }

    #[test]
    fn a_selection_survives_its_text_and_a_wrong_size_is_refused() {
        let m = Mask::polygon(13, 9, &[(0, 0), (12, 3), (4, 8)]).unwrap();
        assert_eq!(Mask::decode(&m.encode(), 13, 9), Some(m.clone()));
        assert_eq!(Mask::decode(&m.encode(), 64, 64), None);
        assert_eq!(Mask::decode("not base64!", 13, 9), None);
    }

    #[test]
    fn a_single_pixel_has_four_edges_and_moving_loses_what_falls_off() {
        let m = Mask::rect(3, 3, (1, 1), (1, 1)).unwrap();
        assert_eq!(m.edges().len(), 4);
        assert_eq!(rows(&m.moved(1, 0)), ["...", "..#", "..."]);
        assert!(m.moved(5, 0).is_empty());
    }

    #[test]
    fn a_box_outline_is_four_long_edges_and_a_notch_splits_one() {
        let m = Mask::rect(6, 4, (1, 1), (3, 2)).unwrap();
        let mut e = m.edges();
        e.sort();
        assert_eq!(e, [((1, 1), (1, 3)), ((1, 1), (4, 1)), ((1, 3), (4, 3)), ((4, 1), (4, 3))]);
        let mut notched = m.clone();
        notched.set(2, 1, false);
        assert_eq!(notched.edges().len(), 8, "the top splits in two and the notch adds three sides");
    }
}
