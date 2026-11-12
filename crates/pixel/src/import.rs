//! Graybox pixel art import (G16.5): a PNG, or a sprite sheet cut by a grid, into frames.

use crate::canvas::{self, Canvas};
use crate::sprite::MAX_FRAMES;

/// The longest side of a sheet that is read.
pub const MAX_SHEET_SIDE: u32 = 8192;

/// The frames in PNG `bytes`: the whole picture, or pieces `cell` (width, height) big, left to
/// right then top to bottom. Pieces cut off at the right and bottom edges are left out, and so are
/// empty pieces after the last one with pixels (one frame always stays).
pub fn frames(bytes: &[u8], cell: Option<(u32, u32)>) -> Result<Vec<Canvas>, String> {
    let (w, h, px) = canvas::decode_png(bytes, MAX_SHEET_SIDE)
        .ok_or_else(|| format!("that isn't a PNG Graybox can read ({MAX_SHEET_SIDE} pixels on a side at most)"))?;
    let (cw, ch) = cell.unwrap_or((w, h));
    if cw == 0 || ch == 0 || cw > Canvas::MAX_SIDE || ch > Canvas::MAX_SIDE {
        return Err(format!("a frame is {cw}×{ch}: frames are 1 to {} pixels on a side", Canvas::MAX_SIDE));
    }
    let (cols, rows) = (w / cw, h / ch);
    if cols == 0 || rows == 0 {
        return Err(format!("the picture is {w}×{h}, smaller than one {cw}×{ch} frame"));
    }
    let mut out = Vec::new();
    let mut last_painted = 0;
    'cut: for r in 0..rows {
        for c in 0..cols {
            if out.len() >= MAX_FRAMES {
                break 'cut;
            }
            let mut cel = Vec::with_capacity(cw as usize * ch as usize * 4);
            for y in 0..ch {
                let start = ((r * ch + y) as usize * w as usize + (c * cw) as usize) * 4;
                cel.extend_from_slice(px.get(start..start + cw as usize * 4).ok_or("the picture is shorter than it says")?);
            }
            if cel.chunks_exact(4).any(|p| p[3] > 0) {
                last_painted = out.len();
            }
            out.push(Canvas::from_bytes(cw, ch, &cel).ok_or("a frame couldn't be made")?);
        }
    }
    out.truncate(last_painted + 1);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED: [u8; 4] = [200, 30, 40, 255];

    /// A `w` × `h` PNG with red dots at `dots`.
    fn png(w: u32, h: u32, dots: &[(u32, u32)]) -> Vec<u8> {
        let mut px = vec![0; (w * h * 4) as usize];
        for (x, y) in dots {
            let i = ((y * w + x) * 4) as usize;
            px[i..i + 4].copy_from_slice(&RED);
        }
        canvas::encode_png(w, h, &px).unwrap()
    }

    #[test]
    fn a_sheet_is_cut_left_to_right_then_down_and_trailing_empty_cells_are_dropped() {
        // 3 × 2 cells of 4 × 4 (and 2 spare pixels on the right); cells 0, 1 and 3 have a dot.
        let sheet = png(14, 8, &[(0, 0), (5, 1), (2, 6)]);
        let frames = frames(&sheet, Some((4, 4))).unwrap();
        assert_eq!(frames.len(), 4, "cells 4 and 5 are empty and come after the last dot");
        assert_eq!(frames[1].get(1, 1), Some(RED));
        assert_eq!(frames[2].get(0, 0), Some([0; 4]), "an empty cell between keeps its place");
        assert_eq!(frames[3].get(2, 2), Some(RED));
    }

    #[test]
    fn a_plain_png_is_one_frame_and_bad_input_is_refused() {
        let one = frames(&png(5, 3, &[]), None).unwrap();
        assert_eq!((one.len(), one[0].width(), one[0].height()), (1, 5, 3), "even an empty picture is one frame");
        assert!(frames(b"not a png", None).unwrap_err().contains("PNG"));
        assert!(frames(&png(4, 4, &[]), Some((8, 8))).unwrap_err().contains("smaller"));
        assert!(frames(&png(4, 4, &[]), Some((0, 4))).unwrap_err().contains("frame"));
        assert!(frames(&png(2000, 1, &[]), None).unwrap_err().contains("1024"), "one frame is at most 1024 wide");
    }
}
