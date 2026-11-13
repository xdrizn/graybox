use figcraft_geom::Point;

use super::*;
use crate::NodeKind;

/// A page with a 320 × 56 bar at (100, 100) holding a 100 × 20 label at (10, 10); the bar tilted
/// `t` when given.
fn bar(t: Option<Tilt>) -> (Document, NodeId, NodeId) {
    let mut d = Document::new("t");
    let p = d.first_page().unwrap();
    let f = d.create(p, NodeKind::Frame, "Bar", 100.0, 100.0, 320.0, 56.0).unwrap();
    let l = d.create(f, NodeKind::Rectangle, "Label", 10.0, 10.0, 100.0, 20.0).unwrap();
    if let Some(t) = t {
        d.get_mut(f).unwrap().plugin_data.insert(KEY.into(), t.encode());
    }
    (d, f, l)
}

fn near(a: Point, b: Point) -> bool {
    (a - b).hypot() < 1e-6
}

#[test]
fn a_flat_layer_has_no_page_warp() {
    let (d, f, l) = bar(None);
    assert!(page_warp(&d, f, &[]).is_none());
    assert!(page_warp(&d, l, &[]).is_none());
    let (d, f, _) = bar(Some(Tilt::new(0.0, 0.0, 600.0)));
    assert!(page_warp(&d, f, &[]).is_none(), "3D on but not turned");
}

#[test]
fn the_page_warp_keeps_the_centre_and_brings_the_left_side_near() {
    let (d, f, _) = bar(Some(Tilt::new(0.0, -40.0, 600.0)));
    let h = page_warp(&d, f, &[]).unwrap();
    // The bar's centre on the page stays where it is.
    assert!(near(h.apply(Point::new(260.0, 128.0)).unwrap(), Point::new(260.0, 128.0)));
    // Left side near: its edge is taller than the right one.
    let tl = h.apply(Point::new(100.0, 100.0)).unwrap();
    let bl = h.apply(Point::new(100.0, 156.0)).unwrap();
    let tr = h.apply(Point::new(420.0, 100.0)).unwrap();
    let br = h.apply(Point::new(420.0, 156.0)).unwrap();
    assert!(bl.y - tl.y > 56.0 && br.y - tr.y < 56.0, "left {} right {}", bl.y - tl.y, br.y - tr.y);
}

#[test]
fn a_layer_inside_a_tilted_one_is_drawn_through_its_warp_unless_shown_flat() {
    let (d, f, l) = bar(Some(Tilt::new(20.0, -40.0, 600.0)));
    let outer = page_warp(&d, f, &[]).unwrap();
    let inner = page_warp(&d, l, &[]).unwrap();
    let p = Point::new(115.0, 115.0);
    assert!(near(inner.apply(p).unwrap(), outer.apply(p).unwrap()));
    assert!(page_warp(&d, l, &[f]).is_none(), "the bar shown flat while editing inside it");
}

#[test]
fn nested_tilts_go_inner_first() {
    let (mut d, f, l) = bar(Some(Tilt::new(0.0, -40.0, 600.0)));
    d.get_mut(l).unwrap().plugin_data.insert(KEY.into(), Tilt::new(30.0, 0.0, 400.0).encode());
    let both = page_warp(&d, l, &[]).unwrap();
    let outer = page_warp(&d, f, &[]).unwrap();
    let alone = page_warp(&d, l, &[f]).unwrap();
    let p = Point::new(112.0, 112.0);
    let want = outer.apply(alone.apply(p).unwrap()).unwrap();
    assert!(near(both.apply(p).unwrap(), want));
}
