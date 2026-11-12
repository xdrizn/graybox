//! Pixel art (G16.5): `pixel.export` (files written or handed back) and `pixel.import` (a PNG or a
//! sheet as a new pixel layer).

use base64::Engine as _;
use serde_json::{Value, json};

use super::{ex, refused, sprite};
use crate::Session;
use figcraft_doc::NodeId;

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn unb64(v: &Value) -> Vec<u8> {
    base64::engine::general_purpose::STANDARD.decode(v.as_str().unwrap()).unwrap()
}

/// The exported files (name → bytes) handed back without a folder.
fn exported(s: &mut Session, p: Value) -> Vec<(String, Vec<u8>)> {
    ex(s, "pixel.export", p)["files"].as_array().unwrap().iter().map(|f| (f["name"].as_str().unwrap().to_string(), unb64(&f["data"]))).collect()
}

fn json_file(files: &[(String, Vec<u8>)]) -> Value {
    serde_json::from_slice(&files.iter().find(|(n, _)| n.ends_with(".json")).unwrap().1).unwrap()
}

#[test]
fn a_sprite_exports_a_sheet_with_its_frames_tags_and_9_slice() {
    let (mut s, id) = sprite();
    s.execute("node.set", &json!({"id": id, "name": "Coin/Spin"})).unwrap();
    ex(&mut s, "pixel.stroke", json!({"id": id, "points": [[0, 0]], "color": "#c83447"}));
    ex(&mut s, "pixel.frame.add", json!({"id": id, "copy": 0}));
    ex(&mut s, "pixel.frame.add", json!({"id": id}));
    ex(&mut s, "pixel.tag.add", json!({"id": id, "from": 0, "to": 1, "name": "spin"}));
    // A 9-slice 8 page units (2 art pixels at scale 4) in from each side.
    ex(&mut s, "game.nineSlice.set", json!({"ids": [id], "insets": [8, 8, 8, 8]}));
    let files = exported(&mut s, json!({"id": id, "format": "unity", "scale": 2}));
    let names: Vec<&str> = files.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, ["Coin_Spin.png", "Coin_Spin.json"], "the layer's name, safe for a file");
    let j = json_file(&files);
    assert_eq!(j["frames"].as_array().unwrap().len(), 3);
    assert_eq!(j["frames"][0]["frame"], j["frames"][1]["frame"], "the duplicate frame is stored once");
    assert_eq!(j["meta"]["frameTags"][0]["name"], "spin");
    assert_eq!(j["frames"][0]["border"], json!({"left": 4, "bottom": 4, "right": 4, "top": 4}));
    assert_eq!(j["meta"]["unity"]["filterMode"], "Point");
}

#[test]
fn export_writes_into_a_folder_and_can_leave_hidden_layers_out_or_take_them() {
    let (mut s, id) = sprite();
    ex(&mut s, "pixel.stroke", json!({"id": id, "points": [[0, 0]], "color": "#c83447"}));
    let top = ex(&mut s, "pixel.layer.add", json!({"id": id}))["layer"].as_u64().unwrap();
    ex(&mut s, "pixel.stroke", json!({"id": id, "points": [[1, 0]], "color": "#2040c0", "layer": top}));
    ex(&mut s, "pixel.layer.set", json!({"id": id, "layer": top, "visible": false}));
    let shown = exported(&mut s, json!({"id": id, "format": "png", "trim": true}));
    let pic = figcraft_pixel::Canvas::from_png(&shown[0].1).unwrap();
    assert_eq!((pic.width(), pic.height()), (1, 1), "trimmed to the shown layer's one pixel");
    let both = exported(&mut s, json!({"id": id, "format": "png", "trim": true, "layers": [top, 1]}));
    assert_eq!(figcraft_pixel::Canvas::from_png(&both[0].1).unwrap().width(), 2, "a picked hidden layer is exported");
    let dir = std::env::temp_dir().join(format!("graybox-pixel-export-{}", std::process::id()));
    let r = ex(&mut s, "pixel.export", json!({"id": id, "format": "frames", "dir": dir.to_string_lossy(), "name": "dot"}));
    assert_eq!(r["files"], json!(["dot_0.png"]));
    assert!(dir.join("dot_0.png").exists());
    std::fs::remove_dir_all(&dir).ok();
    assert!(refused(&mut s, "pixel.export", json!({"id": id, "format": "psd"})).contains("format"));
    assert!(refused(&mut s, "pixel.export", json!({"id": id, "tag": "nope"})).contains("nope"));
    assert!(refused(&mut s, "pixel.export", json!({"id": id, "layers": [99]})).contains("layer"));
}

