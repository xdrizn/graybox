//! Smart resize (G16.3): a sprite shown at another size than its own never scales its art. Without
//! 9-slice its middle row and column repeat or go (the default), its last row and column repeat
//! outward, or the whole sprite repeats as a tile. With 9-slice (cuts in art pixels) the corners
//! stay and the edges and the middle are tiled or stretched by whole pixels.

use serde::{Deserialize, Serialize};

use crate::canvas::Canvas;

/// How a sprite without 9-slice grows or shrinks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Resize {
    /// The middle row and column repeat when it grows and go first when it shrinks, so its
    /// corners and borders stay.
    #[default]
    Middle,
    /// The art stays at the top left: its last row and column repeat outward, or it is cut.
    Edges,
    /// The whole sprite repeats from the top left (a tile filling an area), or is cut.
    Tile,
}

impl Resize {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "middle" => Some(Self::Middle),
            "edges" => Some(Self::Edges),
            "tile" => Some(Self::Tile),
            _ => None,
        }
    }
}

/// How a 9-slice's edges and middle fill their space.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SliceFill {
    /// Repeated whole.
    #[default]
    Tile,
    /// Stretched by whole pixels (each source pixel becomes a run of pixels).
    Stretch,
}

impl SliceFill {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "tile" => Some(Self::Tile),
            "stretch" => Some(Self::Stretch),
            _ => None,
        }
    }
}

/// 9-slice cuts in art pixels: top, right, bottom, left.
pub type Insets = [u32; 4];

/// For each of `m` pixels along one side, the source pixel (of `n`) it shows, with `lo` and `hi`
/// pixels kept at the two ends and the part between filled with `fill`.
fn sliced(n: u32, m: u32, lo: u32, hi: u32, fill: SliceFill) -> Vec<u32> {
    let lo = lo.min(n);
    let hi = hi.min(n - lo);
    if m < lo + hi {
        // Smaller than its two ends: each end gives up its share.
        let keep_lo = (u64::from(lo) * u64::from(m) / u64::from((lo + hi).max(1))) as u32;
        let keep_hi = m - keep_lo;
        return (0..keep_lo).chain((n - keep_hi)..n).collect();
    }
    let mid = n - lo - hi;
    let span = m - lo - hi;
    let middle = (0..span).map(|i| match (mid, fill) {
        // Nothing between the ends: the inner pixel of the first end repeats.
        (0, _) => lo.saturating_sub(1),
        (k, SliceFill::Tile) => lo + i % k,
        (k, SliceFill::Stretch) => lo + (u64::from(i) * u64::from(k) / u64::from(span.max(1))) as u32,
    });
    (0..lo).chain(middle).chain((n - hi)..n).collect()
}

/// One side's source pixels for `resize` (no 9-slice).
fn plain(n: u32, m: u32, resize: Resize) -> Vec<u32> {
    match resize {
        Resize::Middle => {
            // Ends of (n - 1) / 2 and the rest less one: the middle pixel is what repeats.
            let lo = n.saturating_sub(1) / 2;
            sliced(n, m, lo, n.saturating_sub(lo + 1), SliceFill::Tile)
        }
        Resize::Edges => (0..m).map(|i| i.min(n.saturating_sub(1))).collect(),
        Resize::Tile => (0..m).map(|i| i % n.max(1)).collect(),
    }
}

