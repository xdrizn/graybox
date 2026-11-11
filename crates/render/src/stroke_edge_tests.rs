//! Graybox: a stroke the colour of what lies behind a shape must hide the fill's edge. Both edges
//! sit on the same curve for an inside stroke, so the fill's soft edge let a thin light line
//! through the stroke's soft edge.

use crate::paint_behaviour_tests::page;
use crate::{View, render_nodes_into};
use figcraft_color::{Color, Paint};
use figcraft_doc::{NodeKind, StrokeAlign};

const BG: u8 = 0x1E;
const WEIGHT: f64 = 8.0;
/// Edges that fall between pixels.
const BOX: (f64, f64, f64, f64) = (20.3, 20.7, 80.4, 60.6);
const CIRCLE: (f64, f64, f64, f64) = (20.3, 20.7, 70.4, 70.4);

/// Signed distance (negative inside) from `(px, py)` to a box with corner radius `r`.
fn box_sdf((x, y, w, h): (f64, f64, f64, f64), r: f64, px: f64, py: f64) -> f64 {
    let (qx, qy) = ((px - (x + w / 2.0)).abs() - (w / 2.0 - r), (py - (y + h / 2.0)).abs() - (h / 2.0 - r));
    qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - r
}

fn circle_sdf((x, y, w, _): (f64, f64, f64, f64), px: f64, py: f64) -> f64 {
    (px - (x + w / 2.0)).hypot(py - (y + w / 2.0)) - w / 2.0
}

fn render_shape(kind: NodeKind, (x, y, w, h): (f64, f64, f64, f64), radius: f64, align: StrokeAlign, stroke: Paint) -> tiny_skia::Pixmap {
    let (mut d, p) = page();
    let n = d.create(p, kind, "S", x, y, w, h).unwrap();
    {
        let n = d.get_mut(n).unwrap();
        n.fills = vec![Paint::solid(Color::from_hex("#D9D9D9").unwrap())];
        n.strokes = vec![stroke];
        n.stroke.weight = WEIGHT;
        n.stroke.align = align;
        n.corners.radii = [radius; 4];
    }
    let mut pix = tiny_skia::Pixmap::new(140, 120).unwrap();
    pix.fill(tiny_skia::Color::from_rgba8(BG, BG, BG, 255));
    render_nodes_into(&d, &[n], &View { origin: (0.0, 0.0), scale: 1.0, width: 140, height: 120 }, &mut pix);
    pix
}

/// The lightest red channel among the pixels past the stroke's inner edge (by a pixel), outwards.
fn lightest_in_stroke(pix: &tiny_skia::Pixmap, align: StrokeAlign, sdf: impl Fn(f64, f64) -> f64) -> u8 {
    let inner = match align {
        StrokeAlign::Inside => -WEIGHT,
        StrokeAlign::Center => -WEIGHT / 2.0,
        StrokeAlign::Outside => 0.0,
    };
    let mut lightest = 0;
    for y in 0..120 {
        for x in 0..140 {
            if sdf(f64::from(x) + 0.5, f64::from(y) + 0.5) > inner + 1.0
                && let Some(c) = pix.pixel(x, y)
            {
                lightest = lightest.max(c.red());
            }
        }
    }
    lightest
}

fn bg_stroke() -> Paint {
    Paint::solid(Color::from_hex("#1E1E1E").unwrap())
}

fn ellipse() -> NodeKind {
    NodeKind::Ellipse { start: 0.0, sweep: std::f64::consts::TAU, inner: 0.0 }
}

fn no_light_line(align: StrokeAlign, kind: NodeKind, bounds: (f64, f64, f64, f64), radius: f64) {
    let pix = render_shape(kind.clone(), bounds, radius, align, bg_stroke());
    let red = if matches!(kind, NodeKind::Ellipse { .. }) {
        lightest_in_stroke(&pix, align, |x, y| circle_sdf(bounds, x, y))
    } else {
        lightest_in_stroke(&pix, align, |x, y| box_sdf(bounds, radius, x, y))
    };
    assert!(red <= BG + 4, "{align:?}: a light line of {red} shows through the stroke");
}

