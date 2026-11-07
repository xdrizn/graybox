//! Graybox corner styles: each style's outline, the offsets per-side strokes use, the file field
//! and hit testing on the new outlines.

use crate::corner_style::{offset_outline, outline, side_strokes};
use crate::{CornerStyle, Corners, Document, NodeKind, StrokeAlign};
use figcraft_geom::kurbo::{BezPath, Point, Shape};

const W: f64 = 100.0;
const H: f64 = 60.0;

fn shape(style: CornerStyle, r: f64) -> BezPath {
    outline(W, H, [r; 4], 0.0, style)
}

fn inside(p: &BezPath, x: f64, y: f64) -> bool {
    p.contains(Point::new(x, y))
}

fn area(p: &BezPath) -> f64 {
    p.area().abs()
}

fn close(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

#[test]
fn round_is_upstreams_rounded_rectangle() {
    let ours = outline(W, H, [12.0, 4.0, 0.0, 20.0], 0.6, CornerStyle::Round);
    assert_eq!(ours, figcraft_geom::rounded_rect(W, H, [12.0, 4.0, 0.0, 20.0], 0.6));
}

#[test]
fn chamfer_cuts_each_corner_with_a_straight_diagonal() {
    let p = shape(CornerStyle::Chamfer, 20.0);
    assert!(close(area(&p), W * H - 4.0 * 200.0, 1e-6), "four 20 x 20 half squares cut off");
    for (x, y) in [(9.0, 9.0), (91.0, 9.0), (91.0, 51.0), (9.0, 51.0)] {
        assert!(!inside(&p, x, y), "({x}, {y}) is cut off");
    }
    for (x, y) in [(11.0, 11.0), (89.0, 11.0), (89.0, 49.0), (11.0, 49.0), (50.0, 30.0)] {
        assert!(inside(&p, x, y), "({x}, {y}) stays");
    }
}

#[test]
fn scoop_takes_a_quarter_circle_around_each_corner() {
    let p = shape(CornerStyle::Scoop, 20.0);
    let expected = W * H - std::f64::consts::PI * 400.0;
    assert!(close(area(&p), expected, 0.5), "{} vs {expected}", area(&p));
    // Inside the circle round the corner is cut; a 45° point past it stays.
    assert!(!inside(&p, 13.0, 13.0));
    assert!(inside(&p, 15.0, 15.0));
    assert!(!inside(&p, 87.0, 47.0));
    assert!(inside(&p, 85.0, 45.0));
    // Unlike a round corner, the middle of the corner square is cut.
    assert!(inside(&outline(W, H, [20.0; 4], 0.0, CornerStyle::Round), 10.0, 10.0));
}

#[test]
fn notch_cuts_a_square_step() {
    let p = shape(CornerStyle::Notch, 20.0);
    assert!(close(area(&p), W * H - 4.0 * 400.0, 1e-6));
    assert!(!inside(&p, 19.0, 19.0));
    assert!(inside(&p, 21.0, 5.0));
    assert!(inside(&p, 5.0, 21.0));
    assert!(!inside(&p, 81.0, 41.0));
    assert!(inside(&p, 79.0, 55.0));
}

#[test]
fn v_notch_cuts_a_v_towards_the_middle() {
    let p = shape(CornerStyle::VNotch, 20.0);
    // The kite (0,0) (10,0) (20,20) (0,10) is 200 square px.
    assert!(close(area(&p), W * H - 4.0 * 200.0, 1e-6));
    assert!(!inside(&p, 3.0, 3.0));
    assert!(!inside(&p, 18.0, 18.0), "the V reaches the full size along the diagonal");
    assert!(inside(&p, 18.0, 5.0), "beside the V");
    assert!(inside(&p, 5.0, 18.0));
    assert!(inside(&p, 12.0, 1.0) && !inside(&p, 8.0, 1.0), "it opens half the size along the edge");
}

#[test]
fn per_corner_sizes_and_clamping_work_for_every_style() {
    for style in CornerStyle::ALL {
        let p = outline(W, H, [20.0, 0.0, 0.0, 0.0], 0.0, style);
        assert!(inside(&p, 99.0, 1.0) && inside(&p, 99.0, 59.0) && inside(&p, 1.0, 59.0), "{style:?}: only the top left is cut");
        assert!(!inside(&p, 1.0, 1.0), "{style:?}");
        // Bigger than half the shorter side: clamped to 30 like round corners.
        assert_eq!(outline(W, H, [1000.0; 4], 0.0, style), outline(W, H, [30.0; 4], 0.0, style), "{style:?}");
        // Junk never panics and draws something sane.
        for junk in [f64::NAN, f64::INFINITY, -5.0] {
            let p = outline(W, H, [junk; 4], 0.0, style);
            assert!(close(area(&p), W * H, 1e-6), "{style:?} with {junk}");
        }
        assert!(outline(0.0, 0.0, [10.0; 4], 0.0, style).elements().is_empty());
        assert!(outline(f64::NAN, -1.0, [10.0; 4], 0.0, style).elements().is_empty());
    }
}

#[test]
fn zero_size_corners_are_square_in_every_style() {
    for style in CornerStyle::ALL {
        assert!(close(area(&outline(W, H, [0.0; 4], 0.0, style)), W * H, 1e-9), "{style:?}");
    }
}

#[test]
fn insets_move_cuts_parallel_to_themselves() {
    let d = 4.0;
    // Chamfer: the cut moves d along its normal, so its line is x + y = 20 + d√2.
    let p = offset_outline(W, H, [20.0; 4], CornerStyle::Chamfer, [d; 4]);
    let c = 20.0 + d * std::f64::consts::SQRT_2;
    assert!(!inside(&p, c / 2.0 - 0.1, c / 2.0 - 0.1) && inside(&p, c / 2.0 + 0.1, c / 2.0 + 0.1));
    // Notch: the step moves d too.
    let p = offset_outline(W, H, [20.0; 4], CornerStyle::Notch, [d; 4]);
    assert!(!inside(&p, 23.0, 10.0) && inside(&p, 25.0, 10.0));
    // Scoop: the circle keeps its centre on the box corner and grows by d.
    let p = offset_outline(W, H, [20.0; 4], CornerStyle::Scoop, [d; 4]);
    assert!(!inside(&p, 16.5, 16.5) && inside(&p, 17.5, 17.5));
    // Growing outward: straight cuts keep their size (mitred, like a stroke's outer edge), a scoop
    // shrinks around the box corner until it closes to a square corner.
    let grown = |style| offset_outline(W, H, [5.0; 4], style, [-40.0; 4]);
    assert!(inside(&grown(CornerStyle::Scoop), -39.0, -39.0));
    let p = grown(CornerStyle::Chamfer);
    assert!(!inside(&p, -39.0, -39.0) && inside(&p, -20.0, -30.0));
    let p = grown(CornerStyle::Notch);
    assert!(!inside(&p, -37.0, -37.0) && inside(&p, -30.0, -38.0));
    let p = offset_outline(W, H, [20.0; 4], CornerStyle::Scoop, [-3.0; 4]);
    assert!(!inside(&p, 10.0, 10.0) && inside(&p, -2.0, 30.0), "a scoop narrower by 3, the sides out by 3");
    for style in CornerStyle::ALL {
        for d in [-1e6, -40.0, -10.0, 10.0, 25.0, f64::NAN] {
            let p = offset_outline(W, H, [20.0, 5.0, 0.0, 30.0], style, [d; 4]);
            assert!(p.area().is_finite(), "{style:?} {d}");
        }
    }
}

#[test]
fn side_strokes_follow_styled_corners() {
    for style in [CornerStyle::Chamfer, CornerStyle::Scoop, CornerStyle::Notch, CornerStyle::VNotch] {
        let ring = side_strokes(W, H, [20.0; 4], style, [4.0, 2.0, 8.0, 4.0], StrokeAlign::Inside);
        assert!(inside(&ring, 2.0, 30.0) && inside(&ring, 50.0, 3.0) && inside(&ring, 50.0, 55.0), "{style:?}: each side");
        assert!(!inside(&ring, 50.0, 30.0), "{style:?}: hollow");
        assert!(!inside(&ring, 50.0, 5.0), "{style:?}: the top is only 4 thick");
        assert!(!inside(&ring, 1.0, 1.0), "{style:?}: nothing at the cut corner");
        let ring = side_strokes(W, H, [20.0; 4], style, [4.0; 4], StrokeAlign::Outside);
        assert!(inside(&ring, -2.0, 30.0) && !inside(&ring, 2.0, 30.0), "{style:?}: outside");
    }
    // Along a notch's step the stroke runs too.
    let ring = side_strokes(W, H, [20.0; 4], CornerStyle::Notch, [4.0; 4], StrokeAlign::Inside);
    assert!(inside(&ring, 22.0, 10.0) && !inside(&ring, 26.0, 10.0));
    assert!(side_strokes(W, H, [20.0; 4], CornerStyle::Notch, [0.0; 4], StrokeAlign::Inside).elements().is_empty());
}

#[test]
fn files_without_a_style_load_round_and_round_is_not_written() {
    let c: Corners = serde_json::from_str(r#"{"radii":[4,4,4,4],"smoothing":0}"#).unwrap();
    assert_eq!(c.style, CornerStyle::Round);
    assert!(!serde_json::to_string(&c).unwrap().contains("style"));
    let c = Corners { style: CornerStyle::VNotch, ..c };
    let s = serde_json::to_string(&c).unwrap();
    assert!(s.contains(r#""style":"vNotch""#), "{s}");
    assert_eq!(serde_json::from_str::<Corners>(&s).unwrap(), c);
}

#[test]
fn styles_parse_by_id_and_common_names() {
    for s in CornerStyle::ALL {
        assert_eq!(CornerStyle::parse(s.id()), Some(s));
        assert_eq!(CornerStyle::parse(s.label()), Some(s));
    }
    assert_eq!(CornerStyle::parse("Bevel"), Some(CornerStyle::Chamfer));
    assert_eq!(CornerStyle::parse("inverse-round"), Some(CornerStyle::Scoop));
    assert_eq!(CornerStyle::parse("inset"), Some(CornerStyle::Notch));
    assert_eq!(CornerStyle::parse("inverted chamfer"), Some(CornerStyle::VNotch));
    assert_eq!(CornerStyle::parse("wobbly"), None);
}

#[test]
fn hit_testing_follows_the_cut_corner() {
    let mut d = Document::new("x");
    let page = d.first_page().unwrap();
    let r = d.create(page, NodeKind::Rectangle, "R", 0.0, 0.0, 100.0, 60.0).unwrap();
    {
        let n = d.get_mut(r).unwrap();
        n.corners.radii = [20.0; 4];
        n.corners.style = CornerStyle::Notch;
    }
    assert_eq!(d.hit_test(page, Point::new(25.0, 5.0), 0.0, None), Some(r));
    assert_ne!(d.hit_test(page, Point::new(10.0, 10.0), 0.0, None), Some(r), "the notch is empty");
    let path = crate::geometry::shape_path(d.node(r).unwrap()).unwrap();
    assert!(close(area(&path), W * H - 1600.0, 1e-6));
}
