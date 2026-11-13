use super::*;

fn close(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() < 1e-6 && (a.y - b.y).abs() < 1e-6
}

#[test]
fn a_flat_tilt_maps_every_point_to_itself() {
    let h = Tilt::default().homography(Point::new(50.0, 20.0), (100.0, 40.0));
    for p in [Point::new(0.0, 0.0), Point::new(100.0, 40.0), Point::new(-30.0, 7.5)] {
        assert!(close(h.apply(p).unwrap(), p));
    }
}

#[test]
fn y_sixty_degrees_brings_the_right_side_near() {
    // Worked by hand: a 200 × 100 box centred on the origin, Y = 60°, perspective 1000.
    // Corner (100, -50): z = 100·sin 60° = 86.603; X' = 1000·50 / 913.397 = 54.741, Y' = -54.741.
    let t = Tilt::new(0.0, 60.0, 1000.0);
    let h = t.homography(Point::ORIGIN, (100.0, 50.0));
    let p = h.apply(Point::new(100.0, -50.0)).unwrap();
    assert!((p.x - 54.741).abs() < 1e-3 && (p.y + 54.741).abs() < 1e-3, "{p:?}");
    // Left corner goes away: smaller.
    let q = h.apply(Point::new(-100.0, -50.0)).unwrap();
    assert!(q.y.abs() < 50.0 && p.y.abs() > 50.0, "{q:?}");
}

#[test]
fn x_positive_tips_the_top_away() {
    let h = Tilt::new(40.0, 0.0, 800.0).homography(Point::ORIGIN, (100.0, 50.0));
    let top = h.apply(Point::new(100.0, -50.0)).unwrap();
    let bottom = h.apply(Point::new(100.0, 50.0)).unwrap();
    assert!(top.x < 100.0 && bottom.x > 100.0, "{top:?} {bottom:?}");
}

#[test]
fn angles_are_clamped_and_junk_is_dropped() {
    assert_eq!(Tilt::new(200.0, -95.0, 10.0), Tilt { x: 80.0, y: -80.0, perspective: MIN_PERSPECTIVE });
    assert_eq!(Tilt::new(f64::NAN, f64::INFINITY, f64::NAN), Tilt::default());
    assert_eq!(Tilt::decode("10, -20, 500"), Some(Tilt::new(10.0, -20.0, 500.0)));
    for junk in ["", "1,2", "1,2,3,4", "a,b,c", "1,,3"] {
        assert_eq!(Tilt::decode(junk), None, "{junk}");
    }
    assert_eq!(Tilt::decode(&Tilt::new(12.5, -3.0, 640.0).encode()), Some(Tilt::new(12.5, -3.0, 640.0)));
}

#[test]
fn a_layer_turned_in_its_own_space_keeps_its_centre() {
    // A 200 × 40 bar at (300, 100), rotated 30° in the plane, then Y = -45° (left side near).
    let a = Affine::translate((300.0, 100.0)) * Affine::rotate(30f64.to_radians());
    let h = Tilt::new(0.0, -45.0, 600.0).local(a, 200.0, 40.0);
    let c = h.apply(Point::new(100.0, 20.0)).unwrap();
    assert!(close(c, Point::new(100.0, 20.0)), "{c:?}");
    // The left end, in the parent, is farther from the centre than the right end (nearer = bigger).
    let to_parent = |p: Point| a * h.apply(p).unwrap();
    let centre = a * Point::new(100.0, 20.0);
    let (l, r) = (to_parent(Point::new(0.0, 20.0)), to_parent(Point::new(200.0, 20.0)));
    assert!((l - centre).hypot() > (r - centre).hypot(), "{l:?} {r:?}");
}

#[test]
fn bounds_cut_what_falls_behind_the_eye() {
    let h = Tilt::new(0.0, 80.0, 50.0).homography(Point::ORIGIN, (10.0, 10.0));
    // Far past the box, the right side would be behind the eye: cut, still finite.
    let b = h.bounds(Rect::new(-10_000.0, -10.0, 10_000.0, 10.0)).unwrap();
    assert!(b.x1.is_finite() && b.x0.is_finite());
    assert!(h.bounds(Rect::new(1e6, -1.0, 2e6, 1.0)).is_none());
}

/// The owner's bar, 320 × 56 at (100, 100), Y −60° at perspective 600, its bottom-right corner
/// dragged: the solved corner is drawn where the pointer is, the left side stays put (flat).
#[test]
fn a_resized_tilted_box_is_drawn_with_its_corner_under_the_pointer() {
    let t = Tilt::new(0.0, -60.0, 600.0);
    let r = Resize {
        parent: Affine::IDENTITY,
        start: Affine::translate((100.0, 100.0)),
        size: (320.0, 56.0),
        anchor: Point::new(0.0, 0.0),
        moves: (true, true),
    };
    let start = r.drawn(&t, Point::new(320.0, 56.0)).unwrap();
    // Untouched, the corner is drawn where it was.
    assert!(close(r.solve(&t, start, Point::new(320.0, 56.0)).unwrap(), Point::new(320.0, 56.0)));
    let p = Point::new(start.x + 30.0, start.y + 4.0);
    let c = r.solve(&t, p, Point::new(320.0, 56.0)).unwrap();
    assert!(c.x > 320.0, "wider: {c:?}");
    let at = r.drawn(&t, c).unwrap();
    assert!((at - p).hypot() < 1e-6, "{at:?} {p:?}");
    // A flat box: the pointer's own point.
    let flat = Tilt::default();
    assert!(close(r.solve(&flat, Point::new(500.0, 170.0), Point::new(320.0, 56.0)).unwrap(), Point::new(400.0, 70.0)));
}