#[test]
fn an_inside_stroke_hides_the_fill_edge() {
    no_light_line(StrokeAlign::Inside, ellipse(), CIRCLE, 0.0);
    no_light_line(StrokeAlign::Inside, NodeKind::Rectangle, BOX, 0.0);
    no_light_line(StrokeAlign::Inside, NodeKind::Rectangle, BOX, 14.0);
}

#[test]
fn a_center_stroke_hides_the_fill_edge() {
    no_light_line(StrokeAlign::Center, ellipse(), CIRCLE, 0.0);
    no_light_line(StrokeAlign::Center, NodeKind::Rectangle, BOX, 0.0);
    no_light_line(StrokeAlign::Center, NodeKind::Rectangle, BOX, 14.0);
}

#[test]
fn an_outside_stroke_hides_the_fill_edge() {
    no_light_line(StrokeAlign::Outside, ellipse(), CIRCLE, 0.0);
    no_light_line(StrokeAlign::Outside, NodeKind::Rectangle, BOX, 0.0);
    no_light_line(StrokeAlign::Outside, NodeKind::Rectangle, BOX, 14.0);
}

/// The fill still reaches the stroke's inner edge: no gap shows between them, and the middle of
/// the shape keeps the fill.
#[test]
fn the_fill_still_meets_the_inside_stroke_without_a_seam() {
    let pix = render_shape(NodeKind::Rectangle, BOX, 0.0, StrokeAlign::Inside, Paint::solid(Color::from_hex("#FF0000").unwrap()));
    // Along the middle row, past the soft outer edge: the stroke, then straight to the fill.
    let row: Vec<u8> = (21..40).filter_map(|x| pix.pixel(x, 50)).map(|c| c.green()).collect();
    // Red stroke has no green; the fill #D9D9D9 does: green only ever goes up (soft step, no dip).
    assert!(row.windows(2).all(|w| w[1] + 2 >= w[0]), "a dark seam between stroke and fill: {row:?}");
    assert_eq!(pix.pixel(60, 50).map(|c| c.green()), Some(0xD9));
}

/// A translucent stroke lets the fill show through, so the fill keeps its full outline.
#[test]
fn a_translucent_inside_stroke_keeps_the_full_fill() {
    let mut paint = Paint::solid(Color::from_hex("#1E1E1E").unwrap());
    paint.opacity = 0.5;
    let pix = render_shape(NodeKind::Rectangle, BOX, 0.0, StrokeAlign::Inside, paint);
    // Inside the stroke band (x = 22) the fill is half visible under the stroke: lighter than bg.
    let c = pix.pixel(22, 50).unwrap();
    assert!(c.red() > 100, "{c:?}");
}

/// Dashed, per-side, translucent, blended and gradient strokes leave gaps or let the fill show:
/// the fill keeps its own outline there.
#[test]
fn only_a_solid_opaque_inside_stroke_shrinks_the_fill() {
    let (mut d, p) = page();
    let id = d.create(p, NodeKind::Rectangle, "R", 0.0, 0.0, 80.0, 60.0).unwrap();
    {
        let n = d.get_mut(id).unwrap();
        n.strokes = vec![bg_stroke()];
        n.stroke.weight = 8.0;
        n.stroke.align = StrokeAlign::Inside;
    }
    let shrinks = |d: &figcraft_doc::Document| crate::covered::fill_under_stroke(d.node(id).unwrap(), 1.0).is_some();
    assert!(shrinks(&d));
    d.get_mut(id).unwrap().stroke.dashes = vec![4.0, 4.0];
    assert!(!shrinks(&d), "dashed");
    d.get_mut(id).unwrap().stroke.dashes.clear();
    d.get_mut(id).unwrap().stroke.sides = Some([8.0, 0.0, 8.0, 0.0]);
    assert!(!shrinks(&d), "per side");
    d.get_mut(id).unwrap().stroke.sides = None;
    d.get_mut(id).unwrap().strokes[0].opacity = 0.5;
    assert!(!shrinks(&d), "translucent");
    d.get_mut(id).unwrap().strokes[0].opacity = 1.0;
    d.get_mut(id).unwrap().strokes[0].blend = figcraft_color::BlendMode::Multiply;
    assert!(!shrinks(&d), "blended");
    d.get_mut(id).unwrap().strokes[0].blend = figcraft_color::BlendMode::Normal;
    d.get_mut(id).unwrap().stroke.align = StrokeAlign::Center;
    assert!(!shrinks(&d), "centre");
}
