//! Pixel art (G16.3): 9-slice in whole art pixels, its cuts as auto-layout padding, Pixel text, the
//! Pixel button starter, palette-swap variants, and instances that follow their main's pixels.

use figcraft_doc::nine_slice::NineSlice;
use figcraft_doc::{NodeId, NodeKind};
use figcraft_pixel::store;
use serde_json::json;

use super::{ex, refused, sprite};
use crate::Session;

fn slice(s: &Session, id: u64) -> Option<NineSlice> {
    NineSlice::of(s.doc().unwrap().node(NodeId(id)).unwrap())
}

fn padding(s: &Session, id: u64) -> [f64; 4] {
    s.doc().unwrap().node(NodeId(id)).unwrap().layout.as_ref().unwrap().padding
}

fn id_of(v: &serde_json::Value) -> u64 {
    v["id"].as_u64().unwrap()
}

#[test]
fn a_pixel_layer_nine_slices_in_whole_art_pixels() {
    let (mut s, id) = sprite();
    ex(&mut s, "game.nineSlice.set", json!({"ids": [id], "enabled": true}));
    let on = slice(&s, id).unwrap();
    assert_eq!(on.insets, [8.0; 4], "a quarter of the 8-pixel side, 2 art pixels at scale 4");
    assert_eq!(on.base, (32.0, 32.0));
    ex(&mut s, "game.nineSlice.set", json!({"ids": [id], "insets": [5, 6, 3, 0]}));
    assert_eq!(slice(&s, id).unwrap().insets, [4.0, 8.0, 4.0, 0.0], "cuts snap to whole art pixels");
    // Resized, the base stays the sprite's own size.
    ex(&mut s, "transform.resize", json!({"ids": [id], "width": 80, "height": 40}));
    assert_eq!(slice(&s, id).unwrap().base, (32.0, 32.0));
    // A new scale keeps the cuts in art pixels.
    ex(&mut s, "pixel.set", json!({"id": id, "scale": 2}));
    let half = slice(&s, id).unwrap();
    assert_eq!((half.insets, half.base), ([2.0, 4.0, 2.0, 0.0], (16.0, 16.0)));
}

#[test]
fn resize_settings_are_kept_and_bad_ones_refused() {
    let (mut s, id) = sprite();
    let r = ex(&mut s, "pixel.set", json!({"id": id, "resize": "tile", "sliceFill": "stretch", "slicePadding": false}));
    assert_eq!((r["resize"].as_str(), r["sliceFill"].as_str(), r["slicePadding"].as_bool()), (Some("tile"), Some("stretch"), Some(false)));
    let sp = store::load(&s.doc().unwrap().node(NodeId(id)).unwrap().plugin_data).unwrap();
    assert_eq!(sp.resize, figcraft_pixel::resize::Resize::Tile);
    assert!(refused(&mut s, "pixel.set", json!({"id": id, "resize": "melt"})).contains("resize"));
    assert!(refused(&mut s, "pixel.set", json!({"id": id, "sliceFill": 3})).contains("sliceFill"));
}

/// A sprite drawn in a frame becomes the frame's background; with auto layout its 9-slice cuts are
/// the padding, so a label inside sits clear of the border.
#[test]
fn a_pixel_background_gives_its_cuts_as_auto_layout_padding() {
    let mut s = Session::new();
    let f = id_of(&ex(&mut s, "frame.create", json!({"width": 64, "height": 40})));
    let px = id_of(&ex(&mut s, "pixel.create", json!({"parent": f, "width": 8, "height": 8, "scale": 4})));
    ex(&mut s, "game.image.toBackground", json!({"ids": [px]}));
    let doc = s.doc().unwrap();
    assert!(doc.node(NodeId(px)).is_none());
    assert!(store::is_pixel(&doc.node(NodeId(f)).unwrap().plugin_data), "the frame carries the sprite");
    ex(&mut s, "node.set", json!({"ids": [f], "props": {"layout": "horizontal"}}));
    ex(&mut s, "game.nineSlice.set", json!({"ids": [f], "enabled": true}));
    assert_eq!(padding(&s, f), [8.0; 4]);
    ex(&mut s, "game.nineSlice.set", json!({"ids": [f], "insets": [12, 4, 4, 4]}));
    assert_eq!(padding(&s, f), [12.0, 4.0, 4.0, 4.0]);
    // Off: the padding is the frame's own again.
    ex(&mut s, "pixel.set", json!({"id": f, "slicePadding": false}));
    ex(&mut s, "game.nineSlice.set", json!({"ids": [f], "insets": [8, 8, 8, 8]}));
    assert_eq!(padding(&s, f), [12.0, 4.0, 4.0, 4.0]);
    // A frame that already has pixel art takes no second sprite.
    let more = id_of(&ex(&mut s, "pixel.create", json!({"parent": f, "width": 4, "height": 4})));
    assert!(refused(&mut s, "game.image.toBackground", json!({"ids": [more]})).contains("pixel"));
}

