//! Graybox corner styles through the engine: `node.set` `cornerStyle`, undo, instances, copy and
//! paste of properties, clipping in the renderer, and the SVG and PDF exports.

use figcraft_doc::{CornerStyle, NodeId};
use figcraft_geom::Shape;
use serde_json::{Value, json};

use crate::Session;
use crate::params::id_of;

fn ex(s: &mut Session, cmd: &str, p: Value) -> Value {
    s.execute(cmd, &p).unwrap_or_else(|e| panic!("{cmd}: {e}"))
}

fn create(s: &mut Session, cmd: &str, p: Value) -> NodeId {
    id_of(&ex(s, cmd, p)["id"]).unwrap()
}

fn style(s: &Session, id: NodeId) -> CornerStyle {
    s.doc().unwrap().node(id).unwrap().corners.style
}

fn area(s: &Session, id: NodeId) -> f64 {
    figcraft_doc::geometry::shape_path(s.doc().unwrap().node(id).unwrap()).unwrap().area().abs()
}

#[test]
fn node_set_takes_every_style_and_undo_brings_round_back() {
    let mut s = Session::new();
    let r = create(&mut s, "shape.rectangle", json!({"x": 0, "y": 0, "width": 100, "height": 60}));
    ex(&mut s, "node.set", json!({"id": r, "props": {"cornerRadius": 20}}));
    let round = area(&s, r);
    for st in CornerStyle::ALL {
        ex(&mut s, "node.set", json!({"id": r, "props": {"cornerStyle": st.id()}}));
        assert_eq!(style(&s, r), st);
    }
    ex(&mut s, "node.set", json!({"id": r, "props": {"cornerStyle": "chamfer"}}));
    assert!((area(&s, r) - (6000.0 - 800.0)).abs() < 1e-6, "four chamfers cut 200 each");
    assert_ne!(area(&s, r), round);
    // Inspection tells the style; round stays unsaid.
    assert_eq!(ex(&mut s, "node.get", json!({"id": r}))["cornerStyle"], json!("chamfer"));
    // Common names work; junk is refused and changes nothing.
    ex(&mut s, "node.set", json!({"id": r, "props": {"cornerShape": "Inverse round"}}));
    assert_eq!(style(&s, r), CornerStyle::Scoop);
    for junk in [json!("wobbly"), json!(3), json!([]), json!({"style": "notch"})] {
        assert!(s.execute("node.set", &json!({"id": r, "props": {"cornerStyle": junk}})).is_err(), "{junk}");
        assert_eq!(style(&s, r), CornerStyle::Scoop);
    }
    ex(&mut s, "edit.undo", json!({}));
    assert_eq!(style(&s, r), CornerStyle::Chamfer);
    ex(&mut s, "node.set", json!({"id": r, "props": {"cornerStyle": null}}));
    assert_eq!(style(&s, r), CornerStyle::Round);
    assert!(ex(&mut s, "node.get", json!({"id": r})).get("cornerStyle").is_none());
    assert!((area(&s, r) - round).abs() < 1e-9);
}

#[test]
fn instances_follow_the_main_until_they_set_their_own_style() {
    let mut s = Session::new();
    let f = create(&mut s, "frame.create", json!({"x": 0, "y": 0, "width": 100, "height": 60}));
    ex(&mut s, "node.set", json!({"id": f, "props": {"cornerRadius": 12}}));
    let c = create(&mut s, "component.create", json!({"ids": [f]}));
    let i = create(&mut s, "instance.create", json!({"component": c, "x": 200, "y": 0}));
    ex(&mut s, "node.set", json!({"id": c, "props": {"cornerStyle": "notch"}}));
    assert_eq!(style(&s, i), CornerStyle::Notch, "the instance follows its main");
    ex(&mut s, "node.set", json!({"id": i, "props": {"cornerStyle": "vNotch"}}));
    ex(&mut s, "node.set", json!({"id": c, "props": {"cornerStyle": "chamfer"}}));
    assert_eq!(style(&s, i), CornerStyle::VNotch, "its own style is an override");
}

#[test]
fn copy_and_paste_properties_carry_the_style() {
    let mut s = Session::new();
    let a = create(&mut s, "shape.rectangle", json!({"x": 0, "y": 0, "width": 50, "height": 50}));
    let b = create(&mut s, "shape.rectangle", json!({"x": 100, "y": 0, "width": 50, "height": 50}));
    ex(&mut s, "node.set", json!({"id": a, "props": {"cornerRadius": 10, "cornerStyle": "scoop"}}));
    ex(&mut s, "edit.copyProperties", json!({"id": a}));
    ex(&mut s, "edit.pasteProperties", json!({"ids": [b]}));
    assert_eq!(style(&s, b), CornerStyle::Scoop);
}

