//! Outside Pixel mode a pixel layer hugs its used pixels (G16.7): its box is the box round the
//! pixels drawn, times the scale, where they are on the page. The board keeps its size and art, so
//! Pixel mode shows it whole again; an empty board keeps the board's size.

use serde_json::json;

use super::ex;
use crate::Session;
use figcraft_doc::NodeId;

fn board(s: &mut Session, w: u32, h: u32, scale: u32) -> u64 {
    ex(s, "pixel.create", json!({"x": 100, "y": 50, "width": w, "height": h, "scale": scale}))["id"].as_u64().unwrap()
}

fn sprite(s: &Session, id: u64) -> figcraft_pixel::Sprite {
    figcraft_pixel::store::load(&s.doc().unwrap().node(NodeId(id)).unwrap().plugin_data).unwrap()
}

/// The box of the board the layer shows outside Pixel mode (art pixels).
fn shown_box(s: &Session, id: u64) -> [u32; 4] {
    figcraft_pixel::store::shown_box(&s.doc().unwrap().node(NodeId(id)).unwrap().plugin_data, &sprite(s, id))
}

/// The layer's box on the page: x, y, width, height.
fn page_box(s: &Session, id: u64) -> (f64, f64, f64, f64) {
    let doc = s.doc().unwrap();
    let n = doc.node(NodeId(id)).unwrap();
    let t = doc.world_transform(NodeId(id)).as_coeffs();
    (t[4], t[5], n.width, n.height)
}

/// Every pixel of the board, frame 0 (layers merged), as `#rrggbb` or ".".
fn board_rows(s: &mut Session, id: u64) -> serde_json::Value {
    ex(s, "pixel.get", json!({"id": id, "pixels": true, "frame": 0}))["pictures"][0]["rows"].clone()
}

/// An 8 × 12 block of art at (4, 6), red with a blue top-left pixel.
fn draw_block(s: &mut Session, id: u64) {
    ex(s, "pixel.shape", json!({"id": id, "kind": "rect", "from": [4, 6], "to": [11, 17], "filled": true, "color": "#ff0000"}));
    ex(s, "pixel.stroke", json!({"id": id, "points": [[4, 6]], "color": "#0000ff"}));
}

#[test]
fn an_empty_board_keeps_the_boards_size() {
    let mut s = Session::new();
    let id = board(&mut s, 32, 32, 4);
    assert_eq!(page_box(&s, id), (100.0, 50.0, 128.0, 128.0));
    assert_eq!(shown_box(&s, id), [0, 0, 32, 32]);
}

#[test]
fn the_layer_hugs_its_used_pixels_where_they_are_and_the_board_stays_whole() {
    let mut s = Session::new();
    let id = board(&mut s, 32, 32, 4);
    let empty = board_rows(&mut s, id);
    draw_block(&mut s, id);
    // 8 × 12 art pixels at (4, 6), scale 4: 32 × 48 points at (116, 74).
    assert_eq!(page_box(&s, id), (116.0, 74.0, 32.0, 48.0));
    let sp = sprite(&s, id);
    assert_eq!((sp.width, sp.height), (32, 32), "the board keeps its size");
    assert_eq!(shown_box(&s, id), [4, 6, 8, 12]);
    let rows = board_rows(&mut s, id);
    assert_eq!(rows.as_array().unwrap().len(), 32, "the board's art is all there");
    assert_eq!(rows[6][4], "#0000ff");
    // Undo goes back to the empty board, at the board's size and place.
    ex(&mut s, "edit.undo", json!({}));
    ex(&mut s, "edit.undo", json!({}));
    assert_eq!(page_box(&s, id), (100.0, 50.0, 128.0, 128.0));
    assert_eq!(board_rows(&mut s, id), empty);
}

#[test]
fn drawing_further_out_grows_the_box_and_the_art_never_moves_on_the_page() {
    let mut s = Session::new();
    let id = board(&mut s, 32, 32, 4);
    draw_block(&mut s, id);
    ex(&mut s, "pixel.stroke", json!({"id": id, "points": [[0, 0]], "color": "#00ff00"}));
    assert_eq!(page_box(&s, id), (100.0, 50.0, 48.0, 72.0));
    // Erased again: it hugs the block once more.
    ex(&mut s, "pixel.stroke", json!({"id": id, "points": [[0, 0]], "erase": true}));
    assert_eq!(page_box(&s, id), (116.0, 74.0, 32.0, 48.0));
    // Everything erased: back to the whole board.
    ex(&mut s, "pixel.clear", json!({"id": id}));
    assert_eq!(page_box(&s, id), (100.0, 50.0, 128.0, 128.0));
}

#[test]
fn the_image_fill_shows_the_trimmed_art() {
    let mut s = Session::new();
    let id = board(&mut s, 32, 32, 4);
    draw_block(&mut s, id);
    let doc = s.doc().unwrap();
    let n = doc.node(NodeId(id)).unwrap();
    let hash = n
        .fills
        .iter()
        .find_map(|f| match &f.kind {
            figcraft_color::PaintKind::Image(img) => Some(img.image.clone()),
            _ => None,
        })
        .unwrap();
    let img = doc.images.get(&hash).unwrap();
    assert_eq!((img.width, img.height), (8, 12));
}

#[test]
fn a_bigger_board_keeps_the_layer_where_its_art_is() {
    let mut s = Session::new();
    let id = board(&mut s, 32, 32, 4);
    draw_block(&mut s, id);
    ex(&mut s, "pixel.canvas", json!({"id": id, "width": 48, "height": 40, "anchor": "br"}));
    assert_eq!(page_box(&s, id), (116.0, 74.0, 32.0, 48.0), "the art stays on the page");
    assert_eq!(shown_box(&s, id), [20, 14, 8, 12]);
}