#[test]
fn pixel_text_snaps_to_the_scale_of_the_pixel_art_around_it() {
    let mut s = Session::new();
    let f = id_of(&ex(&mut s, "frame.create", json!({"width": 90, "height": 30})));
    let px = id_of(&ex(&mut s, "pixel.create", json!({"parent": f, "width": 30, "height": 10, "scale": 3})));
    ex(&mut s, "game.image.toBackground", json!({"ids": [px]}));
    let t = id_of(&ex(&mut s, "text.create", json!({"text": "Play", "x": 0, "y": 0, "parent": f})));
    let loose = id_of(&ex(&mut s, "text.create", json!({"text": "Free", "x": 0, "y": 100})));
    ex(&mut s, "pixel.text", json!({"ids": [t, loose], "on": true}));
    let key = |s: &Session, id: u64| s.doc().unwrap().node(NodeId(id)).unwrap().plugin_data.get("graybox.pixelText").cloned();
    assert_eq!(key(&s, t).as_deref(), Some("3"), "the frame's sprite scale");
    assert_eq!(key(&s, loose).as_deref(), Some("4"), "no pixel art around it: 4");
    ex(&mut s, "pixel.text", json!({"ids": [loose], "on": true, "scale": 2}));
    assert_eq!(key(&s, loose).as_deref(), Some("2"));
    ex(&mut s, "pixel.text", json!({"ids": [t], "on": false}));
    assert_eq!(key(&s, t), None);
    assert!(refused(&mut s, "pixel.text", json!({"ids": [f], "on": true})).contains("text"));
}

/// One command makes a component set of three pixel buttons (Default, Hover, Pressed): each a
/// frame with auto layout hugging a Pixel text label, its art 9-sliced, its cuts the padding.
#[test]
fn the_pixel_button_starter_makes_three_states_that_grow_with_their_label() {
    let mut s = Session::new();
    let before = s.undo_labels(1000).len();
    let set = id_of(&ex(&mut s, "pixel.button", json!({"x": 0, "y": 0})));
    assert_eq!(s.undo_labels(1000).len(), before + 1, "one undo step: {:?}", s.undo_labels(1000));
    let doc = s.doc().unwrap();
    let set_node = doc.node(NodeId(set)).unwrap();
    assert!(matches!(set_node.kind, NodeKind::ComponentSet { .. }));
    assert_eq!(set_node.name, "Pixel button");
    assert!(!set_node.clip, "a state that grows stays in view");
    let names: Vec<String> = set_node.children.iter().map(|c| doc.node(*c).unwrap().name.clone()).collect();
    assert_eq!(names, ["State=Default", "State=Hover", "State=Pressed"]);
    let first = set_node.children[0];
    let b = doc.node(first).unwrap();
    assert!(store::is_pixel(&b.plugin_data));
    let cut = NineSlice::of(b).unwrap().insets;
    assert_eq!(cut, [16.0; 4], "4 art pixels at scale 4");
    assert_eq!(b.layout.as_ref().unwrap().padding, [16.0; 4]);
    let label = b.children[0];
    assert!(doc.node(label).unwrap().plugin_data.contains_key("graybox.pixelText"));
    let w = b.width;
    assert!(w > 48.0, "it hugs the label");
    let boxes: Vec<(f64, f64)> = set_node.children.iter().map(|c| doc.node(*c).unwrap()).map(|n| (n.transform.as_coeffs()[5], n.height)).collect();
    for pair in boxes.windows(2) {
        assert!(pair[1].0 >= pair[0].0 + pair[0].1 + 8.0, "the states don't overlap: {boxes:?}");
    }
    ex(&mut s, "text.set", json!({"id": label.0, "characters": "Continue playing"}));
    assert!(s.doc().unwrap().node(first).unwrap().width > w + 40.0, "a longer label, a wider button");
    assert_eq!(s.selection(), vec![NodeId(set)]);
    ex(&mut s, "edit.undo", json!({}));
    assert!(s.doc().unwrap().node(NodeId(set)).is_some(), "the first undo takes back the label");
    ex(&mut s, "edit.undo", json!({}));
    assert!(s.doc().unwrap().node(NodeId(set)).is_none(), "the button is one undo step");
}

#[test]
fn a_palette_swap_becomes_a_variant_with_its_colours_replaced() {
    let mut s = Session::new();
    let set = id_of(&ex(&mut s, "pixel.button", json!({})));
    let default = s.doc().unwrap().node(NodeId(set)).unwrap().children[0].0;
    let before = s.undo_labels(1000).len();
    let r = ex(&mut s, "pixel.paletteVariant", json!({"id": default, "value": "Red", "swap": [["#3a63c4", "#c43a3a"]]}));
    assert_eq!(s.undo_labels(1000).len(), before + 1, "one undo step");
    let red = id_of(&r);
    assert!(r["changed"].as_u64().unwrap() > 0);
    let doc = s.doc().unwrap();
    assert_eq!(doc.node(NodeId(red)).unwrap().parent, Some(NodeId(set)), "a variant in the same set");
    assert!(doc.node(NodeId(red)).unwrap().name.contains("Palette=Red"));
    let colours = |s: &mut Session, id: u64| ex(s, "pixel.get", json!({"id": id, "pixels": true, "frame": 0}))["pictures"][0]["rows"].to_string();
    assert!(colours(&mut s, red).contains("#c43a3a") && !colours(&mut s, red).contains("#3a63c4"));
    assert!(colours(&mut s, default).contains("#3a63c4"), "the original keeps its colours");
    assert!(refused(&mut s, "pixel.paletteVariant", json!({"id": default, "value": "X", "swap": []})).contains("swap"));
}

