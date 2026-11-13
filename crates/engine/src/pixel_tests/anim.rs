//! Pixel art (G16.4): frames (new, duplicate, delete, move, durations), tags, what a layer plays,
//! and the frame the renderer shows at a moment.

use figcraft_doc::NodeId;
use figcraft_pixel::store;
use serde_json::json;

use super::{ex, refused, sprite};
use crate::Session;

fn frames(s: &mut Session, id: u64) -> Vec<u64> {
    ex(s, "pixel.get", json!({"id": id}))["frames"].as_array().unwrap().iter().map(|f| f["duration"].as_u64().unwrap()).collect()
}

/// Which frames have pixels (`#` for a frame with any, `.` for an empty one).
fn painted(s: &mut Session, id: u64) -> String {
    let n = frames(s, id).len();
    (0..n)
        .map(|i| {
            let r = ex(s, "pixel.get", json!({"id": id, "pixels": true, "frame": i}));
            let any = r["pictures"][0]["rows"].as_array().unwrap().iter().flat_map(|row| row.as_array().unwrap()).any(|c| c != ".");
            if any { '#' } else { '.' }
        })
        .collect()
}

fn dot(s: &mut Session, id: u64, frame: usize) {
    ex(s, "pixel.stroke", json!({"id": id, "points": [[frame, 0]], "color": "#c83447", "frame": frame}));
}

fn undo_len(s: &Session) -> usize {
    s.undo_labels(1000).len()
}

#[test]
fn frames_are_added_duplicated_moved_and_deleted_as_one_undo_step_each() {
    let (mut s, id) = sprite();
    dot(&mut s, id, 0);
    let undo = undo_len(&s);
    assert_eq!(ex(&mut s, "pixel.frame.add", json!({"id": id}))["frame"], 1, "an empty frame at the end");
    assert_eq!(undo_len(&s), undo + 1);
    assert_eq!(painted(&mut s, id), "#.");
    ex(&mut s, "pixel.frame.add", json!({"id": id, "index": 1, "copy": 0, "duration": 250}));
    assert_eq!(painted(&mut s, id), "##.", "the duplicate has the pixels");
    assert_eq!(frames(&mut s, id), [100, 250, 100]);
    ex(&mut s, "pixel.frame.move", json!({"id": id, "frame": 2, "index": 0}));
    assert_eq!(painted(&mut s, id), ".##");
    assert_eq!(frames(&mut s, id), [100, 100, 250]);
    ex(&mut s, "pixel.frame.delete", json!({"id": id, "frame": 0}));
    assert_eq!(painted(&mut s, id), "##");
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(painted(&mut s, id), ".##");
    // A deleted frame's cels leave the layer.
    ex(&mut s, "pixel.frame.delete", json!({"id": id, "frame": 2}));
    let data = &s.doc().unwrap().node(NodeId(id)).unwrap().plugin_data;
    assert_eq!(data.keys().filter(|k| k.starts_with(store::CEL_PREFIX)).count(), 1);
    assert!(refused(&mut s, "pixel.frame.delete", json!({"id": id, "frame": 5})).contains("frame"));
    ex(&mut s, "pixel.frame.delete", json!({"id": id, "frame": 0}));
    assert!(refused(&mut s, "pixel.frame.delete", json!({"id": id, "frame": 0})).contains("at least one frame"));
}

#[test]
fn several_frames_are_deleted_in_one_undo_step_and_never_all_of_them() {
    let (mut s, id) = sprite();
    for f in 1..4 {
        ex(&mut s, "pixel.frame.add", json!({"id": id}));
        dot(&mut s, id, f);
    }
    assert_eq!(painted(&mut s, id), ".###");
    let undo = undo_len(&s);
    ex(&mut s, "pixel.frame.delete", json!({"id": id, "frames": [3, 1, 1]}));
    assert_eq!(painted(&mut s, id), ".#");
    assert_eq!(undo_len(&s), undo + 1);
    assert!(refused(&mut s, "pixel.frame.delete", json!({"id": id, "frames": [0, 1]})).contains("at least one frame"));
    assert!(refused(&mut s, "pixel.frame.delete", json!({"id": id, "frames": []})).contains("which frames"));
    assert!(refused(&mut s, "pixel.frame.move", json!({"id": id, "frame": 1, "index": 1})).contains("already"));
    assert!(refused(&mut s, "pixel.set", json!({"id": id, "play": 3})).contains("play"));
}

#[test]
fn durations_are_set_for_one_frame_a_run_or_all_and_kept_in_range() {
    let (mut s, id) = sprite();
    ex(&mut s, "pixel.frame.add", json!({"id": id}));
    ex(&mut s, "pixel.frame.add", json!({"id": id}));
    ex(&mut s, "pixel.frame.set", json!({"id": id, "frame": 1, "duration": 40}));
    assert_eq!(frames(&mut s, id), [100, 40, 100]);
    ex(&mut s, "pixel.frame.set", json!({"id": id, "frames": [0, 2], "duration": 1}));
    assert_eq!(frames(&mut s, id), [10, 40, 10], "the shortest a frame lasts is 10 ms");
    ex(&mut s, "pixel.frame.set", json!({"id": id, "duration": 1e9}));
    assert_eq!(frames(&mut s, id), [60000, 60000, 60000]);
    assert!(refused(&mut s, "pixel.frame.set", json!({"id": id, "duration": "x"})).contains("duration"));
}

