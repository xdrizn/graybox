//! Changing what is selected on a cel (or the whole cel): move, flip, rotate by quarter turns,
//! outline, and colour replacement. Each lifts the selected pixels, changes them, and puts them
//! back; transparent pixels of what was lifted don't cover what they land on.

use crate::canvas::{CLEAR, Canvas, Rgba, over_pixel};
use crate::paint::similar;
use crate::select::Mask;

/// A change to the selected pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    /// Moved by whole art pixels.
    Move { dx: i64, dy: i64 },
    /// Mirrored left to right (`horizontal`) or top to bottom, within the selection's box.
    Flip { horizontal: bool },
    /// A quarter turn, clockwise or not, about the middle of the selection's box.
    Rotate { clockwise: bool },
}

/// The selected pixels taken off `cel` (left transparent there) as a canvas the cel's size.
fn lift(cel: &mut Canvas, mask: &Mask) -> Option<Canvas> {
    let mut out = Canvas::new(cel.width(), cel.height())?;
    for y in 0..i64::from(cel.height()) {
        for x in 0..i64::from(cel.width()) {
            if mask.get(x, y)
                && let Some(c) = cel.get(x, y)
            {
                out.set(x, y, c);
                cel.set(x, y, CLEAR);
            }
        }
    }
    Some(out)
}

/// Applies `change` to the pixels of `cel` that `mask` selects (all of them for None). Returns the
/// selection where the pixels went, or None when nothing could be done (a size that doesn't fit).
pub fn apply(cel: &mut Canvas, mask: Option<&Mask>, change: Change) -> Option<Mask> {
    let all;
    let mask = match mask {
        Some(m) if (m.width(), m.height()) == (cel.width(), cel.height()) => m,
        Some(_) => return None,
        None => {
            all = Mask::all(cel.width(), cel.height())?;
            &all
        }
    };
    let (x0, y0, x1, y1) = mask.bounds()?;
    let (bw, bh) = (x1 - x0 + 1, y1 - y0 + 1);
    let lifted = lift(cel, mask)?;
    let mut moved = Mask::new(cel.width(), cel.height())?;
    // Where each selected pixel lands.
    let to = |x: i64, y: i64| -> (i64, i64) {
        match change {
            Change::Move { dx, dy } => (x.saturating_add(dx), y.saturating_add(dy)),
            Change::Flip { horizontal: true } => (x0 + x1 - x, y),
            Change::Flip { horizontal: false } => (x, y0 + y1 - y),
            Change::Rotate { clockwise } => {
                // The turned box keeps its middle (to the nearest pixel).
                let (nx0, ny0) = (x0 + (bw - bh) / 2, y0 + (bh - bw) / 2);
                let (lx, ly) = (x - x0, y - y0);
                if clockwise { (nx0 + (bh - 1 - ly), ny0 + lx) } else { (nx0 + ly, ny0 + (bw - 1 - lx)) }
            }
        }
    };
    for y in y0..=y1 {
        for x in x0..=x1 {
            if !mask.get(x, y) {
                continue;
            }
            let (nx, ny) = to(x, y);
            moved.set(nx, ny, true);
            if let Some(c) = lifted.get(x, y).filter(|c| c[3] > 0)
                && let Some(under) = cel.get(nx, ny)
            {
                cel.set(nx, ny, over_pixel(under, c, 255));
            }
        }
    }
    Some(moved)
}

/// Draws an outline in `color` round the opaque pixels of `cel` within `mask` (all for None):
/// `outside` on the transparent pixels next to them by a side, else on their own edge pixels.
/// How many pixels changed.
pub fn outline(cel: &mut Canvas, mask: Option<&Mask>, color: Rgba, outside: bool) -> usize {
    let opaque = |c: &Canvas, x: i64, y: i64| c.get(x, y).is_some_and(|p| p[3] > 0);
    let before = cel.clone();
    let mut changed = 0;
    for y in 0..i64::from(cel.height()) {
        for x in 0..i64::from(cel.width()) {
            if mask.is_some_and(|m| !m.get(x, y)) {
                continue;
            }
            let near = [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)];
            let hit = if outside {
                !opaque(&before, x, y) && near.iter().any(|&(nx, ny)| opaque(&before, nx, ny))
            } else {
                opaque(&before, x, y) && near.iter().any(|&(nx, ny)| !opaque(&before, nx, ny))
            };
            if hit && cel.get(x, y) != Some(color) {
                cel.set(x, y, color);
                changed += 1;
            }
        }
    }
    changed
}

