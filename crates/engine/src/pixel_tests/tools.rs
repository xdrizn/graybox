//! Pixel mode's helpers that live in the engine (G16.6 follow-ups): the timeline toggle the app
//! remembers and colours taken from the file's colour styles.

use serde_json::json;

use super::{ex, refused};
use crate::Session;

/// The timeline over Pixel mode's pill starts off, is turned on and off by a command (the pill's
/// button, its shortcut and the Actions palette run it) and the app keeps the choice.
#[test]
fn the_timeline_toggle_starts_off_and_is_kept_in_the_preferences() {
    let mut s = Session::new();
    assert!(!crate::cmd::pixel::timeline_shown(&s), "off at first: a still sprite takes no room");
    s.prefs_unsaved = false;
    assert_eq!(ex(&mut s, "pixel.timeline", json!({}))["on"], true);
    assert!(crate::cmd::pixel::timeline_shown(&s));
    assert!(s.prefs_unsaved, "kept at once");
    assert_eq!(ex(&mut s, "pixel.timeline", json!({}))["on"], false, "a toggle");
    assert_eq!(ex(&mut s, "pixel.timeline", json!({"on": true}))["on"], true);
    assert_eq!(ex(&mut s, "pixel.timeline", json!({"on": true}))["on"], true, "set, not toggled");
    assert!(s.undo_labels(10).is_empty(), "not an edit of the file");
    assert!(refused(&mut s, "pixel.timeline", json!({"on": [1]})).contains("on"));
}

fn style(s: &mut Session, name: &str, color: &str) -> String {
    ex(s, "style.create", json!({"type": "paint", "name": name, "value": color}))["id"].as_str().unwrap().to_string()
}

fn links(s: &Session, id: u64) -> std::collections::BTreeMap<String, String> {
    let n = s.doc().unwrap().node(figcraft_doc::NodeId(id)).unwrap();
    figcraft_pixel::store::load(&n.plugin_data).unwrap().links
}

fn at(s: &mut Session, id: u64, x: i64, y: i64) -> Option<String> {
    ex(s, "pixel.pick", json!({"id": id, "x": x, "y": y}))["color"].as_str().map(str::to_string)
}

/// Pixel art paints with the file's colour styles and variables: a stroke records which style its
/// colour came from, and when the style changes, Update from styles recolours those pixels in
/// every layer and frame (one undo step) and keeps the link.
#[test]
fn colours_painted_from_a_style_follow_it() {
    let (mut s, id) = super::sprite();
    let gold = style(&mut s, "Coin/Gold", "#e8b830");
    ex(&mut s, "pixel.stroke", json!({"id": id, "points": [[0, 0], [3, 0]], "color": "#e8b830", "link": format!("style:{gold}")}));
    ex(&mut s, "pixel.frame.add", json!({"id": id, "copy": 0}));
    assert_eq!(links(&s, id).get("#e8b830").map(String::as_str), Some(format!("style:{gold}").as_str()));
    // A link whose colour isn't the one painted is not kept (it would recolour the wrong pixels).
    ex(&mut s, "pixel.stroke", json!({"id": id, "points": [[0, 2]], "color": "#123456", "link": format!("style:{gold}")}));
    assert!(!links(&s, id).contains_key("#123456"));
    ex(&mut s, "style.update", json!({"style": gold, "value": ["#ffd040"]}));
    let r = ex(&mut s, "pixel.styles.follow", json!({}));
    assert_eq!(r["changed"], 8, "4 pixels in each of 2 frames");
    assert_eq!(at(&mut s, id, 2, 0).as_deref(), Some("#ffd040"));
    let frame1 = ex(&mut s, "pixel.pick", json!({"id": id, "x": 2, "y": 0, "frame": 1}));
    assert_eq!(frame1["color"], "#ffd040", "every frame");
    assert!(links(&s, id).contains_key("#ffd040") && !links(&s, id).contains_key("#e8b830"), "the link moved with it");
    assert_eq!(ex(&mut s, "pixel.styles.follow", json!({}))["changed"], 0, "nothing left to follow");
    s.undo().unwrap();
    s.undo().unwrap();
    assert_eq!(at(&mut s, id, 2, 0).as_deref(), Some("#e8b830"), "one undo step each");
    // A bad link is refused, not stored.
    assert!(refused(&mut s, "pixel.stroke", json!({"id": id, "points": [[0, 0]], "link": "style:nope"})).contains("link"));
}

