//! What a sprite is made of: its size, its pixel scale, its layers and frames, its animation tags
//! and its own palette. The pixels themselves (one cel per layer and frame) live beside it
//! ([`crate::store`]).

use serde::{Deserialize, Serialize};

use crate::canvas::{Canvas, parse_hex};
use crate::resize::{Resize, SliceFill};

/// The most layers a sprite has.
pub const MAX_LAYERS: usize = 64;
/// The most frames a sprite has.
pub const MAX_FRAMES: usize = 1024;
/// The most tags a sprite has.
pub const MAX_TAGS: usize = 256;
/// The most colours in a sprite's own palette.
pub const MAX_PALETTE: usize = 256;
/// The largest pixel scale (canvas pixels per art pixel).
pub const MAX_SCALE: u32 = 64;
/// A frame's shortest and longest time on screen (milliseconds).
pub const DURATION_RANGE: (u32, u32) = (10, 60_000);
/// The most times a tag plays before it stops (0 loops forever).
pub const MAX_REPEAT: u32 = 10_000;
const MAX_NAME: usize = 200;

fn one() -> u32 {
    1
}
fn yes() -> bool {
    true
}
fn full() -> u8 {
    255
}
fn hundred() -> u32 {
    100
}

/// A layer of the sprite (bottom first in [`Sprite::layers`]).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Layer {
    pub id: u32,
    pub name: String,
    #[serde(default = "yes")]
    pub visible: bool,
    /// 0–255.
    #[serde(default = "full")]
    pub opacity: u8,
}

/// A frame of the animation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Frame {
    pub id: u32,
    /// How long it shows, in milliseconds.
    #[serde(default = "hundred")]
    pub duration: u32,
}

/// Which way a tag plays.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Direction {
    #[default]
    Forward,
    Reverse,
    PingPong,
}

/// A named run of frames (`from` to `to`, both included), such as `idle` or `hover`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tag {
    pub name: String,
    pub from: usize,
    pub to: usize,
    #[serde(default)]
    pub direction: Direction,
    /// How many times it plays before it stops on its last frame; 0 loops forever.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub repeat: u32,
}

fn is_zero(v: &u32) -> bool {
    *v == 0
}

/// A sprite's description (its pixels are stored per cel).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sprite {
    pub width: u32,
    pub height: u32,
    /// Canvas pixels per art pixel when the layer is at its own size.
    #[serde(default = "one")]
    pub scale: u32,
    pub layers: Vec<Layer>,
    pub frames: Vec<Frame>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<Tag>,
    /// The tag the layer plays in Prototype and Play (G16.4); None plays every frame.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub play: Option<String>,
    /// The sprite's own colours, as `#rrggbb` or `#rrggbbaa`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub palette: Vec<String>,
    /// Where its colours came from (G16.6): `#rrggbb[aa]` → `style:<id>` (a colour style) or
    /// `var:<id>` (a colour variable), so they can follow the style when it changes.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub links: std::collections::BTreeMap<String, String>,
    /// The id the next layer or frame gets.
    #[serde(default)]
    pub next_id: u32,
    /// How the art grows or shrinks without 9-slice when the layer is resized (G16.3).
    #[serde(default)]
    pub resize: Resize,
    /// How its 9-slice edges and middle fill their space.
    #[serde(default)]
    pub slice_fill: SliceFill,
    /// Whether a frame with auto layout takes the 9-slice cuts as its padding.
    #[serde(default = "yes")]
    pub slice_padding: bool,
}

impl Sprite {
    /// A blank sprite `width` × `height` at `scale`: one layer, one frame. None for a size the
    /// sprite can't have.
    pub fn new(width: u32, height: u32, scale: u32) -> Option<Self> {
        Canvas::new(width, height)?;
        Some(Self {
            width,
            height,
            scale: scale.clamp(1, MAX_SCALE),
            layers: vec![Layer { id: 1, name: "Layer 1".into(), visible: true, opacity: 255 }],
            frames: vec![Frame { id: 2, duration: 100 }],
            tags: Vec::new(),
            play: None,
            palette: Vec::new(),
            links: std::collections::BTreeMap::new(),
            next_id: 3,
            resize: Resize::default(),
            slice_fill: SliceFill::default(),
            slice_padding: true,
        })
    }

