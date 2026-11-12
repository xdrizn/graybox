//! Which art pixels a line, a rectangle or an ellipse covers. Points are art-pixel coordinates
//! (x right, y down); every shape includes both of its end or corner points.

/// An art-pixel position.
pub type Pt = (i64, i64);

/// The longest path a shape or stroke may have (points); the rest is left out.
pub const MAX_POINTS: usize = 1 << 20;

/// The pixels of a straight line from `a` to `b`, both included, without gaps (each pixel touches
/// the next by a side or a corner).
pub fn line(a: Pt, b: Pt) -> Vec<Pt> {
    let (mut x, mut y) = a;
    let (dx, dy) = ((b.0 - a.0).abs(), -(b.1 - a.1).abs());
    let (sx, sy) = (if a.0 < b.0 { 1 } else { -1 }, if a.1 < b.1 { 1 } else { -1 });
    let mut err = dx + dy;
    let mut out = Vec::with_capacity((dx.max(-dy) as usize + 1).min(MAX_POINTS));
    loop {
        out.push((x, y));
        if (x, y) == b || out.len() >= MAX_POINTS {
            return out;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

/// The pixels of lines through `points` in turn (a freehand stroke): no pixel twice in a row.
pub fn polyline(points: &[Pt]) -> Vec<Pt> {
    let mut out: Vec<Pt> = Vec::new();
    match points {
        [] => {}
        [p] => out.push(*p),
        _ => {
            for w in points.windows(2) {
                if let [a, b] = w {
                    for p in line(*a, *b) {
                        if out.last() != Some(&p) {
                            out.push(p);
                        }
                        if out.len() >= MAX_POINTS {
                            return out;
                        }
                    }
                }
            }
        }
    }
    out
}

/// A one-pixel path with its L-shaped corners taken out, so a freehand line stays one pixel thin
/// ("pixel-perfect"): a pixel whose neighbours before and after touch each other diagonally goes.
pub fn pixel_perfect(path: &[Pt]) -> Vec<Pt> {
    let mut out: Vec<Pt> = Vec::with_capacity(path.len());
    let mut i = 0;
    while i < path.len() {
        let Some(&cur) = path.get(i) else { break };
        if let (Some(&prev), Some(&next)) = (out.last(), path.get(i + 1)) {
            let touches_prev = prev.0 == cur.0 || prev.1 == cur.1;
            let touches_next = next.0 == cur.0 || next.1 == cur.1;
            if touches_prev && touches_next && prev.0 != next.0 && prev.1 != next.1 && (prev.0 - next.0).abs() <= 1 && (prev.1 - next.1).abs() <= 1 {
                i += 1;
                continue;
            }
        }
        out.push(cur);
        i += 1;
    }
    out
}

/// The corner `b` moved so the box from `a` is square (the larger side wins), for Shift.
pub fn squared(a: Pt, b: Pt) -> Pt {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let side = dx.abs().max(dy.abs());
    let sign = |d: i64| if d < 0 { -1 } else { 1 };
    (a.0 + side * sign(dx), a.1 + side * sign(dy))
}

fn ordered(a: Pt, b: Pt) -> (i64, i64, i64, i64) {
    (a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1))
}

/// Whether a box from `a` to `b` has more pixels than a stroke may cover.
fn too_big(x0: i64, y0: i64, x1: i64, y1: i64) -> bool {
    let (w, h) = ((x1 - x0 + 1) as u128, (y1 - y0 + 1) as u128);
    w * h > MAX_POINTS as u128
}

/// The outline of the box with corners `a` and `b` (one pixel wide).
pub fn rect_outline(a: Pt, b: Pt) -> Vec<Pt> {
    let (x0, y0, x1, y1) = ordered(a, b);
    let mut out = Vec::new();
    if (x1 - x0) as u128 + (y1 - y0) as u128 > MAX_POINTS as u128 / 2 {
        return out;
    }
    for x in x0..=x1 {
        out.push((x, y0));
        if y1 != y0 {
            out.push((x, y1));
        }
    }
    for y in y0 + 1..y1 {
        out.push((x0, y));
        if x1 != x0 {
            out.push((x1, y));
        }
    }
    out
}

/// Every pixel of the box with corners `a` and `b`.
pub fn rect_filled(a: Pt, b: Pt) -> Vec<Pt> {
    let (x0, y0, x1, y1) = ordered(a, b);
    if too_big(x0, y0, x1, y1) {
        return Vec::new();
    }
    (y0..=y1).flat_map(|y| (x0..=x1).map(move |x| (x, y))).collect()
}

/// The outline of the ellipse that fits the box with corners `a` and `b`, symmetric for even and
/// odd sizes (the rasterising algorithm for ellipses in a rectangle from A. Zingl's paper "A
/// Rasterizing Algorithm for Drawing Curves", 2012).
pub fn ellipse_outline(a: Pt, b: Pt) -> Vec<Pt> {
    let (mut x0, mut y0, mut x1, y1) = ordered(a, b);
    let mut out = Vec::new();
    if too_big(x0, y0, x1, y1) {
        return out;
    }
    let (aa, bb) = (x1 - x0, y1 - y0);
    let b1 = bb & 1;
    let mut dx = 4 * (1 - aa) * bb * bb;
    let mut dy = 4 * (b1 + 1) * aa * aa;
    let mut err = dx + dy + b1 * aa * aa;
    y0 += (bb + 1) / 2;
    let mut y1 = y0 - b1;
    let (a8, b8) = (8 * aa * aa, 8 * bb * bb);
    loop {
        out.extend([(x1, y0), (x0, y0), (x0, y1), (x1, y1)]);
        let e2 = 2 * err;
        if e2 <= dy {
            y0 += 1;
            y1 -= 1;
            dy += a8;
            err += dy;
        }
        if e2 >= dx || 2 * err > dy {
            x0 += 1;
            x1 -= 1;
            dx += b8;
            err += dx;
        }
        if x0 > x1 || out.len() >= MAX_POINTS {
            break;
        }
    }
    // A flat ellipse (one pixel wide) stops early: finish its tips.
    while y0 - y1 <= bb && out.len() < MAX_POINTS {
        out.extend([(x0 - 1, y0), (x1 + 1, y0), (x0 - 1, y1), (x1 + 1, y1)]);
        y0 += 1;
        y1 -= 1;
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// Every pixel inside the ellipse that fits the box with corners `a` and `b`, its outline included.
pub fn ellipse_filled(a: Pt, b: Pt) -> Vec<Pt> {
    let outline = ellipse_outline(a, b);
    let mut rows: std::collections::BTreeMap<i64, (i64, i64)> = std::collections::BTreeMap::new();
    for (x, y) in outline {
        let e = rows.entry(y).or_insert((x, x));
        e.0 = e.0.min(x);
        e.1 = e.1.max(x);
    }
    rows.into_iter().flat_map(|(y, (x0, x1))| (x0..=x1).map(move |x| (x, y))).take(MAX_POINTS).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picture(points: &[Pt], w: i64, h: i64) -> Vec<String> {
        (0..h).map(|y| (0..w).map(|x| if points.contains(&(x, y)) { '#' } else { '.' }).collect()).collect()
    }

    #[test]
    fn a_line_has_no_gaps_and_both_ends() {
        assert_eq!(line((0, 0), (3, 1)), vec![(0, 0), (1, 0), (2, 1), (3, 1)]);
        assert_eq!(line((2, 2), (2, 2)), vec![(2, 2)]);
        assert_eq!(line((3, 0), (0, 0)).len(), 4);
        let l = line((0, 0), (5, 9));
        for w in l.windows(2) {
            assert!((w[0].0 - w[1].0).abs() <= 1 && (w[0].1 - w[1].1).abs() <= 1);
        }
    }

    #[test]
    fn pixel_perfect_takes_out_l_corners() {
        // A stair drawn freehand: an L at each step.
        let path = vec![(0, 0), (1, 0), (1, 1), (2, 1), (2, 2)];
        assert_eq!(pixel_perfect(&path), vec![(0, 0), (1, 1), (2, 2)]);
        let straight = vec![(0, 0), (1, 0), (2, 0)];
        assert_eq!(pixel_perfect(&straight), straight);
    }

    #[test]
    fn a_polyline_joins_its_points() {
        assert_eq!(polyline(&[(0, 0), (2, 0), (2, 2)]), vec![(0, 0), (1, 0), (2, 0), (2, 1), (2, 2)]);
        assert_eq!(polyline(&[(4, 4)]), vec![(4, 4)]);
    }

    #[test]
    fn rectangles_outline_and_fill() {
        assert_eq!(picture(&rect_outline((0, 0), (3, 2)), 4, 3), ["####", "#..#", "####"]);
        assert_eq!(rect_filled((1, 1), (0, 0)).len(), 4);
        assert_eq!(rect_outline((0, 0), (0, 0)), vec![(0, 0)]);
    }

    #[test]
    fn an_ellipse_fits_its_box_and_is_symmetric() {
        assert_eq!(picture(&ellipse_outline((0, 0), (5, 5)), 6, 6), ["..##..", ".#..#.", "#....#", "#....#", ".#..#.", "..##.."]);
        assert_eq!(picture(&ellipse_filled((0, 0), (4, 2)), 5, 3), [".###.", "#####", ".###."]);
        assert_eq!(picture(&ellipse_outline((0, 0), (0, 3)), 1, 4), ["#", "#", "#", "#"], "a one-pixel-wide ellipse is a line");
    }

    #[test]
    fn shift_makes_the_box_square() {
        assert_eq!(squared((0, 0), (5, 2)), (5, 5));
        assert_eq!(squared((5, 5), (3, 0)), (0, 0));
    }

    #[test]
    fn huge_shapes_are_left_empty_instead_of_running_out_of_memory() {
        assert!(rect_filled((0, 0), (1 << 30, 1 << 30)).is_empty());
        assert!(ellipse_outline((0, 0), (i64::from(i32::MAX), 5)).is_empty());
    }
}