/// `frame` shown at `w` × `h` art pixels: by its 9-slice cuts when it has some, else by `resize`.
/// None for a size a canvas can't have.
pub fn fit(frame: &Canvas, (w, h): (u32, u32), resize: Resize, slice: Option<(Insets, SliceFill)>) -> Option<Canvas> {
    let (n, k) = (frame.width(), frame.height());
    if (n, k) == (w, h) {
        return Some(frame.clone());
    }
    let (xs, ys) = match slice {
        Some(([t, r, b, l], fill)) => (sliced(n, w, l, r, fill), sliced(k, h, t, b, fill)),
        None => (plain(n, w, resize), plain(k, h, resize)),
    };
    let mut out = Canvas::new(w, h)?;
    for (y, sy) in ys.iter().enumerate() {
        for (x, sx) in xs.iter().enumerate() {
            if let Some(c) = frame.get(i64::from(*sx), i64::from(*sy)) {
                out.set(x as i64, y as i64, c);
            }
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::Rgba;

    /// A canvas from rows of letters: each letter its own colour, `.` transparent.
    fn art(rows: &[&str]) -> Canvas {
        let w = rows[0].len() as u32;
        let px: Vec<Rgba> = rows.iter().flat_map(|r| r.bytes()).map(|b| if b == b'.' { [0, 0, 0, 0] } else { [b, 0, 0, 255] }).collect();
        Canvas::from_pixels(w, rows.len() as u32, px).unwrap()
    }

    fn rows(c: &Canvas) -> Vec<String> {
        (0..i64::from(c.height()))
            .map(|y| (0..i64::from(c.width())).map(|x| c.get(x, y).filter(|p| p[3] > 0).map_or('.', |p| p[0] as char)).collect())
            .collect()
    }

    #[test]
    fn growing_repeats_the_middle_and_keeps_the_corners() {
        let c = art(&["abc", "def", "ghi"]);
        assert_eq!(rows(&fit(&c, (5, 4), Resize::Middle, None).unwrap()), ["abbbc", "deeef", "deeef", "ghhhi"]);
    }

    #[test]
    fn shrinking_takes_the_middle_out_first_then_shares_the_ends() {
        let c = art(&["abcde"]);
        assert_eq!(rows(&fit(&c, (4, 1), Resize::Middle, None).unwrap()), ["abde"]);
        assert_eq!(rows(&fit(&c, (3, 1), Resize::Middle, None).unwrap()), ["ade"]);
        assert_eq!(rows(&fit(&c, (1, 1), Resize::Middle, None).unwrap()), ["e"]);
    }

    #[test]
    fn edges_repeat_outward_and_tile_repeats_the_sprite() {
        let c = art(&["ab", "cd"]);
        assert_eq!(rows(&fit(&c, (4, 3), Resize::Edges, None).unwrap()), ["abbb", "cddd", "cddd"]);
        assert_eq!(rows(&fit(&c, (5, 3), Resize::Tile, None).unwrap()), ["ababa", "cdcdc", "ababa"]);
        assert_eq!(rows(&fit(&c, (1, 1), Resize::Edges, None).unwrap()), ["a"]);
    }

    #[test]
    fn a_nine_slice_keeps_corners_and_tiles_or_stretches_the_rest() {
        let c = art(&["abcd", "efgh", "ijkl"]);
        let tiled = fit(&c, (7, 3), Resize::Middle, Some(([1, 1, 1, 1], SliceFill::Tile))).unwrap();
        assert_eq!(rows(&tiled), ["abcbcbd", "efgfgfh", "ijkjkjl"]);
        let stretched = fit(&c, (7, 3), Resize::Middle, Some(([1, 1, 1, 1], SliceFill::Stretch))).unwrap();
        assert_eq!(rows(&stretched), ["abbbccd", "efffggh", "ijjjkkl"]);
    }

    #[test]
    fn cuts_wider_than_the_size_share_it_and_odd_cuts_are_kept_in_bounds() {
        let c = art(&["abcd"]);
        assert_eq!(rows(&fit(&c, (2, 1), Resize::Middle, Some(([0, 2, 0, 2], SliceFill::Tile))).unwrap()), ["ad"]);
        // Cuts larger than the sprite are cut down to it; with nothing between them the inner
        // pixel of the first end repeats.
        assert_eq!(rows(&fit(&c, (6, 1), Resize::Middle, Some(([0, 9, 0, 9], SliceFill::Tile))).unwrap()), ["abcddd"]);
    }
}