/// A side handle moves one side: the solved point is on that side, drawn under the pointer.
#[test]
fn a_side_dragged_through_the_tilt_lands_under_the_pointer() {
    let t = Tilt::new(20.0, 35.0, 500.0);
    let r = Resize {
        parent: Affine::translate((10.0, 5.0)),
        start: Affine::translate((100.0, 100.0)),
        size: (200.0, 40.0),
        anchor: Point::new(0.0, 0.0),
        moves: (true, false),
    };
    let p = r.drawn(&t, Point::new(200.0, 30.0)).unwrap() + figcraft_geom::Vec2::new(-25.0, 3.0);
    let c = r.solve(&t, p, Point::new(200.0, 30.0)).unwrap();
    assert!((r.drawn(&t, c).unwrap() - p).hypot() < 1e-6);
    assert!(c.x < 200.0);
}

mod props {
    use super::super::*;
    use proptest::prelude::*;

    fn tilt() -> impl Strategy<Value = Tilt> {
        (-100.0..100.0f64, -100.0..100.0f64, 1.0..200_000.0f64).prop_map(|(x, y, d)| Tilt::new(x, y, d))
    }

    proptest! {
        #[test]
        fn corners_map_in_front_and_finite(t in tilt(), w in 1.0..5000.0f64, h in 1.0..5000.0f64, rot in -3.2..3.2f64, tx in -1e4..1e4f64) {
            let a = Affine::translate((tx, -tx / 2.0)) * Affine::rotate(rot);
            let m = t.local(a, w, h);
            for p in [Point::new(0.0, 0.0), Point::new(w, 0.0), Point::new(w, h), Point::new(0.0, h)] {
                prop_assert!(m.apply_h(p).2 >= 0.19, "{t:?} {p:?} {:?}", m.apply_h(p));
                let q = m.apply(p);
                prop_assert!(q.is_some_and(|q| q.x.is_finite() && q.y.is_finite()));
            }
            prop_assert!(t.x.abs() <= MAX_ANGLE && t.y.abs() <= MAX_ANGLE);
        }

        #[test]
        fn the_inverse_round_trips(t in tilt(), w in 1.0..2000.0f64, h in 1.0..2000.0f64, u in 0.0..1.0f64, v in 0.0..1.0f64) {
            let m = t.local(Affine::translate((40.0, 10.0)), w, h);
            let inv = m.inverse().unwrap();
            let p = Point::new(u * w, v * h);
            let back = inv.apply(m.apply(p).unwrap()).unwrap();
            prop_assert!((back.x - p.x).abs() < 1e-6 * w.max(h) && (back.y - p.y).abs() < 1e-6 * w.max(h), "{p:?} {back:?}");
        }

        #[test]
        fn bounds_hold_every_corner(t in tilt(), w in 1.0..2000.0f64, h in 1.0..2000.0f64) {
            let m = t.homography(Point::new(w / 2.0, h / 2.0), (w / 2.0, h / 2.0));
            let b = m.bounds(Rect::new(0.0, 0.0, w, h)).unwrap();
            for p in [Point::new(0.0, 0.0), Point::new(w, 0.0), Point::new(w, h), Point::new(0.0, h)] {
                let q = m.apply(p).unwrap();
                prop_assert!(b.inflate(1e-6, 1e-6).contains(q));
            }
        }

        #[test]
        fn a_dragged_corner_is_drawn_under_the_pointer(
            x in -60.0..60.0f64, y in -60.0..60.0f64, d in 300.0..5000.0f64,
            w in 20.0..1000.0f64, h in 20.0..1000.0f64, rot in -3.2..3.2f64,
            corner in 0usize..4, du in -0.2..0.2f64, dv in -0.2..0.2f64,
        ) {
            let t = Tilt::new(x, y, d);
            let start = Affine::translate((50.0, -20.0)) * Affine::rotate(rot);
            let corners = [Point::new(0.0, 0.0), Point::new(w, 0.0), Point::new(w, h), Point::new(0.0, h)];
            let (moving, anchor) = (corners[corner], corners[(corner + 2) % 4]);
            let r = Resize { parent: Affine::scale(1.5), start, size: (w, h), anchor, moves: (true, true) };
            let from = r.drawn(&t, moving).unwrap();
            let p = from + figcraft_geom::Vec2::new(du * w, dv * h);
            let c = r.solve(&t, p, moving);
            prop_assert!(c.is_some(), "no solution for {p:?}");
            let at = r.drawn(&t, c.unwrap()).unwrap();
            prop_assert!((at - p).hypot() < 1e-4 * w.max(h), "{at:?} {p:?}");
        }

        #[test]
        fn decode_never_fails_loudly(s in ".{0,40}") {
            if let Some(t) = Tilt::decode(&s) {
                prop_assert!(t.x.is_finite() && t.y.is_finite() && t.perspective.is_finite());
            }
        }
    }
}