#[test]
fn tags_are_named_uniquely_follow_their_frames_and_choose_what_plays() {
    let (mut s, id) = sprite();
    for _ in 0..3 {
        ex(&mut s, "pixel.frame.add", json!({"id": id}));
    }
    assert_eq!(ex(&mut s, "pixel.tag.add", json!({"id": id, "from": 0, "to": 1, "name": "idle"}))["name"], "idle");
    assert_eq!(
        ex(&mut s, "pixel.tag.add", json!({"id": id, "from": 3, "to": 2, "name": "idle"}))["name"],
        "idle 2",
        "names stay unique; from and to come in order"
    );
    ex(&mut s, "pixel.tag.set", json!({"id": id, "tag": "idle 2", "name": "hover", "direction": "pingPong", "repeat": 2}));
    ex(&mut s, "pixel.set", json!({"id": id, "play": "hover"}));
    let r = ex(&mut s, "pixel.get", json!({"id": id}));
    assert_eq!(r["play"], "hover");
    assert_eq!(r["tags"][1], json!({"name": "hover", "from": 2, "to": 3, "direction": "pingPong", "repeat": 2}));
    // A new frame after idle's last one joins idle; hover moves along.
    ex(&mut s, "pixel.frame.add", json!({"id": id, "index": 2}));
    let r = ex(&mut s, "pixel.get", json!({"id": id}));
    assert_eq!((r["tags"][0]["to"].clone(), r["tags"][1]["from"].clone()), (json!(2), json!(3)));
    // Renaming the played tag keeps it played; deleting it plays every frame.
    ex(&mut s, "pixel.tag.set", json!({"id": id, "tag": "hover", "name": "Hover"}));
    assert_eq!(ex(&mut s, "pixel.get", json!({"id": id}))["play"], "Hover");
    ex(&mut s, "pixel.tag.delete", json!({"id": id, "tag": "Hover"}));
    assert!(ex(&mut s, "pixel.get", json!({"id": id})).get("play").is_none());
    assert!(refused(&mut s, "pixel.set", json!({"id": id, "play": "nope"})).contains("tag"));
    ex(&mut s, "pixel.set", json!({"id": id, "play": ""}));
    assert!(refused(&mut s, "pixel.tag.set", json!({"id": id, "tag": "gone"})).contains("gone"));
    assert!(refused(&mut s, "pixel.tag.add", json!({"id": id, "from": 0, "to": 99})).contains("frame"));
}

#[test]
fn the_layer_shows_its_played_tag_at_the_presenters_time_and_its_first_frame_at_rest() {
    let (mut s, id) = sprite();
    for f in 1..3 {
        ex(&mut s, "pixel.frame.add", json!({"id": id}));
        dot(&mut s, id, f);
    }
    ex(&mut s, "pixel.tag.add", json!({"id": id, "from": 1, "to": 2, "name": "blink"}));
    ex(&mut s, "pixel.set", json!({"id": id, "play": "blink"}));
    let doc = s.doc().unwrap();
    let n = doc.node(NodeId(id)).unwrap();
    let at = |t: f64| figcraft_render::pixel::shown(doc, n, t).unwrap().frame;
    assert_eq!((at(0.0), at(0.15), at(0.25)), (1, 2, 1), "the tag's frames, 100 ms each, looping");
    // The animation clock wakes the presenter when the frame changes.
    let page = n.parent.unwrap();
    let (_, next) = figcraft_render::images::animation_clock(doc, page, 0.15).unwrap();
    assert!((next - 0.05).abs() < 1e-9, "{next}");
    // The image fill (what exports and older apps see) is the played tag's first frame.
    let fill = n.fills.iter().find_map(|p| match &p.kind {
        figcraft_color::PaintKind::Image(img) => Some(img.image.clone()),
        _ => None,
    });
    let png = doc.images.get(&fill.unwrap()).unwrap().bytes.clone();
    let shown = figcraft_pixel::Canvas::from_png(&png).unwrap();
    // G16.7: the picture is cut round the pixels the frames use (the dots at x 1 and 2).
    assert_eq!((shown.width(), shown.height()), (2, 1));
    assert_eq!(shown.get(0, 0), Some([0xc8, 0x34, 0x47, 255]), "frame 1's dot");
    assert_eq!(shown.get(1, 0), Some([0, 0, 0, 0]), "not frame 2's");
}

#[test]
fn a_still_sprite_has_no_clock() {
    let (s, id) = sprite();
    let doc = s.doc().unwrap();
    let n = doc.node(NodeId(id)).unwrap();
    assert!(figcraft_render::images::animation_clock(doc, n.parent.unwrap(), 0.5).is_none());
    assert_eq!(figcraft_render::pixel::shown(doc, n, 9.0).unwrap().frame, 0);
}