/// Replacing a colour with a style's colour links the new colour; the old link goes.
#[test]
fn replace_colour_moves_the_link() {
    let (mut s, id) = super::sprite();
    let a = style(&mut s, "A", "#ff0000");
    let b = style(&mut s, "B", "#00ff00");
    ex(&mut s, "pixel.stroke", json!({"id": id, "points": [[0, 0]], "color": "#ff0000", "link": format!("style:{a}")}));
    ex(&mut s, "pixel.replaceColor", json!({"ids": [id], "from": "#ff0000", "to": "#00ff00", "link": format!("style:{b}")}));
    let l = links(&s, id);
    assert_eq!(l.get("#00ff00").map(String::as_str), Some(format!("style:{b}").as_str()));
    assert!(!l.contains_key("#ff0000"));
}

/// A `.gpl` or `.hex` palette becomes colour styles in a folder (one undo step), which every tool
/// can then paint with.
#[test]
fn a_palette_file_becomes_colour_styles() {
    let mut s = Session::new();
    let gpl = "GIMP Palette\nName: Dusk\n#\n 30  20  60\tNight\n 90  60 140\tViolet\n200 170 240\n";
    let r = ex(&mut s, "pixel.palette.import", json!({"text": gpl, "name": "Dusk"}));
    assert_eq!(r["styles"].as_array().unwrap().len(), 3);
    let names: Vec<String> = s.doc().unwrap().styles.iter().map(|st| st.name.clone()).collect();
    assert_eq!(names, ["Dusk/Night", "Dusk/Violet", "Dusk/3"]);
    assert_eq!(s.undo_labels(10).len(), 1, "one step");
    assert!(refused(&mut s, "pixel.palette.import", json!({"text": "nothing here"})).contains("colour"));
}

/// Shade steps along the ramp it is given (the colour style folder the colour came from).
#[test]
fn shade_follows_the_ramp_it_is_given() {
    let (mut s, id) = super::sprite();
    ex(&mut s, "pixel.paint", json!({"id": id, "rows": ["mm"], "palette": {"m": "#5a3c8c"}}));
    let ramp = ["#1e143c", "#5a3c8c", "#c8aaf0"];
    ex(&mut s, "pixel.stroke", json!({"id": id, "points": [[0, 0]], "shade": 1, "ramp": ramp}));
    ex(&mut s, "pixel.stroke", json!({"id": id, "points": [[1, 0]], "shade": -1, "ramp": ramp}));
    assert_eq!(at(&mut s, id, 0, 0).as_deref(), Some("#c8aaf0"));
    assert_eq!(at(&mut s, id, 1, 0).as_deref(), Some("#1e143c"));
    assert!(refused(&mut s, "pixel.stroke", json!({"id": id, "points": [[0, 0]], "shade": 1, "ramp": "red"})).contains("ramp"));
}

/// With colours taken from styles, a sprite has no palette of its own: Save palette writes the
/// colours it is drawn with.
#[test]
fn save_palette_writes_the_colours_drawn_with_when_there_is_no_palette() {
    let mut s = Session::new();
    let id = ex(&mut s, "pixel.create", json!({"x": 0, "y": 0, "width": 8, "height": 8}))["id"].as_u64().unwrap();
    ex(&mut s, "pixel.stroke", json!({"id": id, "points": [[1, 1]], "color": "#112233"}));
    ex(&mut s, "pixel.stroke", json!({"id": id, "points": [[2, 2]], "color": "#445566"}));
    let text = ex(&mut s, "pixel.palette.export", json!({"id": id, "format": "hex"}))["text"].as_str().unwrap().to_string();
    assert_eq!(text.split_whitespace().collect::<Vec<_>>(), vec!["112233", "445566"]);
}
