//! A pixel layer is a board (G16.7): its size is its drawing area in art pixels, and resizing it
//! adds or cuts art pixels on the side moved, the art staying where it is on the page (the layer
//! keeps hugging it; an empty board takes the new size).

use serde_json::json;

use super::{ex, refused};
use crate::Session;
use figcraft_doc::NodeId;

fn board(s: &mut Session, w: u32, h: u32, scale: u32) -> u64 {
    ex(s, "pixel.create", json!({"x": 100, "y": 50, "width": w, "height": h, "scale": scale}))["id"].as_u64().unwrap()
}

fn sprite(s: &Session, id: u64) -> figcraft_pixel::Sprite {
    figcraft_pixel::store::load(&s.doc().unwrap().node(NodeId(id)).unwrap().plugin_data).unwrap()
}

fn page_box(s: &Session, id: u64) -> (f64, f64, f64, f64) {
    let doc = s.doc().unwrap();
    let n = doc.node(NodeId(id)).unwrap();
    let t = doc.world_transform(NodeId(id)).as_coeffs();
    (t[4], t[5], n.width, n.height)
}

fn color_at(s: &mut Session, id: u64, x: i64, y: i64) -> Option<String> {
    ex(s, "pixel.pick", json!({"id": id, "x": x, "y": y}))["color"].as_str().map(str::to_string)
}

#[test]
fn a_bigger_canvas_adds_art_pixels_and_keeps_the_art_and_the_scale() {
    let mut s = Session::new();
    let id = board(&mut s, 32, 32, 4);
    ex(&mut s, "pixel.stroke", json!({"id": id, "points": [[0, 0]], "color": "#ff0000"}));
    // Outside Pixel mode the layer hugs its one red pixel.
    assert_eq!(page_box(&s, id), (100.0, 50.0, 4.0, 4.0));
    ex(&mut s, "pixel.canvas", json!({"id": id, "width": 48}));
    let sp = sprite(&s, id);
    assert_eq!((sp.width, sp.height, sp.scale), (48, 32, 4));
    assert_eq!(page_box(&s, id), (100.0, 50.0, 4.0, 4.0), "the layer stays round its art");
    assert_eq!(color_at(&mut s, id, 0, 0).as_deref(), Some("#ff0000"));
    // Undo is one step.
    ex(&mut s, "edit.undo", json!({}));
    assert_eq!(sprite(&s, id).width, 32);
}

#[test]
fn an_empty_board_takes_its_new_size_from_the_side_kept() {
    let mut s = Session::new();
    let id = board(&mut s, 8, 8, 2);
    ex(&mut s, "pixel.canvas", json!({"id": id, "width": 12, "anchor": "r"}));
    assert_eq!(page_box(&s, id), (92.0, 50.0, 24.0, 16.0), "its left side moved out by 4 art pixels");
    ex(&mut s, "pixel.canvas", json!({"id": id, "width": 8, "height": 12, "anchor": "c"}));
    assert_eq!(page_box(&s, id), (96.0, 46.0, 16.0, 24.0), "2 art pixels off each side, 2 added top and bottom");
}

#[test]
fn the_anchor_keeps_the_art_in_place_on_the_page() {
    let mut s = Session::new();
    let id = board(&mut s, 8, 8, 2);
    ex(&mut s, "pixel.stroke", json!({"id": id, "points": [[0, 0]], "color": "#00ff00"}));
    // Grown from the right edge (anchor r: the art stays on the right) by 4 art pixels.
    ex(&mut s, "pixel.canvas", json!({"id": id, "width": 12, "anchor": "r"}));
    assert_eq!(page_box(&s, id), (100.0, 50.0, 2.0, 2.0), "the art is where it was on the page");
    assert_eq!(color_at(&mut s, id, 4, 0).as_deref(), Some("#00ff00"), "4 art pixels in from the board's new left side");
    // Cut from the centre: 2 art pixels go from each side.
    ex(&mut s, "pixel.canvas", json!({"id": id, "width": 8, "anchor": "c"}));
    assert_eq!(page_box(&s, id), (100.0, 50.0, 2.0, 2.0));
    assert_eq!(color_at(&mut s, id, 2, 0).as_deref(), Some("#00ff00"));
}

