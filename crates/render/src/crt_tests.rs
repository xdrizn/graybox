//! The CRT backdrop: scanlines every period, bleed, direction, curvature, colour split, the shape's
//! outline, zoomed-out lines, render cuts and degenerate layers.

use figcraft_color::{Color, Paint};
use figcraft_doc::{Crt, CrtDirection, Document, Effect, EffectKind, NodeId, NodeKind};
use tiny_skia::Pixmap;

use crate::{Options, View, render_node, render_view};

/// Only the lines: period 4, dark half, fully dark, no bleed.
fn lines_only() -> Crt {
    Crt {
        curvature: 0.0,
        vignette: 0.0,
        line_size: 4.0,
        thickness: 0.5,
        line_opacity: 1.0,
        bleed: 0.0,
        direction: CrtDirection::Horizontal,
        rgb_split: 0.0,
        phosphor: 0.0,
        glow: 0.0,
        brightness: 1.0,
    }
}

/// Nothing at all: the backdrop as it is.
fn plain() -> Crt {
    Crt { line_opacity: 0.0, ..lines_only() }
}

fn solid(d: &mut Document, id: NodeId, hex: &str) {
    d.get_mut(id).unwrap().fills = vec![Paint::solid(Color::from_hex(hex).unwrap())];
}

/// A 240 × 160 frame filled with `back` (or, for "bars", white and black vertical bars 20 wide)
/// and a fill-less 120 × 80 pane at (60, 40) carrying only `c`: (document, frame, pane).
fn crt_scene(c: Crt, back: &str) -> (Document, NodeId, NodeId) {
    let mut d = Document::new("crt");
    let p = d.first_page().unwrap();
    let f = d.create(p, NodeKind::Frame, "Frame", 0.0, 0.0, 240.0, 160.0).unwrap();
    d.get_mut(f).unwrap().clip = true;
    if back == "bars" {
        solid(&mut d, f, "000000");
        for i in 0..6 {
            let b = d.create(f, NodeKind::Rectangle, "Bar", f64::from(i) * 40.0, 0.0, 20.0, 160.0).unwrap();
            solid(&mut d, b, "FFFFFF");
        }
    } else {
        solid(&mut d, f, back);
    }
    let pane = d.create(f, NodeKind::Rectangle, "Pane", 60.0, 40.0, 120.0, 80.0).unwrap();
    let n = d.get_mut(pane).unwrap();
    n.fills = vec![];
    n.effects = vec![Effect { kind: EffectKind::Crt(c), visible: true }];
    (d, f, pane)
}

fn px(p: &Pixmap, x: u32, y: u32) -> [u8; 4] {
    let c = p.pixel(x, y).unwrap();
    [c.red(), c.green(), c.blue(), c.alpha()]
}

/// The frame rendered without the pane's effect.
fn without_effect(d: &Document, f: NodeId, pane: NodeId, scale: f64) -> Pixmap {
    let mut d = d.clone();
    d.get_mut(pane).unwrap().effects.clear();
    render_node(&d, f, scale).unwrap()
}

#[test]
fn lines_darken_every_period() {
    let (d, f, _) = crt_scene(lines_only(), "808080");
    let got = render_node(&d, f, 1.0).unwrap();
    for y in 40..120 {
        let local = (y - 40) % 4;
        for x in [61, 100, 178] {
            let g = px(&got, x, y)[1];
            if local < 2 {
                assert!(g <= 10, "dark row {y}: {g}");
            } else {
                assert!(g.abs_diff(0x80) <= 3, "light row {y}: {g}");
            }
        }
    }
    // Outside the pane the backdrop is untouched.
    assert!(px(&got, 30, 41)[1].abs_diff(0x80) <= 1);
}

#[test]
fn bleed_keeps_white_lines_white() {
    let (d, f, _) = crt_scene(Crt { bleed: 1.0, ..lines_only() }, "FFFFFF");
    let got = render_node(&d, f, 1.0).unwrap();
    for y in 40..120 {
        assert!(px(&got, 100, y)[1] >= 0xF0, "row {y}");
    }
}