/// Every pixel of `cel` within `mask` (all for None) that is `from` (within `tolerance`) becomes
/// `to`. How many changed.
pub fn replace_color(cel: &mut Canvas, mask: Option<&Mask>, from: Rgba, to: Rgba, tolerance: u8) -> usize {
    let mut changed = 0;
    for y in 0..i64::from(cel.height()) {
        for x in 0..i64::from(cel.width()) {
            if mask.is_some_and(|m| !m.get(x, y)) {
                continue;
            }
            if let Some(c) = cel.get(x, y)
                && similar(c, from, tolerance)
                && (c[3] > 0 || from[3] == 0)
                && c != to
            {
                cel.set(x, y, to);
                changed += 1;
            }
        }
    }
    changed
}

/// Every pixel of `cel` that is exactly one of the `from` colours of `pairs` becomes its `to`, all at
/// once (a swap of red and blue swaps them). How many changed.
pub fn swap_colors(cel: &mut Canvas, pairs: &[(Rgba, Rgba)]) -> usize {
    let mut changed = 0;
    for y in 0..i64::from(cel.height()) {
        for x in 0..i64::from(cel.width()) {
            let Some(c) = cel.get(x, y) else { continue };
            if let Some((_, to)) = pairs.iter().find(|(from, _)| *from == c)
                && *to != c
            {
                cel.set(x, y, *to);
                changed += 1;
            }
        }
    }
    changed
}

/// Puts back every pixel of `cel` outside `mask` as it was in `before` (drawing stays inside the
/// selection). How many pixels differ from `before` afterwards.
pub fn clip_to(cel: &mut Canvas, before: &Canvas, mask: &Mask) -> usize {
    let mut changed = 0;
    for y in 0..i64::from(cel.height()) {
        for x in 0..i64::from(cel.width()) {
            let old = before.get(x, y);
            if !mask.get(x, y) {
                if let Some(o) = old {
                    cel.set(x, y, o);
                }
            } else if cel.get(x, y) != old {
                changed += 1;
            }
        }
    }
    changed
}

/// The selected pixels of `cel` (the rest transparent), cut to the selection's box: what Copy takes.
pub fn copy(cel: &Canvas, mask: Option<&Mask>) -> Option<Canvas> {
    let all;
    let mask = match mask {
        Some(m) => m,
        None => {
            all = Mask::all(cel.width(), cel.height())?;
            &all
        }
    };
    let (x0, y0, x1, y1) = mask.bounds()?;
    let mut out = Canvas::new((x1 - x0 + 1) as u32, (y1 - y0 + 1) as u32)?;
    for y in y0..=y1 {
        for x in x0..=x1 {
            if mask.get(x, y)
                && let Some(c) = cel.get(x, y)
            {
                out.set(x - x0, y - y0, c);
            }
        }
    }
    Some(out)
}

/// Puts `picture` on `cel` with its top left at `x`, `y` (transparent pixels cover nothing).
/// The selection it now covers.
pub fn paste(cel: &mut Canvas, picture: &Canvas, x: i64, y: i64) -> Option<Mask> {
    let mut m = Mask::new(cel.width(), cel.height())?;
    for py in 0..i64::from(picture.height()) {
        for px in 0..i64::from(picture.width()) {
            let (cx, cy) = (x.saturating_add(px), y.saturating_add(py));
            m.set(cx, cy, true);
            if let (Some(c), Some(under)) = (picture.get(px, py), cel.get(cx, cy))
                && c[3] > 0
            {
                cel.set(cx, cy, over_pixel(under, c, 255));
            }
        }
    }
    Some(m)
}

#[cfg(test)]
mod tests {
    use super::*;

    const R: Rgba = [255, 0, 0, 255];
    const B: Rgba = [0, 0, 255, 255];

    /// A canvas from rows: `r` red, `b` blue, `.` transparent.
    fn canvas(rows: &[&str]) -> Canvas {
        let mut c = Canvas::new(rows[0].len() as u32, rows.len() as u32).unwrap();
        for (y, row) in rows.iter().enumerate() {
            for (x, ch) in row.chars().enumerate() {
                let p = match ch {
                    'r' => R,
                    'b' => B,
                    _ => CLEAR,
                };
                c.set(x as i64, y as i64, p);
            }
        }
        c
    }