/// An instance of a pixel component shows its main's pixels, so editing the main updates every
/// instance; the instance itself isn't drawn on.
#[test]
fn instances_follow_their_mains_pixels() {
    let mut s = Session::new();
    let set = id_of(&ex(&mut s, "pixel.button", json!({})));
    let main = s.doc().unwrap().node(NodeId(set)).unwrap().children[0];
    let inst = id_of(&ex(&mut s, "instance.create", json!({"component": main.0, "x": 300, "y": 0})));
    let doc = s.doc().unwrap();
    assert_eq!(figcraft_render::pixel::source(doc, doc.node(NodeId(inst)).unwrap()).map(|n| n.id), Some(main));
    assert!(refused(&mut s, "pixel.stroke", json!({"id": inst, "points": [[0, 0]]})).contains("instance"));
    assert!(refused(&mut s, "pixel.mode", json!({"id": inst})).contains("instance"));
}

#[test]
fn a_palette_swap_of_a_lone_component_makes_it_a_set_of_two() {
    let (mut s, id) = sprite();
    ex(&mut s, "pixel.fill", json!({"id": id, "x": 0, "y": 0, "color": "#ff0000"}));
    let comp = id_of(&ex(&mut s, "component.create", json!({"ids": [id]})));
    let red = id_of(&ex(&mut s, "pixel.paletteVariant", json!({"id": comp, "value": "Blue", "swap": [["#ff0000", "#0000ff"]]})));
    let doc = s.doc().unwrap();
    let set = doc.node(NodeId(red)).unwrap().parent.unwrap();
    assert!(matches!(doc.node(set).unwrap().kind, NodeKind::ComponentSet { .. }));
    assert_eq!(doc.node(set).unwrap().children.len(), 2);
}

/// Review fixes: the Pressed label drop survives new cuts, a frame's own 9-slice becomes one in art
/// pixels when a sprite moves in, an instance's pixels can be read, and swap variants get their own
/// names.
#[test]
fn cuts_keep_extra_padding_and_moved_slices_snap() {
    let mut s = Session::new();
    let set = id_of(&ex(&mut s, "pixel.button", json!({})));
    let pressed = s.doc().unwrap().node(NodeId(set)).unwrap().children[2].0;
    assert_eq!(padding(&s, pressed), [20.0, 16.0, 12.0, 16.0], "the label sits one art pixel lower");
    ex(&mut s, "game.nineSlice.set", json!({"ids": [pressed], "insets": [12, 12, 12, 12]}));
    assert_eq!(padding(&s, pressed), [16.0, 12.0, 8.0, 12.0], "still one art pixel lower");

    let f = id_of(&ex(&mut s, "frame.create", json!({"x": 500, "width": 200, "height": 100})));
    ex(&mut s, "game.nineSlice.set", json!({"ids": [f], "insets": [10, 10, 10, 10]}));
    let px = id_of(&ex(&mut s, "pixel.create", json!({"parent": f, "width": 8, "height": 8, "scale": 4})));
    ex(&mut s, "game.image.toBackground", json!({"ids": [px]}));
    let moved = slice(&s, f).unwrap();
    assert_eq!((moved.insets, moved.base), ([12.0; 4], (32.0, 32.0)), "10 snaps to 3 art pixels; the base is the sprite");
}

#[test]
fn an_instance_reads_as_its_main_and_swap_variants_get_their_own_names() {
    let mut s = Session::new();
    let set = id_of(&ex(&mut s, "pixel.button", json!({})));
    let main = s.doc().unwrap().node(NodeId(set)).unwrap().children[0].0;
    let inst = id_of(&ex(&mut s, "instance.create", json!({"component": main, "x": 400, "y": 0})));
    let rows = |s: &mut Session, id: u64| ex(s, "pixel.get", json!({"id": id, "pixels": true, "frame": 0}))["pictures"][0]["rows"].clone();
    assert_eq!(rows(&mut s, inst), rows(&mut s, main));
    let swap = json!({"id": main, "value": "Red", "swap": [["#3a63c4", "#c43a3a"]]});
    let a = id_of(&ex(&mut s, "pixel.paletteVariant", swap.clone()));
    let b = id_of(&ex(&mut s, "pixel.paletteVariant", swap));
    let doc = s.doc().unwrap();
    let (na, nb) = (&doc.node(NodeId(a)).unwrap().name, &doc.node(NodeId(b)).unwrap().name);
    assert!(na.contains("Palette=Red") && nb.contains("Palette=Red 2"), "{na} / {nb}");
}