#[test]
fn a_layer_resized_in_design_keeps_its_size_when_its_pixels_change() {
    let mut s = Session::new();
    let id = board(&mut s, 32, 32, 4);
    draw_block(&mut s, id);
    // Grown in Design (its art regrows): drawing again leaves its box alone.
    ex(&mut s, "transform.resize", json!({"ids": [id], "width": 64, "height": 48}));
    ex(&mut s, "pixel.stroke", json!({"id": id, "points": [[0, 0]], "color": "#00ff00"}));
    assert_eq!(page_box(&s, id), (116.0, 74.0, 64.0, 48.0));
}

#[test]
fn nine_slice_cuts_are_measured_on_the_trimmed_art_and_an_instance_keeps_its_corners() {
    let mut s = Session::new();
    let id = board(&mut s, 32, 32, 4);
    draw_block(&mut s, id);
    ex(&mut s, "game.nineSlice.set", json!({"ids": [id], "enabled": true}));
    let n = s.doc().unwrap().node(NodeId(id)).unwrap().clone();
    let slice = figcraft_doc::nine_slice::NineSlice::of(&n).unwrap();
    // A quarter of the trimmed art's shorter side (8 art pixels): 2 art pixels, 8 points.
    assert_eq!(slice.insets, [8.0; 4]);
    assert_eq!(slice.base, (32.0, 48.0));
    // A component of it, an instance resized: its corners stay (the blue pixel at the top left).
    let main = ex(&mut s, "component.create", json!({"ids": [id]}))["id"].as_u64().unwrap();
    let inst = ex(&mut s, "instance.create", json!({"component": main, "x": 400, "y": 0}))["id"].as_u64().unwrap();
    ex(&mut s, "transform.resize", json!({"ids": [inst], "width": 96, "height": 96}));
    let doc = s.doc().unwrap();
    let px = figcraft_render::render_area(doc, &[NodeId(inst)], figcraft_geom::Rect::new(400.0, 0.0, 496.0, 96.0), 1.0).unwrap();
    let at = |x: u32, y: u32| {
        let p = px.pixels()[(y * px.width() + x) as usize].demultiply();
        [p.red(), p.green(), p.blue(), p.alpha()]
    };
    assert_eq!(at(1, 1), [0, 0, 255, 255], "the top-left corner stays");
    assert_eq!(at(94, 94), [255, 0, 0, 255]);
}

/// G16.8: inside an auto layout the layer shows the whole board instead of hugging. Hugging moves
/// the layer to keep its art still and the layout puts it straight back, so the art jumped on the
/// page every time the sprite was drawn on.
#[test]
fn a_sprite_an_auto_layout_places_shows_its_whole_board_and_its_art_never_moves() {
    let mut s = Session::new();
    let row = ex(&mut s, "frame.create", json!({"x": 0, "y": 0, "width": 300, "height": 200, "name": "Row"}))["id"].as_u64().unwrap();
    ex(&mut s, "node.set", json!({"ids": [row], "props": {"layout": "horizontal", "padding": [10, 10, 10, 10], "gap": 8}}));
    let id = ex(&mut s, "pixel.create", json!({"parent": row, "width": 16, "height": 16, "scale": 4}))["id"].as_u64().unwrap();
    // Art in the middle of the board, then a pixel further up and left than all of it.
    ex(&mut s, "pixel.shape", json!({"id": id, "kind": "rect", "from": [4, 4], "to": [11, 11], "filled": true, "color": "#ff0000"}));
    assert_eq!(shown_box(&s, id), [0, 0, 16, 16], "the whole board, not the art's box");
    assert_eq!(page_box(&s, id), (10.0, 10.0, 64.0, 64.0));
    ex(&mut s, "pixel.stroke", json!({"id": id, "points": [[0, 0]], "color": "#0000ff"}));
    assert_eq!(shown_box(&s, id), [0, 0, 16, 16]);
    assert_eq!(page_box(&s, id), (10.0, 10.0, 64.0, 64.0), "nothing moved or resized");
    // Erased again: still the whole board, still in the row.
    ex(&mut s, "pixel.stroke", json!({"id": id, "points": [[0, 0]], "erase": true}));
    assert_eq!(page_box(&s, id), (10.0, 10.0, 64.0, 64.0));
    assert_eq!(s.doc().unwrap().node(NodeId(id)).unwrap().parent, Some(NodeId(row)), "still in the auto layout");
    assert_eq!(s.doc().unwrap().node(NodeId(row)).unwrap().children, vec![NodeId(id)]);
}

/// A layer told to ignore the auto layout is placed by itself again, so it hugs as usual.
#[test]
fn a_sprite_that_ignores_the_auto_layout_hugs_its_art_again() {
    let mut s = Session::new();
    let row = ex(&mut s, "frame.create", json!({"x": 0, "y": 0, "width": 300, "height": 200}))["id"].as_u64().unwrap();
    ex(&mut s, "node.set", json!({"ids": [row], "props": {"layout": "horizontal", "padding": [10, 10, 10, 10]}}));
    let id = ex(&mut s, "pixel.create", json!({"parent": row, "width": 16, "height": 16, "scale": 4}))["id"].as_u64().unwrap();
    ex(&mut s, "node.set", json!({"ids": [id], "props": {"absolute": true}}));
    ex(&mut s, "pixel.shape", json!({"id": id, "kind": "rect", "from": [4, 4], "to": [11, 11], "filled": true, "color": "#ff0000"}));
    assert_eq!(shown_box(&s, id), [4, 4, 8, 8], "it hugs its art");
    let (_, _, w, h) = page_box(&s, id);
    assert_eq!((w, h), (32.0, 32.0));
}
