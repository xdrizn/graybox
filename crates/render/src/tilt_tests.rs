use crate::paint_behaviour_tests::{page, rect};
use crate::{View, render_nodes_into};
use figcraft_color::{Color, Paint};
use figcraft_doc::tilt::{KEY, Tilt};
use figcraft_doc::{Document, NodeId, NodeKind};

/// A 200 × 40 bar at (100, 80) on a 400 × 200 view, red, with a white fill on its left half.
fn bar(t: Option<Tilt>) -> (Document, NodeId) {
    let (mut d, p) = page();
    let f = d.create(p, NodeKind::Frame, "Bar", 100.0, 80.0, 200.0, 40.0).unwrap();
    d.get_mut(f).unwrap().fills = vec![Paint::solid(Color::from_hex("#ff0000").unwrap())];
    rect(&mut d, f, (0.0, 0.0, 100.0, 40.0), vec![Paint::solid(Color::WHITE)]);
    if let Some(t) = t {
        d.get_mut(f).unwrap().plugin_data.insert(KEY.into(), t.encode());
    }
    (d, f)
}

fn draw(d: &Document, f: NodeId) -> tiny_skia::Pixmap {
    let mut pix = tiny_skia::Pixmap::new(400, 200).unwrap();
    pix.fill(tiny_skia::Color::BLACK);
    render_nodes_into(d, &[f], &View { origin: (0.0, 0.0), scale: 1.0, width: 400, height: 200 }, &mut pix);
    pix
}

/// Rows in column `x` that aren't black.
fn height(pix: &tiny_skia::Pixmap, x: u32) -> u32 {
    (0..pix.height()).filter(|y| pix.pixel(x, *y).is_some_and(|c| c.red() > 128 || c.green() > 128)).count() as u32
}

/// Columns that aren't black.
fn span(pix: &tiny_skia::Pixmap) -> (u32, u32) {
    let cols: Vec<u32> = (0..pix.width()).filter(|x| height(pix, *x) > 0).collect();
    (*cols.first().unwrap(), *cols.last().unwrap())
}

#[test]
fn a_bar_turned_left_side_near_is_taller_on_the_left() {
    let (d, f) = bar(Some(Tilt::new(0.0, -45.0, 400.0)));
    let pix = draw(&d, f);
    let (l, r) = span(&pix);
    let (hl, hr) = (height(&pix, l + 3), height(&pix, r - 3));
    assert!(hl > 44 && hr < 36, "left {hl}, right {hr}");
    // The white half is still on the left, the red one on the right.
    let mid = 100;
    let at = |x: u32| pix.pixel(x, mid).unwrap();
    assert!(at(l + 5).green() > 200, "white on the left");
    let red = at(r - 5);
    assert!(red.red() > 200 && red.green() < 60, "red on the right");
}

#[test]
fn a_flat_tilt_draws_as_before() {
    let (d0, f0) = bar(None);
    let (d1, f1) = bar(Some(Tilt::new(0.0, 0.0, 300.0)));
    assert_eq!(draw(&d0, f0).data(), draw(&d1, f1).data());
}

#[test]
fn moving_a_tilted_bar_moves_its_drawing() {
    let (mut d, f) = bar(Some(Tilt::new(20.0, 30.0, 600.0)));
    let before = draw(&d, f);
    let n = d.get_mut(f).unwrap();
    n.transform = figcraft_geom::Affine::translate((110.0, 90.0));
    let after = draw(&d, f);
    for (x, y) in [(150u32, 95u32), (200, 100), (280, 110)] {
        assert_eq!(before.pixel(x, y), after.pixel(x + 10, y + 10), "({x}, {y})");
    }
}

#[test]
fn bounds_reach_the_near_side_and_take_in_a_change_inside() {
    let (d, f) = bar(Some(Tilt::new(0.0, -45.0, 400.0)));
    let b = crate::draw::paint_bounds(&d, f).unwrap();
    // The near (left) end is taller than the 40-unit box.
    assert!(b.height() > 44.0 && b.y0 < 78.0, "{b:?}");
    let child = d.node(f).unwrap().children[0];
    let cb = crate::draw::paint_bounds(&d, child).unwrap();
    assert_eq!(cb.union(b), cb, "{cb:?} {b:?}");
}

#[test]
fn a_tilted_frame_drawn_at_any_angle_never_panics() {
    for (x, y, p) in [(80.0, 80.0, 50.0), (-80.0, 80.0, 50.0), (80.0, -80.0, 100_000.0), (1e9, -1e9, f64::NAN)] {
        let (d, f) = bar(Some(Tilt::new(x, y, p)));
        let _ = draw(&d, f);
    }
}

#[test]
fn an_export_takes_in_the_near_side() {
    let (d, f) = bar(Some(Tilt::new(0.0, -45.0, 400.0)));
    let b = crate::export_bounds(&d, f).unwrap();
    // Drawn 48.6 tall at its near (left) end: more than its 40-unit box.
    assert!(b.height() > 44.0 && b.y0 < 78.0, "{b:?}");
    let pix = crate::render_node(&d, f, 1.0).unwrap();
    // The near end's top-left corner is painted, not cut off.
    let painted = (0..4).any(|y| (0..6).any(|x| pix.pixel(x, y).is_some_and(|c| c.alpha() > 0)));
    assert!(painted, "{}×{}", pix.width(), pix.height());
}