#[test]
fn vertical_lines_run_down() {
    let (d, f, _) = crt_scene(Crt { direction: CrtDirection::Vertical, ..lines_only() }, "808080");
    let got = render_node(&d, f, 1.0).unwrap();
    for x in 60..180 {
        let top = px(&got, x, 42)[1];
        assert!(((x - 60) % 4 < 2) == (top <= 10), "column {x}: {top}");
        for y in [60, 100, 118] {
            assert_eq!(px(&got, x, y)[1], top, "column {x} row {y}");
        }
    }
}

#[test]
fn full_curvature_blackens_corners_keeps_centre() {
    let (d, f, _) = crt_scene(Crt { curvature: 1.0, ..plain() }, "808080");
    let got = render_node(&d, f, 1.0).unwrap();
    assert!(px(&got, 61, 41)[1] <= 10, "{:?}", px(&got, 61, 41));
    assert!(px(&got, 120, 80)[1].abs_diff(0x80) <= 3, "{:?}", px(&got, 120, 80));
}

#[test]
fn rgb_split_parts_colours_at_the_edge_only() {
    let (d, f, _) = crt_scene(Crt { rgb_split: 1.0, ..plain() }, "bars");
    let got = render_node(&d, f, 1.0).unwrap();
    // A bar edge near the pane's left side (x = 80) and one at its centre (x = 120).
    let edge = (76..84).map(|x| px(&got, x, 44)).any(|p| p[0].abs_diff(p[2]) > 40);
    assert!(edge, "red and blue part near the edge");
    for x in 116..124 {
        let p = px(&got, x, 80);
        // At most a hair of a hard bar edge (the split grows from 0 at the centre).
        assert!(p[0].abs_diff(p[2]) <= 16, "centre {x}: {p:?}");
    }
}

#[test]
fn rounded_shape_leaves_outside_pixels() {
    let (mut d, f, pane) = crt_scene(lines_only(), "808080");
    d.get_mut(pane).unwrap().corners.radii = [30.0; 4];
    let got = render_node(&d, f, 1.0).unwrap();
    let want = without_effect(&d, f, pane, 1.0);
    assert_eq!(px(&got, 61, 41), px(&want, 61, 41));
    // Inside the rounding the lines show.
    assert!(px(&got, 100, 41)[1] <= 10);
}

#[test]
fn zoomed_out_lines_fade_evenly() {
    let (d, f, _) = crt_scene(lines_only(), "808080");
    let got = render_node(&d, f, 0.25).unwrap();
    // The pane lies at 15..45 × 10..30; keep a pixel clear of its edges.
    let vals: Vec<u8> = (11..29).flat_map(|y| (16..44).map(move |x| (x, y))).map(|(x, y)| px(&got, x, y)[1]).collect();
    let (lo, hi) = (vals.iter().min().unwrap(), vals.iter().max().unwrap());
    assert!(hi - lo <= 12, "{lo}..{hi}");
    // Half of each period is dark: about half the grey.
    assert!(lo.abs_diff(0x40) <= 12, "{lo}");
}

#[test]
fn band_cut_does_not_shift_lines() {
    let (d, f, _) = crt_scene(Crt { curvature: 0.5, rgb_split: 0.5, ..lines_only() }, "bars");
    let page = d.first_page().unwrap();
    let _ = f;
    let at = |y0: f64| View { origin: (0.0, y0), scale: 1.0, width: 240, height: 160 };
    let a = render_view(&d, page, &at(0.0), &Options::default()).unwrap();
    let b = render_view(&d, page, &at(3.0), &Options::default()).unwrap();
    let mut worst = 0;
    for y in 40..117 {
        for x in 60..180 {
            let (p, q) = (px(&a, x, y + 3), px(&b, x, y));
            worst = worst.max(p.iter().zip(q).map(|(c, d)| c.abs_diff(d)).max().unwrap());
        }
    }
    assert!(worst <= 2, "{worst}");
}