#[test]
fn a_sheet_imports_as_a_pixel_layer_with_a_frame_per_cell() {
    let (mut s, id) = sprite();
    // Export a 3-frame row, then read it back cut by the frame size.
    ex(&mut s, "pixel.stroke", json!({"id": id, "points": [[0, 0]], "color": "#c83447"}));
    ex(&mut s, "pixel.frame.add", json!({"id": id}));
    ex(&mut s, "pixel.stroke", json!({"id": id, "points": [[7, 7]], "color": "#2040c0", "frame": 1}));
    ex(&mut s, "pixel.frame.add", json!({"id": id, "copy": 0}));
    let sheet = exported(&mut s, json!({"id": id, "format": "sheet", "layout": "row"}));
    let png = &sheet.iter().find(|(n, _)| n.ends_with(".png")).unwrap().1;
    // Packed shares the two same frames; a row keeps one cell each: 24 × 8.
    assert_eq!(figcraft_pixel::Canvas::from_png(png).map(|c| (c.width(), c.height())), Some((24, 8)));
    let undo = s.undo_labels(1000).len();
    let r = ex(&mut s, "pixel.import", json!({"data": b64(png), "frameWidth": 8, "name": "Walk", "duration": 80, "scale": 2}));
    assert_eq!(r["frames"], 3);
    assert_eq!(s.undo_labels(1000).len(), undo + 1, "one undo step");
    let new = r["id"].as_u64().unwrap();
    let got = ex(&mut s, "pixel.get", json!({"id": new}));
    assert_eq!((got["width"].clone(), got["height"].clone(), got["scale"].clone()), (json!(8), json!(8), json!(2)));
    assert_eq!(got["frames"][2]["duration"], 80);
    assert_eq!(ex(&mut s, "pixel.pick", json!({"id": new, "x": 7, "y": 7, "frame": 1}))["color"], "#2040c0");
    assert_eq!(s.doc().unwrap().node(figcraft_doc::NodeId(new)).unwrap().name, "Walk");
    assert!(refused(&mut s, "pixel.import", json!({"data": "bm90IGEgcG5n"})).contains("PNG"));
    assert!(refused(&mut s, "pixel.import", json!({})).contains("path"));
}

