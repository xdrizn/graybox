//! 9-slice with components (G16.7): set on the main component, on its pixel layer, or on a pixel
//! layer before it becomes a component, an instance resized keeps the art's corners.

use serde_json::json;

use super::ex;
use crate::Session;
use figcraft_doc::NodeId;

/// A 16 × 16 board at scale 4 at (0, 0), red, with a blue top-left and a green bottom-right pixel.
fn button(s: &mut Session) -> u64 {
    let id = ex(s, "pixel.create", json!({"x": 0, "y": 0, "width": 16, "height": 16, "scale": 4}))["id"].as_u64().unwrap();
    ex(s, "pixel.shape", json!({"id": id, "kind": "rect", "from": [0, 0], "to": [15, 15], "filled": true, "color": "#ff0000"}));
    ex(s, "pixel.stroke", json!({"id": id, "points": [[0, 0]], "color": "#0000ff"}));
    ex(s, "pixel.stroke", json!({"id": id, "points": [[15, 15]], "color": "#00ff00"}));
    id
}

/// An instance of `main` at (400, 0), resized to 128 × 128; its id.
fn grown_instance(s: &mut Session, main: u64) -> u64 {
    let inst = ex(s, "instance.create", json!({"component": main, "x": 400, "y": 0}))["id"].as_u64().unwrap();
    ex(s, "transform.resize", json!({"ids": [inst], "width": 128, "height": 128}));
    inst
}

/// The colour drawn at (`x`, `y`) inside the 128 × 128 instance at (400, 0).
fn drawn(s: &Session, inst: u64, x: u32, y: u32) -> [u8; 4] {
    let doc = s.doc().unwrap();
    let px = figcraft_render::render_area(doc, &[NodeId(inst)], figcraft_geom::Rect::new(400.0, 0.0, 528.0, 128.0), 1.0).unwrap();
    let p = px.pixels()[(y * px.width() + x) as usize].demultiply();
    [p.red(), p.green(), p.blue(), p.alpha()]
}

fn corners_stay(s: &Session, inst: u64) {
    const BLUE: [u8; 4] = [0, 0, 255, 255];
    const GREEN: [u8; 4] = [0, 255, 0, 255];
    const RED: [u8; 4] = [255, 0, 0, 255];
    assert_eq!(drawn(s, inst, 1, 1), BLUE, "the top-left corner pixel stays one art pixel (4 points)");
    assert_eq!(drawn(s, inst, 5, 5), RED, "past the corner pixel, the art");
    assert_eq!(drawn(s, inst, 126, 126), GREEN, "the bottom-right corner moves with the instance's side");
    assert_eq!(drawn(s, inst, 64, 64), RED, "the middle stretches");
}

#[test]
fn nine_slice_on_the_main_components_pixel_layer_resizes_its_instances() {
    let mut s = Session::new();
    let id = button(&mut s);
    let main = ex(&mut s, "component.create", json!({"ids": [id]}))["id"].as_u64().unwrap();
    let doc = s.doc().unwrap();
    let layer = if doc.node(NodeId(main)).unwrap().children.is_empty() { main } else { id };
    ex(&mut s, "game.nineSlice.set", json!({"ids": [layer], "enabled": true}));
    let inst = grown_instance(&mut s, main);
    corners_stay(&s, inst);
}

#[test]
fn nine_slice_on_the_main_component_itself_resizes_its_instances() {
    let mut s = Session::new();
    let id = button(&mut s);
    let main = ex(&mut s, "component.create", json!({"ids": [id]}))["id"].as_u64().unwrap();
    ex(&mut s, "game.nineSlice.set", json!({"ids": [main], "enabled": true, "insets": [4, 4, 4, 4]}));
    let inst = grown_instance(&mut s, main);
    corners_stay(&s, inst);
}

#[test]
fn a_layer_9_sliced_before_it_became_a_component_resizes_its_instances() {
    let mut s = Session::new();
    let id = button(&mut s);
    ex(&mut s, "game.nineSlice.set", json!({"ids": [id], "enabled": true}));
    let main = ex(&mut s, "component.create", json!({"ids": [id]}))["id"].as_u64().unwrap();
    let inst = grown_instance(&mut s, main);
    corners_stay(&s, inst);
}