#[test]
fn a_smaller_canvas_cuts_what_falls_outside() {
    let mut s = Session::new();
    let id = board(&mut s, 16, 16, 1);
    ex(&mut s, "pixel.stroke", json!({"id": id, "points": [[15, 15]], "color": "#0000ff"}));
    ex(&mut s, "pixel.canvas", json!({"id": id, "width": 10, "height": 10}));
    let r = ex(&mut s, "pixel.get", json!({"id": id, "pixels": true, "frame": 0}));
    let rows = r["pictures"][0]["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 10);
    assert!(rows.iter().all(|row| row.as_array().unwrap().iter().all(|c| c == ".")));
}

#[test]
fn every_layer_and_frame_move_together() {
    let mut s = Session::new();
    let id = board(&mut s, 32, 16, 1);
    ex(&mut s, "pixel.stroke", json!({"id": id, "points": [[0, 0]], "color": "#ff0000"}));
    let top = ex(&mut s, "pixel.layer.add", json!({"id": id}))["layer"].as_u64().unwrap();
    ex(&mut s, "pixel.stroke", json!({"id": id, "layer": top, "points": [[1, 0]], "color": "#00ff00"}));
    ex(&mut s, "pixel.frame.add", json!({"id": id}));
    ex(&mut s, "pixel.stroke", json!({"id": id, "frame": 1, "points": [[2, 0]], "color": "#0000ff"}));
    ex(&mut s, "pixel.canvas", json!({"id": id, "width": 36, "height": 20, "anchor": "br"}));
    assert_eq!(color_at(&mut s, id, 4, 4).as_deref(), Some("#ff0000"));
    assert_eq!(color_at(&mut s, id, 5, 4).as_deref(), Some("#00ff00"));
    let f1 = ex(&mut s, "pixel.pick", json!({"id": id, "x": 6, "y": 4, "frame": 1}));
    assert_eq!(f1["color"], "#0000ff");
    assert_eq!(sprite(&s, id).frames.len(), 2);
}

#[test]
fn canvas_size_refuses_what_it_cannot_do() {
    let mut s = Session::new();
    let id = board(&mut s, 8, 8, 1);
    assert!(refused(&mut s, "pixel.canvas", json!({"id": id, "width": 0})).contains("width"));
    assert!(refused(&mut s, "pixel.canvas", json!({"id": id, "width": 5000})).contains("width"));
    assert!(refused(&mut s, "pixel.canvas", json!({"id": id, "width": 9, "anchor": "middle"})).contains("anchor"));
}

#[test]
fn a_9_sliced_layer_or_an_instance_is_no_board() {
    let mut s = Session::new();
    let id = board(&mut s, 8, 8, 1);
    assert!(crate::cmd::pixel::board::is_board(s.doc().unwrap(), NodeId(id)));
    ex(&mut s, "game.nineSlice.set", json!({"ids": [id], "enabled": true}));
    assert!(!crate::cmd::pixel::board::is_board(s.doc().unwrap(), NodeId(id)));
    let plain = board(&mut s, 8, 8, 1);
    let main = ex(&mut s, "component.create", json!({"ids": [plain]}))["id"].as_u64().unwrap();
    let inst = ex(&mut s, "instance.create", json!({"component": main, "x": 300, "y": 0}))["id"].as_u64().unwrap();
    let doc = s.doc().unwrap();
    assert!(!crate::cmd::pixel::board::is_board(doc, NodeId(inst)));
    // Nor is the pixel layer inside the instance.
    let mut inside = doc.node(NodeId(inst)).unwrap().children.clone();
    inside.push(NodeId(inst));
    assert!(inside.iter().all(|i| !crate::cmd::pixel::board::is_board(doc, *i)));
}
