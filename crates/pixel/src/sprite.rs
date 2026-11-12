//! What a sprite is made of: its size, its pixel scale, its layers and frames, its animation tags
//! and its own palette. The pixels themselves (one cel per layer and frame) live beside it
//! ([`crate::store`]).

use serde::{Deserialize, Serialize};

use crate::canvas::{Canvas, parse_hex};

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
    /// The sprite's own colours, as `#rrggbb` or `#rrggbbaa`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub palette: Vec<String>,
    /// The id the next layer or frame gets.
    #[serde(default)]
    pub next_id: u32,
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
            palette: Vec::new(),
            next_id: 3,
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
        }
        self.tags.truncate(MAX_TAGS);
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
}