#[test]
fn a_frame_clips_its_content_to_its_cut_corners() {
    let mut s = Session::new();
    let f = create(&mut s, "frame.create", json!({"x": 0, "y": 0, "width": 100, "height": 100}));
    ex(&mut s, "node.set", json!({"id": f, "props": {"cornerRadius": 30, "cornerStyle": "notch", "fill": "none", "clip": true}}));
    let kid = create(&mut s, "shape.rectangle", json!({"x": 0, "y": 0, "width": 100, "height": 100, "parent": f}));
    ex(&mut s, "node.set", json!({"id": kid, "props": {"fill": "#ff0000"}}));
    let pix = figcraft_render::render_node(s.doc().unwrap(), f, 1.0).unwrap();
    assert_eq!(pix.pixel(15, 15).unwrap().alpha(), 0, "the notch hides the child");
    assert_eq!(pix.pixel(85, 85).unwrap().alpha(), 0, "every corner");
    assert_eq!(pix.pixel(50, 50).unwrap().alpha(), 255);
    assert_eq!(pix.pixel(40, 10).unwrap().alpha(), 255, "beside the notch");
    // A chamfer: cut on the diagonal, kept past it.
    ex(&mut s, "node.set", json!({"id": f, "props": {"cornerStyle": "chamfer"}}));
    let pix = figcraft_render::render_node(s.doc().unwrap(), f, 1.0).unwrap();
    assert_eq!(pix.pixel(8, 8).unwrap().alpha(), 0);
    assert_eq!(pix.pixel(20, 20).unwrap().alpha(), 255);
}

#[test]
fn strokes_follow_the_cut_corners() {
    let mut s = Session::new();
    let r = create(&mut s, "shape.rectangle", json!({"x": 0, "y": 0, "width": 100, "height": 100}));
    ex(
        &mut s,
        "node.set",
        json!({"id": r, "props": {"cornerRadius": 30, "cornerStyle": "notch", "fill": "none", "stroke": "#000000", "strokeWeight": 4, "strokeAlign": "inside"}}),
    );
    let pix = figcraft_render::render_node(s.doc().unwrap(), r, 1.0).unwrap();
    assert!(pix.pixel(31, 15).unwrap().alpha() > 200, "the stroke runs down the notch's step");
    assert_eq!(pix.pixel(15, 15).unwrap().alpha(), 0, "nothing in the notch");
    assert_eq!(pix.pixel(50, 50).unwrap().alpha(), 0, "hollow");
    // Per-side weights too.
    ex(&mut s, "node.set", json!({"id": r, "props": {"strokeTopWeight": 10}}));
    let pix = figcraft_render::render_node(s.doc().unwrap(), r, 1.0).unwrap();
    assert!(pix.pixel(50, 8).unwrap().alpha() > 200 && pix.pixel(50, 13).unwrap().alpha() == 0);
    assert_eq!(pix.pixel(15, 15).unwrap().alpha(), 0);
}

#[test]
fn svg_and_pdf_exports_draw_the_cut_corners() {
    let mut s = Session::new();
    let r = create(&mut s, "shape.rectangle", json!({"x": 0, "y": 0, "width": 100, "height": 60}));
    ex(&mut s, "node.set", json!({"id": r, "props": {"cornerRadius": 20, "fill": "#336699", "stroke": "#000000", "strokeAlign": "inside"}}));
    let svg = |s: &Session| figcraft_svg::export_svg(s.doc().unwrap(), &[r], &figcraft_svg::SvgOptions::default()).unwrap();
    let round = svg(&s);
    ex(&mut s, "node.set", json!({"id": r, "props": {"cornerStyle": "chamfer"}}));
    let cut = svg(&s);
    assert_ne!(round, cut);
    // A chamfered rectangle is all straight lines: the fill's path has no curves.
    assert!(round.contains('C') || round.contains('A') || round.contains("rx="), "round corners curve: {round}");
    let fill = cut.split("<path").find(|p| p.contains("#336699")).unwrap_or_else(|| panic!("{cut}"));
    let d = fill.split("d=\"").nth(1).unwrap().split('"').next().unwrap();
    assert!(!d.contains('C') && !d.contains('Q') && !d.contains('A'), "{d}");
    assert!(d.contains("20") && d.contains("80"), "the cut starts 20 in: {d}");
    let pdf = figcraft_pdf::export_nodes_pdf(s.doc().unwrap(), &[r]).unwrap();
    assert!(pdf.starts_with(b"%PDF"));
}