    /// A fresh id for a new layer or frame.
    pub fn take_id(&mut self) -> u32 {
        let id = self.next_id.max(1);
        self.next_id = id.saturating_add(1);
        id
    }

    /// The sprite kept within its limits: a size it can have, at least one layer and frame, ids
    /// unique, durations, tags and palette in range. None when its size can't be mended.
    pub fn sanitized(mut self) -> Option<Self> {
        Canvas::new(self.width, self.height)?;
        self.scale = self.scale.clamp(1, MAX_SCALE);
        let mut ids = std::collections::BTreeSet::new();
        self.layers.retain(|l| l.id != 0 && ids.insert(l.id));
        self.layers.truncate(MAX_LAYERS);
        self.frames.retain(|f| f.id != 0 && ids.insert(f.id));
        self.frames.truncate(MAX_FRAMES);
        let max_id = ids.iter().copied().max().unwrap_or(0);
        self.next_id = self.next_id.max(max_id.saturating_add(1));
        if self.layers.is_empty() {
            let id = self.take_id();
            self.layers.push(Layer { id, name: "Layer 1".into(), visible: true, opacity: 255 });
        }
        if self.frames.is_empty() {
            let id = self.take_id();
            self.frames.push(Frame { id, duration: 100 });
        }
        for l in &mut self.layers {
            l.name = clip(&l.name);
        }
        for f in &mut self.frames {
            f.duration = f.duration.clamp(DURATION_RANGE.0, DURATION_RANGE.1);
        }
        let last = self.frames.len().saturating_sub(1);
        self.tags.retain(|t| t.from <= t.to && t.from <= last);
        for t in &mut self.tags {
            t.to = t.to.min(last);
            t.name = clip(&t.name);
            t.repeat = t.repeat.min(MAX_REPEAT);
        }
        self.tags.truncate(MAX_TAGS);
        if self.play.as_deref().is_some_and(|p| self.tag(p).is_none()) {
            self.play = None;
        }
        self.palette.retain(|c| parse_hex(c).is_some());
        self.palette.truncate(MAX_PALETTE);
        Some(self)
    }

    /// The index of layer `id`.
    pub fn layer_index(&self, id: u32) -> Option<usize> {
        self.layers.iter().position(|l| l.id == id)
    }

    /// The index of frame `id`.
    pub fn frame_index(&self, id: u32) -> Option<usize> {
        self.frames.iter().position(|f| f.id == id)
    }

    /// The tag called `name`.
    pub fn tag(&self, name: &str) -> Option<&Tag> {
        self.tags.iter().find(|t| t.name == name)
    }
    /// A new empty layer named `name` (or "Layer N") at `index` (bottom 0; past the top: on top).
    /// Its id, or None when the sprite has [`MAX_LAYERS`] already.
    pub fn add_layer(&mut self, name: Option<&str>, index: usize) -> Option<u32> {
        if self.layers.len() >= MAX_LAYERS {
            return None;
        }
        let id = self.take_id();
        let name = name.map_or_else(|| format!("Layer {}", self.layers.len() + 1), clip);
        self.layers.insert(index.min(self.layers.len()), Layer { id, name, visible: true, opacity: 255 });
        Some(id)
    }

    /// Takes layer `id` out (never the last one). Whether it went.
    pub fn remove_layer(&mut self, id: u32) -> bool {
        if self.layers.len() <= 1 {
            return false;
        }
        let Some(i) = self.layer_index(id) else { return false };
        self.layers.remove(i);
        true
    }

    /// Moves layer `id` to `index` (bottom 0). Whether it moved.
    pub fn move_layer(&mut self, id: u32, index: usize) -> bool {
        let Some(i) = self.layer_index(id) else { return false };
        let to = index.min(self.layers.len().saturating_sub(1));
        if i == to {
            return false;
        }
        let l = self.layers.remove(i);
        self.layers.insert(to, l);
        true
    }

    /// A new frame at `index` (past the end: last) lasting `duration` ms. A tag that holds the frame
    /// before it grows to take it in; tags after it move along. Its id, or None at [`MAX_FRAMES`].
    pub fn insert_frame(&mut self, index: usize, duration: u32) -> Option<u32> {
        if self.frames.len() >= MAX_FRAMES {
            return None;
        }
        let i = index.min(self.frames.len());
        let id = self.take_id();
        self.frames.insert(i, Frame { id, duration: duration.clamp(DURATION_RANGE.0, DURATION_RANGE.1) });
        for t in &mut self.tags {
            if i > 0 && t.from < i && t.to >= i - 1 {
                t.to += 1;
            } else if t.from >= i {
                t.from += 1;
                t.to += 1;
            }
        }
        Some(id)
    }