#[test]
fn degenerate_layers_draw_nothing() {
    for (w, h) in [(0.0, 80.0), (1.0, 1.0), (120.0, 0.0)] {
        let (mut d, f, pane) = crt_scene(Crt::default(), "808080");
        let n = d.get_mut(pane).unwrap();
        n.width = w;
        n.height = h;
        let got = render_node(&d, f, 1.0).unwrap();
        let want = without_effect(&d, f, pane, 1.0);
        if w >= 1.0 && h >= 1.0 {
            continue; // a 1 × 1 pane draws one pixel: only no panic is asked of it
        }
        assert_eq!(got.data(), want.data(), "{w} × {h}");
    }
}

#[test]
fn hidden_crt_changes_nothing() {
    let (mut d, f, pane) = crt_scene(Crt::default(), "808080");
    d.get_mut(pane).unwrap().effects[0].visible = false;
    let got = render_node(&d, f, 1.0).unwrap();
    assert_eq!(got.data(), without_effect(&d, f, pane, 1.0).data());
}

#[test]
fn a_view_cutting_the_pane_draws_the_same_pixels() {
    // The view's top edge runs through the pane's centre: below it the bulge reads away from the
    // cut, so the pixels match. (Above a cut the backdrop past the view is not drawn at all.)
    let (d, _, _) = crt_scene(Crt { curvature: 1.0, rgb_split: 0.5, ..lines_only() }, "bars");
    let page = d.first_page().unwrap();
    let at = |y0: f64, h: u32| View { origin: (0.0, y0), scale: 1.0, width: 240, height: h };
    let whole = render_view(&d, page, &at(0.0, 160), &Options::default()).unwrap();
    let cut = render_view(&d, page, &at(80.0, 80), &Options::default()).unwrap();
    let mut worst = (0, 0, 0);
    for y in 0..38 {
        for x in 60..180 {
            let (p, q) = (px(&whole, x, y + 80), px(&cut, x, y));
            let diff = p.iter().zip(q).map(|(c, d)| c.abs_diff(d)).max().unwrap();
            if diff > worst.0 {
                worst = (diff, x, y);
            }
        }
    }
    assert!(worst.0 <= 2, "{worst:?}");
}

#[test]
fn bands_draw_what_one_piece_draws() {
    // Big enough for the renderer to split it into parallel bands: the bulge reads across a band's
    // edge, so a band must still see the whole backdrop under the pane.
    let (d, _, _) = crt_scene(Crt { curvature: 1.0, rgb_split: 0.5, ..lines_only() }, "bars");
    let page = d.first_page().unwrap();
    let view = View { origin: (0.0, 0.0), scale: 5.0, width: 1200, height: 800 };
    let banded = render_view(&d, page, &view, &Options::default()).unwrap();
    let whole = crate::render_view_single(&d, page, &view, &Options::default()).unwrap();
    let worst = banded.data().iter().zip(whole.data()).map(|(a, b)| a.abs_diff(*b)).max().unwrap();
    assert!(worst <= 2, "{worst}");
}

#[test]
fn over_a_transparent_backdrop_the_crt_replaces_what_is_under_it() {
    // In Play the frame has no background (the game shows through): where the bulged picture
    // leaves the backdrop's transparency, nothing of the unbent layers may show.
    let (mut d, f, pane) = crt_scene(Crt { curvature: 1.0, ..plain() }, "808080");
    d.get_mut(f).unwrap().fills.clear();
    let r = d.create(f, NodeKind::Rectangle, "Box", 100.0, 60.0, 40.0, 40.0).unwrap();
    solid(&mut d, r, "FF5A00");
    // Below the pane in z-order.
    let fr = d.get_mut(f).unwrap();
    let (ri, pi) = (fr.children.iter().position(|c| *c == r).unwrap(), fr.children.iter().position(|c| *c == pane).unwrap());
    fr.children.swap(ri, pi);
    let got = render_node(&d, f, 1.0).unwrap();
    // The box's own corner: the bulge shows the box smaller, so the backdrop there is clear.
    assert!(px(&got, 100, 60)[3] <= 10, "{:?}", px(&got, 100, 60));
    // The box's centre still shows it.
    assert!(px(&got, 120, 80)[0] >= 0xF0, "{:?}", px(&got, 120, 80));
    // Outside the pane nothing changes.
    let want = without_effect(&d, f, pane, 1.0);
    assert_eq!(px(&got, 30, 30), px(&want, 30, 30));
}
