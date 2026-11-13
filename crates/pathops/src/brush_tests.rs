//! Brushes: stretch shapes bend along the path, scatter copies are spaced, capped and repeatable,
//! and a width profile (pen pressure) scales them.

use figcraft_doc::{Brush, StrokeAlign, StrokeProps, WidthProfile};
use figcraft_geom::kurbo::{BezPath, Point, Shape};

use crate::brush::{MAX_COPIES, PRESETS, outline};

fn line(len: f64) -> BezPath {
    let mut p = BezPath::new();
    p.move_to((0.0, 0.0));
    p.line_to((len, 0.0));
    p
}

fn brush(preset: &str) -> Brush {
    Brush { preset: preset.into(), ..Default::default() }
}

fn props(weight: f64, b: &Brush) -> StrokeProps {
    StrokeProps { weight, align: StrokeAlign::Center, brush: Some(b.clone()), ..Default::default() }
}

/// What a brush paints stays within half [`StrokeProps::max_weight`] of the path, which the
/// stroke's bounds go by: plain, with copies grown, turned and strayed all they can, and with a
/// width profile thinning or widening it (strays keep their distance, whatever the profile).
#[test]
fn every_preset_draws_inside_its_reach() {
    for p in PRESETS {
        let wild = Brush { size_jitter: 1.0, rotation_jitter: 1.0, scatter: 4.0, ..brush(p.id) };
        for (b, profile) in [(brush(p.id), None), (wild.clone(), None), (wild.clone(), Some(0.2)), (wild, Some(2.0))] {
            let k = StrokeProps { width_profile: profile.map(|s| WidthProfile::new([(0.0, s), (1.0, s)])), ..props(10.0, &b) };
            let out = outline(&line(200.0), 10.0, &k, &b).unwrap();
            let bb = out.bounding_box();
            let reach = k.max_weight() / 2.0;
            assert!(!out.elements().is_empty(), "{}", p.id);
            assert!(
                bb.y0 >= -reach && bb.y1 <= reach && bb.x0 >= -reach && bb.x1 <= 200.0 + reach,
                "{} {b:?} {profile:?}: {bb:?}, reach {reach}",
                p.id
            );
        }
    }
    assert!(outline(&line(10.0), 1.0, &props(1.0, &brush("nope")), &brush("nope")).is_none());
}

#[test]
fn stretch_bends_with_the_path() {
    // Down a vertical line, the taper's widest point sits halfway down, across x.
    let mut p = BezPath::new();
    p.move_to((0.0, 0.0));
    p.line_to((0.0, 100.0));
    let b = brush("taper");
    let out = outline(&p, 20.0, &props(20.0, &b), &b).unwrap();
    let bb = out.bounding_box();
    assert!((bb.height() - 100.0).abs() < 1.0 && (bb.width() - 15.0).abs() < 1.5, "{bb:?}");
    assert_ne!(out.winding(Point::new(0.0, 50.0)), 0, "filled in the middle");
    assert_eq!(out.winding(Point::new(6.0, 2.0)), 0, "pointed at the start");
}

#[test]
fn scatter_is_spaced_capped_and_repeatable() {
    let b = Brush { spacing: 2.0, size_jitter: 0.5, rotation_jitter: 1.0, scatter: 1.0, seed: 7, ..brush("dots") };
    let a = outline(&line(200.0), 10.0, &props(10.0, &b), &b).unwrap();
    assert_eq!(a, outline(&line(200.0), 10.0, &props(10.0, &b), &b).unwrap(), "the same seed, the same copies");
    let c = Brush { seed: 8, ..b.clone() };
    assert_ne!(a, outline(&line(200.0), 10.0, &props(10.0, &c), &c).unwrap());
    // 200 long, a copy every 20: ten circles (each a move-to).
    let moves = |p: &BezPath| p.elements().iter().filter(|e| matches!(e, figcraft_geom::kurbo::PathEl::MoveTo(_))).count();
    assert_eq!(moves(&a), 10);
    // A tiny spacing on a long path stops at the cap.
    let dense = Brush { spacing: 0.0, ..brush("dots") };
    assert!(moves(&outline(&line(1e6), 1.0, &props(1.0, &dense), &dense).unwrap()) <= MAX_COPIES + 1);
}

#[test]
fn a_width_profile_scales_the_brush() {
    let b = brush("dots");
    let mut pr = props(10.0, &b);
    pr.width_profile = Some(WidthProfile::new([(0.0, 0.2), (1.0, 2.0)]));
    let out = outline(&line(300.0), 10.0, &pr, &b).unwrap();
    let parts = figcraft_geom::split_subpaths(&out);
    let (first, last) = (parts.first().unwrap().bounding_box(), parts.last().unwrap().bounding_box());
    assert!(last.width() > first.width() * 3.0, "{first:?} {last:?}");
}

#[test]
fn an_unsanitized_profile_is_bounded_before_sampling() {
    // 100k points, all past the 64-point cap and out of range: the brush reads the clamped profile.
    let raw = WidthProfile { points: (0..100_000).map(|i| figcraft_doc::WidthPoint::new(f64::from(i), 1e300)).collect() };
    let b = brush("taper");
    let mut p = props(1.0, &b);
    p.width_profile = Some(raw);
    let out = outline(&line(50.0), 1.0, &p, &b).unwrap();
    assert!(out.bounding_box().height() <= 2.0 * WidthProfile::MAX_SCALE);
}