    /// Takes frame `index` out (never the last one). Tags close up over it; a tag of only that frame
    /// goes. Whether it went.
    pub fn remove_frame(&mut self, index: usize) -> bool {
        if self.frames.len() <= 1 || index >= self.frames.len() {
            return false;
        }
        self.frames.remove(index);
        self.tags.retain(|t| !(t.from == index && t.to == index));
        for t in &mut self.tags {
            if t.from > index {
                t.from -= 1;
            }
            if t.to >= index && t.to > 0 {
                t.to -= 1;
            }
        }
        if self.play.as_deref().is_some_and(|p| self.tag(p).is_none()) {
            self.play = None;
        }
        true
    }

    /// Moves frame `from` to `to` (indices). Tags keep their places, so the frame leaves or joins
    /// them as it moves. Whether it moved.
    pub fn move_frame(&mut self, from: usize, to: usize) -> bool {
        let to = to.min(self.frames.len().saturating_sub(1));
        if from >= self.frames.len() || from == to {
            return false;
        }
        let f = self.frames.remove(from);
        self.frames.insert(to, f);
        true
    }
}

/// `s` cut to a name's longest length (at a character boundary).
fn clip(s: &str) -> String {
    s.chars().take(MAX_NAME).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_sprite_has_a_layer_and_a_frame() {
        let s = Sprite::new(16, 8, 4).unwrap();
        assert_eq!((s.layers.len(), s.frames.len(), s.scale), (1, 1, 4));
        assert_ne!(s.layers[0].id, s.frames[0].id);
        assert!(Sprite::new(0, 8, 1).is_none());
        assert_eq!(Sprite::new(8, 8, 1000).unwrap().scale, MAX_SCALE);
    }

    #[test]
    fn a_damaged_sprite_is_mended() {
        let json = r##"{"width": 8, "height": 8, "scale": 0, "layers": [{"id": 5, "name": "a"}, {"id": 5, "name": "dup"}],
            "frames": [{"id": 5}, {"id": 9, "duration": 1}], "tags": [{"name": "t", "from": 0, "to": 99}, {"name": "bad", "from": 3, "to": 1}],
            "palette": ["#ff0000", "nope"], "nextId": 2}"##;
        let s: Sprite = serde_json::from_str(json).unwrap();
        let s = s.sanitized().unwrap();
        assert_eq!(s.scale, 1);
        assert_eq!(s.layers.len(), 1);
        assert_eq!(s.frames.iter().map(|f| f.id).collect::<Vec<_>>(), vec![9], "frame 5 clashes with layer 5");
        assert_eq!(s.frames[0].duration, DURATION_RANGE.0);
        assert_eq!(s.tags.len(), 1);
        assert_eq!(s.tags[0].to, 0);
        assert_eq!(s.palette, vec!["#ff0000".to_string()]);
        assert!(s.next_id > 9);
        let empty: Sprite = serde_json::from_str(r#"{"width": 4, "height": 4, "layers": [], "frames": []}"#).unwrap();
        let empty = empty.sanitized().unwrap();
        assert_eq!((empty.layers.len(), empty.frames.len()), (1, 1));
        let huge: Sprite = serde_json::from_str(r#"{"width": 99999, "height": 4, "layers": [], "frames": []}"#).unwrap();
        assert!(huge.sanitized().is_none());
    }

    #[test]
    fn layers_are_added_moved_and_removed_but_one_always_stays() {
        let mut s = Sprite::new(4, 4, 1).unwrap();
        let a = s.add_layer(None, 99).unwrap();
        let b = s.add_layer(Some("Shadow"), 0).unwrap();
        let names = |s: &Sprite| s.layers.iter().map(|l| l.name.clone()).collect::<Vec<_>>();
        assert_eq!(names(&s), ["Shadow", "Layer 1", "Layer 2"]);
        assert!(s.move_layer(b, 5));
        assert_eq!(names(&s), ["Layer 1", "Layer 2", "Shadow"]);
        assert!(!s.move_layer(b, 2), "already there");
        assert!(s.remove_layer(a) && s.remove_layer(b));
        assert!(!s.remove_layer(1), "the last layer stays");
        assert_ne!(a, b);
    }
}

#[cfg(test)]
mod frame_tests {
    use super::*;

    fn tag(name: &str, from: usize, to: usize) -> Tag {
        Tag { name: name.into(), from, to, direction: Direction::Forward, repeat: 0 }
    }

    fn ranges(s: &Sprite) -> Vec<(usize, usize)> {
        s.tags.iter().map(|t| (t.from, t.to)).collect()
    }

    #[test]
    fn a_new_frame_joins_the_tag_of_the_frame_before_it_and_moves_later_tags_along() {
        let mut s = Sprite::new(2, 2, 1).unwrap();
        for i in 1..4 {
            s.insert_frame(i, 100).unwrap();
        }
        s.tags = vec![tag("idle", 0, 1), tag("hover", 2, 3)];
        // After frame 1 (idle's last): idle grows, hover moves along.
        s.insert_frame(2, 80).unwrap();
        assert_eq!(ranges(&s), [(0, 2), (3, 4)]);
        assert_eq!(s.frames[2].duration, 80);
        // At the very start: everything moves along.
        s.insert_frame(0, 100).unwrap();
        assert_eq!(ranges(&s), [(1, 3), (4, 5)]);
        // Past the end: the last frame, the tag holding the old last frame takes it in.
        let id = s.insert_frame(99, 5).unwrap();
        assert_eq!(s.frames.last().map(|f| (f.id, f.duration)), Some((id, DURATION_RANGE.0)));
        assert_eq!(ranges(&s), [(1, 3), (4, 6)]);
    }

    #[test]
    fn a_deleted_frame_closes_its_tags_and_a_tag_of_only_it_goes() {
        let mut s = Sprite::new(2, 2, 1).unwrap();
        for i in 1..5 {
            s.insert_frame(i, 100).unwrap();
        }
        s.tags = vec![tag("a", 0, 1), tag("b", 2, 2), tag("c", 3, 4)];
        s.play = Some("b".into());
        assert!(s.remove_frame(2));
        assert_eq!(ranges(&s), [(0, 1), (2, 3)]);
        assert_eq!(s.play, None, "the tag it played went");
        assert!(s.remove_frame(0));
        assert_eq!(ranges(&s), [(0, 0), (1, 2)]);
        assert!(!s.remove_frame(9));
        while s.frames.len() > 1 {
            assert!(s.remove_frame(0));
        }
        assert!(!s.remove_frame(0), "the last frame stays");
    }

    #[test]
    fn a_moved_frame_keeps_its_id_and_tags_keep_their_places() {
        let mut s = Sprite::new(2, 2, 1).unwrap();
        for i in 1..3 {
            s.insert_frame(i, 100).unwrap();
        }
        s.tags = vec![tag("a", 0, 1)];
        let ids: Vec<u32> = s.frames.iter().map(|f| f.id).collect();
        assert!(s.move_frame(0, 2));
        assert_eq!(s.frames.iter().map(|f| f.id).collect::<Vec<_>>(), [ids[1], ids[2], ids[0]]);
        assert_eq!(ranges(&s), [(0, 1)]);
        assert!(!s.move_frame(1, 1));
        assert!(s.move_frame(0, 99), "past the end: last");
        assert_eq!(s.frames.last().map(|f| f.id), Some(ids[1]));
    }

    #[test]
    fn a_tag_naming_play_and_repeat_survive_mending() {
        let json = r#"{"width": 2, "height": 2, "layers": [{"id": 1, "name": "a"}], "frames": [{"id": 2}, {"id": 3}],
            "tags": [{"name": "idle", "from": 0, "to": 1, "repeat": 3}], "play": "idle"}"#;
        let s: Sprite = serde_json::from_str(json).unwrap();
        let s = s.sanitized().unwrap();
        assert_eq!((s.play.as_deref(), s.tags[0].repeat), (Some("idle"), 3));
        let json = r#"{"width": 2, "height": 2, "layers": [], "frames": [], "play": "gone"}"#;
        let s: Sprite = serde_json::from_str(json).unwrap();
        assert_eq!(s.sanitized().unwrap().play, None);
    }
}