    fn rows(c: &Canvas) -> Vec<String> {
        (0..i64::from(c.height()))
            .map(|y| {
                (0..i64::from(c.width()))
                    .map(|x| match c.get(x, y) {
                        Some(R) => 'r',
                        Some(B) => 'b',
                        Some(p) if p[3] == 0 => '.',
                        _ => '?',
                    })
                    .collect()
            })
            .collect()
    }

    #[test]
    fn moving_a_selection_leaves_a_hole_and_returns_where_it_went() {
        let mut c = canvas(&["rb..", "....", "...."]);
        let m = Mask::rect(4, 3, (0, 0), (0, 0)).unwrap();
        let moved = apply(&mut c, Some(&m), Change::Move { dx: 2, dy: 1 }).unwrap();
        assert_eq!(rows(&c), [".b..", "..r.", "...."]);
        assert!(moved.get(2, 1) && moved.count() == 1);
    }

    #[test]
    fn flipping_and_turning_the_whole_cel() {
        let mut c = canvas(&["rb.", "..."]);
        apply(&mut c, None, Change::Flip { horizontal: true }).unwrap();
        assert_eq!(rows(&c), [".br", "..."]);
        apply(&mut c, None, Change::Flip { horizontal: false }).unwrap();
        assert_eq!(rows(&c), ["...", ".br"]);
        let mut sq = canvas(&["rb", ".."]);
        apply(&mut sq, None, Change::Rotate { clockwise: true }).unwrap();
        assert_eq!(rows(&sq), [".r", ".b"]);
        apply(&mut sq, None, Change::Rotate { clockwise: false }).unwrap();
        assert_eq!(rows(&sq), ["rb", ".."]);
    }

    #[test]
    fn turning_a_selection_keeps_its_middle() {
        // A 3×1 bar in the middle row turns into a 1×3 bar in the middle column.
        let mut c = canvas(&["...", "rbr", "..."]);
        let m = Mask::rect(3, 3, (0, 1), (2, 1)).unwrap();
        let turned = apply(&mut c, Some(&m), Change::Rotate { clockwise: true }).unwrap();
        assert_eq!(rows(&c), [".r.", ".b.", ".r."]);
        assert_eq!(turned.bounds(), Some((1, 0, 1, 2)));
    }

    #[test]
    fn outlines_go_outside_or_on_the_edge() {
        let mut out = canvas(&["...", ".r.", "..."]);
        assert_eq!(outline(&mut out, None, B, true), 4);
        assert_eq!(rows(&out), [".b.", "brb", ".b."]);
        let mut inside = canvas(&["rrr", "rrr", "rrr"]);
        assert_eq!(outline(&mut inside, None, B, false), 8, "past the cel's edge counts as transparent");
        let mut inside = canvas(&[".....", ".rrr.", ".rrr.", ".rrr.", "....."]);
        assert_eq!(outline(&mut inside, None, B, false), 8);
        assert_eq!(rows(&inside)[2], ".brb.");
    }

    #[test]
    fn replacing_a_colour_only_inside_the_selection() {
        let mut c = canvas(&["rrb"]);
        let m = Mask::rect(3, 1, (1, 0), (2, 0)).unwrap();
        assert_eq!(replace_color(&mut c, Some(&m), R, B, 0), 1);
        assert_eq!(rows(&c), ["rbb"]);
        assert_eq!(replace_color(&mut c, None, CLEAR, R, 0), 0, "nothing transparent to replace");
    }

    #[test]
    fn copy_cuts_to_the_box_and_paste_puts_it_back_without_covering_with_holes() {
        let c = canvas(&["....", ".rb.", ".r..", "...."]);
        let m = Mask::rect(4, 4, (1, 1), (2, 2)).unwrap();
        let piece = copy(&c, Some(&m)).unwrap();
        assert_eq!(rows(&piece), ["rb", "r."]);
        let mut d = canvas(&["bbbb", "bbbb"]);
        let covered = paste(&mut d, &piece, 2, 0).unwrap();
        assert_eq!(rows(&d), ["bbrb", "bbrb"]);
        assert_eq!(covered.count(), 4);
    }
}

#[cfg(test)]
mod swap_tests {
    use super::*;

    #[test]
    fn a_palette_swap_changes_every_colour_at_once() {
        let (r, b, g) = ([255, 0, 0, 255], [0, 0, 255, 255], [0, 255, 0, 255]);
        let mut c = Canvas::from_pixels(4, 1, vec![r, b, g, CLEAR]).unwrap();
        assert_eq!(swap_colors(&mut c, &[(r, b), (b, r)]), 2);
        assert_eq!(c.pixels(), [b, r, g, CLEAR]);
    }
}