#[test]
fn a_screen_sent_to_unity_marks_pixel_layers_and_writes_a_sheet_for_animated_ones() {
    let dir = std::env::temp_dir().join(format!("graybox-pixel-unity-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut s = Session::new();
    let screen = ex(&mut s, "frame.create", json!({"x": 0, "y": 0, "width": 200, "height": 100, "name": "HUD"}))["id"].as_u64().unwrap();
    let still = ex(&mut s, "pixel.create", json!({"parent": screen, "x": 10, "y": 10, "width": 4, "height": 4, "scale": 4}))["id"].as_u64().unwrap();
    let coin = ex(&mut s, "pixel.create", json!({"parent": screen, "x": 50, "y": 10, "width": 4, "height": 4, "scale": 4}))["id"].as_u64().unwrap();
    ex(&mut s, "pixel.stroke", json!({"id": coin, "points": [[0, 0]], "color": "#e8b030"}));
    ex(&mut s, "pixel.frame.add", json!({"id": coin}));
    ex(&mut s, "pixel.tag.add", json!({"id": coin, "from": 0, "to": 1, "name": "spin"}));
    ex(&mut s, "pixel.set", json!({"id": coin, "play": "spin"}));
    let out = ex(&mut s, "game.unity.export", json!({"frame": screen, "dir": dir.to_string_lossy(), "scale": 2}));
    let folder = std::path::PathBuf::from(out["screens"][0]["folder"].as_str().unwrap());
    let layout: Value = serde_json::from_slice(&std::fs::read(folder.join("layout.json")).unwrap()).unwrap();
    let kids = layout["root"]["children"].as_array().unwrap();
    let node = |id: u64| kids.iter().find(|k| k["id"] == json!(id)).unwrap();
    assert_eq!(node(still)["pixel"], true);
    assert!(node(still).get("animation").is_none(), "one frame: no sheet");
    let anim = node(coin)["animation"].as_str().unwrap();
    let sheet: Value = serde_json::from_slice(&std::fs::read(folder.join(anim)).unwrap()).unwrap();
    assert_eq!(sheet["meta"]["unity"]["play"], "spin");
    // Scale 4 on a screen exported at 2: each art pixel is 8 pixels in the sheet.
    assert_eq!(sheet["frames"][0]["sourceSize"], json!({"w": 32, "h": 32}));
    assert!(folder.join("sprites").join(sheet["meta"]["image"].as_str().unwrap()).exists());
    let _ = std::fs::remove_dir_all(&dir);
}

/// A pixel button on a screen goes to Unity as its 9-sliced art (the sprite's own size, no label
/// baked in) with the label as a text child, so the button grows in Unity like in Graybox.
#[test]
fn a_pixel_button_sent_to_unity_keeps_its_label_out_of_the_9_sliced_art() {
    let dir = std::env::temp_dir().join(format!("graybox-pixel-button-unity-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut s = Session::new();
    let screen = ex(&mut s, "frame.create", json!({"x": 0, "y": 0, "width": 400, "height": 200, "name": "Menu"}))["id"].as_u64().unwrap();
    let set = ex(&mut s, "pixel.button", json!({"x": 600, "y": 0, "text": "Start game"}))["id"].as_u64().unwrap();
    let main = s.doc().unwrap().node(NodeId(set)).unwrap().children[0];
    let inst = ex(&mut s, "instance.create", json!({"component": main.0, "x": 20, "y": 20, "parent": screen}))["id"].as_u64().unwrap();
    let out = ex(&mut s, "game.unity.export", json!({"frame": screen, "dir": dir.to_string_lossy()}));
    let folder = std::path::PathBuf::from(out["screens"][0]["folder"].as_str().unwrap());
    let layout: Value = serde_json::from_slice(&std::fs::read(folder.join("layout.json")).unwrap()).unwrap();
    let button = layout["root"]["children"].as_array().unwrap().iter().find(|k| k["id"] == json!(inst)).unwrap().clone();
    assert_eq!(button["pixel"], true);
    assert!(button["border"].is_array(), "9-sliced");
    let label = button["children"].as_array().expect("the label is a child").iter().find(|c| c["text"].is_object()).expect("a text child");
    assert_eq!(label["text"]["characters"], "Start game");
    // The sprite is the art at its own size (the starter's sprite), not the button's wider box.
    let png = image::open(folder.join(button["sprite"].as_str().unwrap())).unwrap();
    let doc = s.doc().unwrap();
    let art = figcraft_render::pixel::source(doc, doc.node(NodeId(inst)).unwrap()).and_then(|n| figcraft_pixel::store::load(&n.plugin_data)).unwrap();
    assert_eq!((png.width(), png.height()), (art.width * art.scale, art.height * art.scale));
    let _ = std::fs::remove_dir_all(&dir);
}
