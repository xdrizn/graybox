//! Pixel art (G16): the `pixel.*` commands, one undo step per stroke, saving and loading, and the
//! picture the layer's image fill shows.

use figcraft_color::PaintKind;
use figcraft_doc::NodeId;
use serde_json::{Value, json};

use crate::Session;
use crate::cmd::file::{file_bytes, load_bytes};

fn ex(s: &mut Session, command: &str, params: Value) -> Value {
    s.execute(command, &params).unwrap()
}

fn refused(s: &mut Session, command: &str, params: Value) -> String {
    s.execute(command, &params).unwrap_err().to_string()
}

/// A session with an 8×8 sprite at scale 4; its id.
fn sprite() -> (Session, u64) {
    let mut s = Session::new();
    let id = ex(&mut s, "pixel.create", json!({"x": 10, "y": 20, "width": 8, "height": 8, "scale": 4}))["id"].as_u64().unwrap();
    (s, id)
}

/// Frame 0 as rows, `#` for any painted pixel.
fn picture(s: &mut Session, id: u64) -> Vec<String> {
    let r = ex(s, "pixel.get", json!({"id": id, "pixels": true, "frame": 0}));
    r["pictures"][0]["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row.as_array().unwrap().iter().map(|c| if c == "." { '.' } else { '#' }).collect())
        .collect()
}

#[test]
fn a_new_sprite_is_a_layer_of_its_size_times_its_scale() {
    let (s, id) = sprite();
    let doc = s.doc().unwrap();
    let n = doc.node(NodeId(id)).unwrap();
    assert_eq!((n.width, n.height), (32.0, 32.0));
    assert_eq!(n.name, "Sprite 8×8");
    assert!(figcraft_pixel::store::is_pixel(&n.plugin_data));
    assert!(n.strokes.is_empty());
    assert!(matches!(n.fills.as_slice(), [f] if matches!(f.kind, PaintKind::Image(_))), "one image fill shows the art");
    assert_eq!(s.selection(), vec![NodeId(id)]);
}

#[test]
fn a_stroke_draws_and_is_one_undo_step() {
    let (mut s, id) = sprite();
    let r = ex(&mut s, "pixel.stroke", json!({"id": id, "points": [[0, 0], [3, 0], [3, 2]], "color": "#ff0000"}));
    assert_eq!(r["changed"], 6);
    assert_eq!(picture(&mut s, id)[..3], ["####....", "...#....", "...#...."]);
    assert_eq!(ex(&mut s, "pixel.pick", json!({"id": id, "x": 3, "y": 2}))["color"], "#ff0000");
    assert_eq!(ex(&mut s, "pixel.pick", json!({"id": id, "x": 7, "y": 7}))["color"], Value::Null);
    ex(&mut s, "edit.undo", json!({}));
    assert!(picture(&mut s, id).iter().all(|r| r == "........"));
    ex(&mut s, "edit.redo", json!({}));
    assert_eq!(picture(&mut s, id)[0], "####....");
}

#[test]
fn a_mirrored_pixel_perfect_stroke_with_a_round_brush() {
    let (mut s, id) = sprite();
    ex(&mut s, "pixel.stroke", json!({"id": id, "points": [[0, 0], [1, 0], [1, 1], [2, 1]], "pixelPerfect": true, "mirror": {"x": 4}}));
    assert_eq!(picture(&mut s, id)[..2], ["#......#", ".##..##."], "the L corner at (1, 0) goes");
    let (mut s, id) = sprite();
    ex(&mut s, "pixel.stroke", json!({"id": id, "points": [[3, 3]], "size": 3, "shape": "circle"}));
    assert_eq!(picture(&mut s, id)[2..5], ["...#....", "..###...", "...#...."]);
}

#[test]
fn shapes_fill_and_erase() {
    let (mut s, id) = sprite();
    ex(&mut s, "pixel.shape", json!({"id": id, "kind": "rect", "from": [1, 1], "to": [5, 4]}));
    assert_eq!(picture(&mut s, id)[1..5], [".#####..", ".#...#..", ".#...#..", ".#####.."]);
    ex(&mut s, "pixel.fill", json!({"id": id, "x": 3, "y": 2, "color": "#00ff00"}));
    assert_eq!(ex(&mut s, "pixel.pick", json!({"id": id, "x": 3, "y": 2}))["color"], "#00ff00");
    assert_eq!(ex(&mut s, "pixel.pick", json!({"id": id, "x": 0, "y": 0}))["color"], Value::Null, "the outside stays empty");
    ex(&mut s, "pixel.shape", json!({"id": id, "kind": "line", "from": [0, 7], "to": [7, 7], "erase": false}));
    ex(&mut s, "pixel.stroke", json!({"id": id, "points": [[0, 7], [7, 7]], "erase": true}));
    assert_eq!(picture(&mut s, id)[7], "........");
    ex(&mut s, "pixel.shape", json!({"id": id, "kind": "ellipse", "from": [0, 0], "to": [7, 7], "filled": true}));
    assert_eq!(picture(&mut s, id)[3], "########");
    ex(&mut s, "pixel.clear", json!({"id": id}));
    assert!(picture(&mut s, id).iter().all(|r| r == "........"));
}

#[test]
fn the_image_fill_follows_the_art() {
    let (mut s, id) = sprite();
    let image = |s: &Session| match &s.doc().unwrap().node(NodeId(id)).unwrap().fills[0].kind {
        PaintKind::Image(i) => i.image.clone(),
        _ => panic!("no image fill"),
    };
    let before = image(&s);
    ex(&mut s, "pixel.stroke", json!({"id": id, "points": [[2, 2]], "color": "#123456"}));
    let after = image(&s);
    assert_ne!(before, after);
    let bytes = s.doc().unwrap().images[&after].bytes.clone();
    let c = figcraft_pixel::Canvas::from_png(&bytes).unwrap();
    assert_eq!((c.width(), c.height()), (8, 8), "one image pixel per art pixel");
    assert_eq!(c.get(2, 2), Some([0x12, 0x34, 0x56, 255]));
}

#[test]
fn a_sprite_survives_saving_and_loading() {
    let (mut s, id) = sprite();
    ex(&mut s, "pixel.stroke", json!({"id": id, "points": [[0, 0], [7, 7]], "color": "#abcdef80"}));
    let before = picture(&mut s, id);
    let bytes = file_bytes(s.ds().unwrap()).unwrap();
    let (doc, _) = load_bytes(&bytes, "x.figcraft").unwrap();
    let mut t = Session::with_document(doc);
    assert_eq!(picture(&mut t, id), before);
    assert_eq!(ex(&mut t, "pixel.pick", json!({"id": id, "x": 4, "y": 4}))["color"], "#abcdef80");
}

#[test]
fn a_background_colour_and_scale() {
    let mut s = Session::new();
    let id = ex(&mut s, "pixel.create", json!({"width": 2, "height": 3, "scale": 1, "background": "#ffffff"}))["id"].as_u64().unwrap();
    assert_eq!(picture(&mut s, id), ["##", "##", "##"]);
    ex(&mut s, "pixel.set", json!({"id": id, "scale": 5}));
    let n = s.doc().unwrap().node(NodeId(id)).unwrap();
    assert_eq!((n.width, n.height), (10.0, 15.0));
    assert_eq!(ex(&mut s, "pixel.get", json!({"id": id}))["scale"], 5);
}

#[test]
fn bad_requests_are_refused_with_a_reason() {
    let (mut s, id) = sprite();
    let rect = ex(&mut s, "shape.rectangle", json!({"x": 0, "y": 0, "width": 10, "height": 10}))["id"].as_u64().unwrap();
    assert!(refused(&mut s, "pixel.stroke", json!({"id": rect, "points": [[0, 0]]})).contains("not a pixel layer"));
    assert!(refused(&mut s, "pixel.stroke", json!({"id": id, "points": []})).contains("points"));
    assert!(refused(&mut s, "pixel.stroke", json!({"id": id, "points": [[0, 0]], "color": "plaid"})).contains("color"));
    assert!(refused(&mut s, "pixel.stroke", json!({"id": id, "points": [[0, 0]], "frame": 3})).contains("frame"));
    assert!(refused(&mut s, "pixel.stroke", json!({"id": id, "points": [[0, 0]], "layer": 999})).contains("layer"));
    assert!(refused(&mut s, "pixel.shape", json!({"id": id, "kind": "star", "from": [0, 0], "to": [1, 1]})).contains("kind"));
    assert!(refused(&mut s, "pixel.create", json!({"width": 5000})).contains("width"));
    assert!(refused(&mut s, "pixel.mode", json!({"id": rect})).contains("not a pixel layer"));
}

#[test]
fn pixel_mode_asks_the_ui() {
    let (mut s, id) = sprite();
    ex(&mut s, "pixel.mode", json!({}));
    let asked = std::mem::take(&mut s.ui_requests);
    assert!(asked.iter().any(|r| r.kind == "pixel.mode" && r.data["id"] == id && r.data["on"] == true));
}

#[test]
fn far_away_points_and_huge_brushes_do_not_hang() {
    let (mut s, id) = sprite();
    let r = ex(&mut s, "pixel.stroke", json!({"id": id, "points": [[-1e300, -1e300], [1e300, 1e300]], "size": 64}));
    assert!(r["changed"].as_u64().unwrap() > 0);
    ex(&mut s, "pixel.shape", json!({"id": id, "kind": "ellipse", "from": [-1e9, -1e9], "to": [1e9, 1e9], "filled": true}));
}

/// Every `pixel.*` command given junk on a real sprite (the sweep in `tests.rs` only has plain
/// layers) refuses or does something, never panics.
#[test]
fn every_pixel_command_survives_junk_on_a_sprite() {
    let junk = [
        json!({"points": [[1e308, "x"], [null]], "color": [1], "size": -5, "shape": 7, "mirror": {"x": "NaN", "y": [1]}, "layer": -1, "frame": 1e308}),
        json!({"points": "x", "color": {"r": "x"}, "size": 1e308, "shape": "round", "mirror": 5, "layer": 4294967296u64, "frame": -1, "tile": "yes"}),
        json!({"kind": [], "from": [1], "to": null, "filled": 3, "square": "x", "x": "NaN", "y": 1e308, "tolerance": -1e308, "contiguous": 2}),
        json!({"kind": "ellipse", "from": [-1e308, 1e308], "to": [1e308, -1e308], "mirror": {"x": 1e308, "y": -1e308}, "tile": true, "size": 64}),
        json!({"width": 1e308, "height": -1, "scale": 0.4, "background": 5, "parent": "x", "pixels": "yes", "on": [], "name": [1]}),
        json!({"x": 3, "y": 3, "color": "#00000000", "tolerance": 255, "contiguous": false, "tile": true, "pixels": true, "frame": 0}),
        json!({"points": [[0, 0]], "mirror": {"x": 0.25}, "erase": true, "pixelPerfect": "x"}),
        json!({"kind": "wand", "op": "intersect", "x": 0, "y": 0, "dx": 1e308, "dy": "x", "ids": [1e308, "x", -1], "outside": "no", "everywhere": 2}),
        json!({"kind": "invert", "op": 5, "png": "not base64 !", "source": -1, "index": -1e308, "opacity": 1e308, "visible": "x", "name": [1]}),
        json!({"op": "rotateCW", "png": "iVBORw0KGgo=", "source": 1, "index": 4294967296u64, "layer": 1, "colors": [1, "x"], "add": "#zz", "remove": 3}),
        json!({"op": "move", "dx": -1e308, "dy": 1e308, "text": "GIMP Palette\n999 -1 x\n", "format": "pdf", "from": "#ff0000", "to": [1]}),
        json!({"kind": "lasso", "points": [[0, 0], [1e9, 1e9], [-1e9, 3]], "op": "add", "x": 1e308, "tolerance": 255}),
    ];
    for spec in crate::cmd::all_commands().iter().filter(|c| c.id.starts_with("pixel.")) {
        for j in &junk {
            let (mut s, id) = sprite();
            let mut p = j.clone();
            p.as_object_mut().unwrap().insert("id".into(), json!(id));
            if let Err(crate::EngineError::Internal { cmd, msg }) = s.execute(spec.id, &p) {
                panic!("{cmd} panicked on {p}: {msg}");
            }
        }
    }
}

#[test]
fn strokes_do_not_pile_up_old_pictures_and_undo_brings_the_old_one_back() {
    let (mut s, id) = sprite();
    let image = |s: &Session| match &s.doc().unwrap().node(NodeId(id)).unwrap().fills[0].kind {
        PaintKind::Image(i) => i.image.clone(),
        _ => panic!("no image fill"),
    };
    let start = s.doc().unwrap().images.len();
    let mut seen = Vec::new();
    for x in 0..6 {
        ex(&mut s, "pixel.stroke", json!({"id": id, "points": [[x, 1]], "color": "#123456"}));
        seen.push(image(&s));
    }
    assert_eq!(s.doc().unwrap().images.len(), start, "one picture of the sprite at a time");
    ex(&mut s, "edit.undo", json!({}));
    let back = image(&s);
    assert_eq!(back, seen[4]);
    assert!(s.doc().unwrap().images.contains_key(&back), "undo brings the earlier picture back");
}

// G16.2: selection, transform, colour, sprite layers and the palette.

#[test]
fn a_rectangle_selection_keeps_strokes_inside_it_until_it_is_cleared() {
    let (mut s, id) = sprite();
    assert_eq!(ex(&mut s, "pixel.select", json!({"id": id, "kind": "rect", "from": [2, 0], "to": [4, 7]}))["count"], 24);
    ex(&mut s, "pixel.stroke", json!({"id": id, "points": [[0, 1], [7, 1]]}));
    assert_eq!(picture(&mut s, id)[1], "..###...");
    ex(&mut s, "pixel.select", json!({"id": id, "kind": "none"}));
    ex(&mut s, "pixel.stroke", json!({"id": id, "points": [[0, 2], [7, 2]]}));
    assert_eq!(picture(&mut s, id)[2], "########");
}

#[test]
fn the_wand_takes_one_colour_region_and_selections_combine() {
    let (mut s, id) = sprite();
    ex(&mut s, "pixel.shape", json!({"id": id, "kind": "rect", "from": [0, 0], "to": [3, 3], "filled": true, "color": "#ff0000"}));
    assert_eq!(ex(&mut s, "pixel.select", json!({"id": id, "kind": "wand", "x": 1, "y": 1}))["count"], 16);
    assert_eq!(ex(&mut s, "pixel.select", json!({"id": id, "kind": "invert"}))["count"], 48);
    assert_eq!(ex(&mut s, "pixel.select", json!({"id": id, "kind": "rect", "from": [0, 0], "to": [0, 0], "op": "add"}))["count"], 49);
    assert_eq!(
        ex(&mut s, "pixel.select", json!({"id": id, "kind": "rect", "from": [0, 0], "to": [7, 0], "op": "subtract"}))["count"],
        44,
        "row 0 held (0, 0) and four more"
    );
    assert_eq!(
        ex(&mut s, "pixel.select", json!({"id": id, "kind": "rect", "from": [0, 0], "to": [7, 4], "op": "intersect"}))["count"],
        20,
        "rows 1–3 right of the square, and row 4"
    );
}

#[test]
fn flipping_and_moving_the_selected_pixels_is_one_undo_step_each() {
    let (mut s, id) = sprite();
    ex(&mut s, "pixel.stroke", json!({"id": id, "points": [[0, 0], [2, 0]]}));
    ex(&mut s, "pixel.transform", json!({"id": id, "op": "flipH"}));
    assert_eq!(picture(&mut s, id)[0], ".....###", "nothing selected: the whole cel flips");
    ex(&mut s, "pixel.select", json!({"id": id, "kind": "rect", "from": [5, 0], "to": [6, 0]}));
    ex(&mut s, "pixel.transform", json!({"id": id, "op": "move", "dx": -5, "dy": 1}));
    assert_eq!(picture(&mut s, id)[..2], [".......#", "##......"]);
    let sel = ex(&mut s, "pixel.select", json!({"id": id, "kind": "rect", "from": [0, 1], "to": [1, 1], "op": "intersect"}));
    assert_eq!(sel["count"], 2, "the selection moved with the pixels");
    ex(&mut s, "edit.undo", json!({}));
    ex(&mut s, "edit.undo", json!({}));
    assert_eq!(picture(&mut s, id)[..2], [".....###", "........"]);
}

#[test]
fn rotating_a_square_selection_turns_it_a_quarter() {
    let (mut s, id) = sprite();
    ex(&mut s, "pixel.stroke", json!({"id": id, "points": [[0, 0], [2, 0]]}));
    ex(&mut s, "pixel.select", json!({"id": id, "kind": "rect", "from": [0, 0], "to": [2, 2]}));
    ex(&mut s, "pixel.transform", json!({"id": id, "op": "rotateCW"}));
    assert_eq!(picture(&mut s, id)[..3], ["..#.....", "..#.....", "..#....."]);
}

#[test]
fn an_outline_rings_the_art_outside_its_edge() {
    let (mut s, id) = sprite();
    ex(&mut s, "pixel.stroke", json!({"id": id, "points": [[3, 3]], "color": "#ff0000"}));
    let r = ex(&mut s, "pixel.outline", json!({"id": id, "color": "#000000"}));
    assert_eq!(r["changed"], 4);
    assert_eq!(picture(&mut s, id)[2..5], ["...#....", "..###...", "...#...."]);
    assert_eq!(ex(&mut s, "pixel.pick", json!({"id": id, "x": 3, "y": 2}))["color"], "#000000");
}

#[test]
fn replacing_a_colour_changes_every_layer_of_every_sprite_given() {
    let (mut s, a) = sprite();
    let b = ex(&mut s, "pixel.create", json!({"x": 100, "y": 20, "width": 4, "height": 4}))["id"].as_u64().unwrap();
    ex(&mut s, "pixel.stroke", json!({"id": a, "points": [[0, 0]], "color": "#ff0000"}));
    let top = ex(&mut s, "pixel.layer.add", json!({"id": a}))["layer"].clone();
    ex(&mut s, "pixel.stroke", json!({"id": a, "points": [[1, 0]], "color": "#ff0000", "layer": top}));
    ex(&mut s, "pixel.stroke", json!({"id": b, "points": [[2, 2]], "color": "#ff0000"}));
    let r = ex(&mut s, "pixel.replaceColor", json!({"ids": [a, b], "from": "#ff0000", "to": "#0000ff"}));
    assert_eq!(r["changed"], 3);
    for (id, x, y) in [(a, 0, 0), (a, 1, 0), (b, 2, 2)] {
        assert_eq!(ex(&mut s, "pixel.pick", json!({"id": id, "x": x, "y": y}))["color"], "#0000ff");
    }
    ex(&mut s, "edit.undo", json!({}));
    assert_eq!(ex(&mut s, "pixel.pick", json!({"id": b, "x": 2, "y": 2}))["color"], "#ff0000", "one undo step for all");
}

#[test]
fn copied_pixels_paste_elsewhere_and_become_the_selection() {
    let (mut s, id) = sprite();
    ex(&mut s, "pixel.stroke", json!({"id": id, "points": [[0, 0], [1, 0]]}));
    ex(&mut s, "pixel.select", json!({"id": id, "kind": "rect", "from": [0, 0], "to": [1, 0]}));
    let c = ex(&mut s, "pixel.copy", json!({"id": id}));
    assert_eq!((c["width"].as_u64(), c["height"].as_u64(), c["x"].as_i64()), (Some(2), Some(1), Some(0)));
    let r = ex(&mut s, "pixel.paste", json!({"id": id, "png": c["png"], "x": 5, "y": 6}));
    assert_eq!((r["width"].as_u64(), r["height"].as_u64()), (Some(2), Some(1)));
    assert_eq!(picture(&mut s, id)[6], ".....##.");
    assert_eq!(ex(&mut s, "pixel.select", json!({"id": id, "kind": "invert"}))["count"], 62, "the paste is selected");
}

#[test]
fn sprite_layers_stack_hide_reorder_merge_and_never_run_out() {
    let (mut s, id) = sprite();
    let bottom = ex(&mut s, "pixel.get", json!({"id": id}))["layers"][0]["id"].as_u64().unwrap();
    ex(&mut s, "pixel.stroke", json!({"id": id, "points": [[0, 0]], "color": "#ff0000"}));
    let top = ex(&mut s, "pixel.layer.add", json!({"id": id, "name": "Ink"}))["layer"].as_u64().unwrap();
    ex(&mut s, "pixel.stroke", json!({"id": id, "points": [[0, 0], [1, 0]], "color": "#0000ff", "layer": top}));
    assert_eq!(ex(&mut s, "pixel.pick", json!({"id": id, "x": 0, "y": 0}))["color"], "#0000ff", "the top layer shows");
    ex(&mut s, "pixel.layer.set", json!({"id": id, "layer": top, "visible": false}));
    assert_eq!(picture(&mut s, id)[0], "#.......");
    ex(&mut s, "pixel.layer.set", json!({"id": id, "layer": top, "visible": true}));
    ex(&mut s, "pixel.layer.move", json!({"id": id, "layer": top, "index": 0}));
    assert_eq!(ex(&mut s, "pixel.pick", json!({"id": id, "x": 0, "y": 0}))["color"], "#ff0000", "the red layer is on top now");
    ex(&mut s, "pixel.layer.merge", json!({"id": id, "layer": bottom}));
    let got = ex(&mut s, "pixel.get", json!({"id": id}));
    assert_eq!(got["layers"].as_array().unwrap().len(), 1);
    assert_eq!(picture(&mut s, id)[0], "##......");
    let only = got["layers"][0]["id"].clone();
    assert!(refused(&mut s, "pixel.layer.delete", json!({"id": id, "layer": only})).contains("one layer"));
}

#[test]
fn a_palette_loads_from_gpl_and_hex_and_exports_again() {
    let (mut s, id) = sprite();
    let gpl = "GIMP Palette\nName: Test\n#\n255   0   0\tRed\n  0 255   0\tGreen\n";
    let r = ex(&mut s, "pixel.palette.set", json!({"id": id, "text": gpl}));
    assert_eq!(r["palette"], json!(["#ff0000", "#00ff00"]));
    ex(&mut s, "pixel.palette.set", json!({"id": id, "add": "#0000ff"}));
    ex(&mut s, "pixel.palette.set", json!({"id": id, "remove": "#ff0000"}));
    assert_eq!(ex(&mut s, "pixel.palette.export", json!({"id": id, "format": "hex"}))["text"], "00ff00\n0000ff\n");
    let back = ex(&mut s, "pixel.palette.export", json!({"id": id}))["text"].as_str().unwrap().to_string();
    assert!(back.starts_with("GIMP Palette"));
    assert_eq!(ex(&mut s, "pixel.palette.set", json!({"id": id, "text": "ff00ff\n"}))["palette"], json!(["#ff00ff"]));
}

#[test]
fn a_layer_pastes_in_at_the_sprite_scale_and_a_huge_one_is_refused() {
    let (mut s, id) = sprite();
    let small = ex(&mut s, "shape.rectangle", json!({"x": 200, "y": 0, "width": 8, "height": 12}))["id"].as_u64().unwrap();
    let r = ex(&mut s, "pixel.paste", json!({"id": id, "source": small, "x": 1, "y": 1}));
    assert_eq!((r["width"].as_u64(), r["height"].as_u64()), (Some(2), Some(3)), "8 × 12 page units at scale 4");
    assert_eq!(picture(&mut s, id)[1..5], [".##.....", ".##.....", ".##.....", "........"]);
    let huge = ex(&mut s, "shape.rectangle", json!({"x": 0, "y": 500, "width": 60000, "height": 40}))["id"].as_u64().unwrap();
    assert!(refused(&mut s, "pixel.paste", json!({"id": id, "source": huge})).contains("1024"));
}