fn square(side: f64) -> BezPath {
    let mut p = BezPath::new();
    p.move_to((0.0, 0.0));
    p.line_to((side, 0.0));
    p.line_to((side, side));
    p.line_to((0.0, side));
    p.close_path();
    p
}

/// Graybox: Taper off: a stretch brush keeps its width up to both ends (no pressure thinning).
#[test]
fn taper_off_keeps_the_full_width_to_the_ends() {
    let on = brush("taper");
    let off = Brush { taper: false, ..brush("taper") };
    let a = outline(&line(200.0), 10.0, &props(10.0, &on), &on).unwrap();
    let b = outline(&line(200.0), 10.0, &props(10.0, &off), &off).unwrap();
    assert_eq!(a.winding(Point::new(2.0, 3.0)), 0, "tapered: thin at the start");
    assert_ne!(b.winding(Point::new(2.0, 3.0)), 0, "taper off: full width at the start");
    assert_ne!(b.winding(Point::new(198.0, -3.0)), 0, "and at the end");
}

/// Graybox: start and end amounts (parts of the length) set how long each taper is.
#[test]
fn start_and_end_amounts_set_the_taper_lengths() {
    let b = Brush { taper_start: Some(0.1), taper_end: Some(0.0), ..brush("taper") };
    let out = outline(&line(200.0), 10.0, &props(10.0, &b), &b).unwrap();
    assert_eq!(out.winding(Point::new(2.0, 3.0)), 0, "a short taper at the start");
    assert_ne!(out.winding(Point::new(30.0, 3.0)), 0, "full width past 10% of the length");
    assert_ne!(out.winding(Point::new(199.0, 3.0)), 0, "no taper at the end");
}

/// Graybox (owner's report): a rectangle with a brush stroke and Taper off has no thin corner
/// where its path starts and ends.
#[test]
fn a_closed_path_with_taper_off_has_no_thin_corner() {
    for id in ["taper", "ink", "brushpen", "marker"] {
        let b = Brush { taper: false, ..brush(id) };
        let out = outline(&square(100.0), 10.0, &props(10.0, &b), &b).unwrap();
        for p in [(3.0, 1.5), (1.5, 3.0), (50.0, 1.5), (98.5, 50.0), (50.0, 98.5), (1.5, 50.0), (1.5, 97.0)] {
            assert_ne!(out.winding(Point::new(p.0, p.1)), 0, "{id}: {p:?} covered");
        }
    }
}

/// Graybox: organic brushes made in code: dry brush, brush pen, chalk and marker.
#[test]
fn organic_brushes_are_stretch_brushes_repeatable_by_seed() {
    for id in ["dry", "brushpen", "chalk", "marker"] {
        let p = crate::brush::preset(id).unwrap_or_else(|| panic!("{id}"));
        assert_eq!(p.kind, crate::brush::BrushKind::Stretch, "{id}");
        let b = brush(id);
        let a = outline(&line(300.0), 12.0, &props(12.0, &b), &b).unwrap();
        assert_eq!(a, outline(&line(300.0), 12.0, &props(12.0, &b), &b).unwrap(), "{id}: repeatable");
        assert!(a.winding(Point::new(150.0, 0.0)) != 0 || a.winding(Point::new(150.0, 2.0)) != 0, "{id}: paints the middle");
    }
    // Dry brush breaks into strands with gaps; chalk has grain; a new seed, a new pattern.
    for id in ["dry", "chalk"] {
        let b = brush(id);
        let c = Brush { seed: 9, ..brush(id) };
        let a = outline(&line(300.0), 12.0, &props(12.0, &b), &b).unwrap();
        assert!(figcraft_geom::split_subpaths(&a).len() > 8, "{id}: many pieces");
        assert_ne!(a, outline(&line(300.0), 12.0, &props(12.0, &c), &c).unwrap(), "{id}: seeded");
    }
}

/// Graybox: a brush pen's width varies along the stroke like pen pressure.
#[test]
fn a_brush_pen_varies_its_width() {
    let b = Brush { taper: false, ..brush("brushpen") };
    let out = outline(&line(400.0), 20.0, &props(20.0, &b), &b).unwrap();
    let widths: Vec<f64> = (1..20)
        .map(|k| {
            let x = f64::from(k) * 20.0;
            (0..=100).map(|j| -10.0 + 0.2 * f64::from(j)).filter(|y| out.winding(Point::new(x, *y)) != 0).count() as f64 * 0.2
        })
        .collect();
    let (lo, hi) = widths.iter().fold((f64::MAX, 0.0f64), |(lo, hi), w| (lo.min(*w), hi.max(*w)));
    assert!(hi - lo > 3.0 && lo > 5.0, "{widths:?}");
}

/// Graybox: a scatter brush with taper amounts makes its copies smaller towards the ends.
#[test]
fn a_tapered_scatter_brush_shrinks_its_ends() {
    let b = Brush { taper_start: Some(0.3), taper_end: Some(0.3), ..brush("dots") };
    let out = outline(&line(300.0), 10.0, &props(10.0, &b), &b).unwrap();
    let parts = figcraft_geom::split_subpaths(&out);
    let (first, mid) = (parts.first().unwrap().bounding_box(), parts.get(parts.len() / 2).unwrap().bounding_box());
    assert!(first.width() < mid.width() * 0.5, "{first:?} {mid:?}");
}
