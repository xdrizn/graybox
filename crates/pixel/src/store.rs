//! A sprite kept in a layer's plugin data: its description as JSON under [`KEY`] and each cel that
//! has pixels as a base64 PNG under `graybox.pixel.cel.<layer id>.<frame id>`. A transparent cel
//! has no key. Copies, duplicates, components and the clipboard carry plugin data, so they carry
//! the sprite.

use std::collections::BTreeMap;

use base64::Engine as _;

use crate::canvas::Canvas;
use crate::select::Mask;
use crate::sprite::Sprite;

/// The plugin data key of a sprite's description; a layer with it is a pixel layer.
pub const KEY: &str = "graybox.pixel";
/// The start of every cel's key.
pub const CEL_PREFIX: &str = "graybox.pixel.cel.";
/// The key of the sprite's selection (what the selection tools picked), when there is one.
pub const SELECTION_KEY: &str = "graybox.pixel.selection";

/// Plugin data, as a layer holds it.
pub type Data = BTreeMap<String, String>;

/// Whether `data` holds a sprite.
pub fn is_pixel(data: &Data) -> bool {
    data.contains_key(KEY)
}

/// The key of the cel of layer `layer` in frame `frame` (ids).
pub fn cel_key(layer: u32, frame: u32) -> String {
    format!("{CEL_PREFIX}{layer}.{frame}")
}

/// The sprite described in `data`, mended where damaged; None when there is none or it can't be
/// read.
pub fn load(data: &Data) -> Option<Sprite> {
    serde_json::from_str::<Sprite>(data.get(KEY)?).ok()?.sanitized()
}

/// Writes the description of `sprite` and drops the cels of layers and frames it no longer has.
pub fn save(data: &mut Data, sprite: &Sprite) {
    if let Ok(json) = serde_json::to_string(sprite) {
        data.insert(KEY.to_string(), json);
    }
    let keep: std::collections::BTreeSet<String> =
        sprite.layers.iter().flat_map(|l| sprite.frames.iter().map(move |f| cel_key(l.id, f.id))).collect();
    data.retain(|k, _| !k.starts_with(CEL_PREFIX) || keep.contains(k));
}

/// Removes the sprite and its cels: the layer is no longer a pixel layer.
pub fn remove(data: &mut Data) {
    data.retain(|k, _| k != KEY && k != SELECTION_KEY && !k.starts_with(CEL_PREFIX));
}

/// The stored text of a cel (or None when transparent).
pub fn encode_cel(cel: &Canvas) -> Option<String> {
    if cel.is_clear() {
        return None;
    }
    cel.to_png().map(|png| base64::engine::general_purpose::STANDARD.encode(png))
}

/// A cel from its stored text, fitted to `width` × `height` (from the top left).
pub fn decode_cel(text: &str, width: u32, height: u32) -> Option<Canvas> {
    let bytes = base64::engine::general_purpose::STANDARD.decode(text.trim()).ok()?;
    let c = Canvas::from_png(&bytes)?;
    if c.width() == width && c.height() == height { Some(c) } else { c.resized(width, height, 0, 0) }
}

/// The pixels of layer `layer` in frame `frame` (ids): transparent when the cel is empty or
/// damaged. None only for a size the sprite can't have.
pub fn cel(data: &Data, sprite: &Sprite, layer: u32, frame: u32) -> Option<Canvas> {
    data.get(&cel_key(layer, frame)).and_then(|t| decode_cel(t, sprite.width, sprite.height)).or_else(|| Canvas::new(sprite.width, sprite.height))
}

/// Stores the pixels of layer `layer` in frame `frame` (a transparent cel is removed).
pub fn put_cel(data: &mut Data, layer: u32, frame: u32, cel: &Canvas) {
    let key = cel_key(layer, frame);
    match encode_cel(cel) {
        Some(text) => {
            data.insert(key, text);
        }
        None => {
            data.remove(&key);
        }
    }
}

/// The sprite's selection (None when nothing is selected, or it no longer fits the sprite).
pub fn selection(data: &Data, sprite: &Sprite) -> Option<Mask> {
    data.get(SELECTION_KEY).and_then(|t| Mask::decode(t, sprite.width, sprite.height)).filter(|m| !m.is_empty())
}

/// Stores the selection (an empty one, or None, removes it).
pub fn put_selection(data: &mut Data, mask: Option<&Mask>) {
    match mask.filter(|m| !m.is_empty()) {
        Some(m) => {
            data.insert(SELECTION_KEY.to_string(), m.encode());
        }
        None => {
            data.remove(SELECTION_KEY);
        }
    }
}

/// Frame `index` with its visible layers drawn bottom to top at their opacity.
pub fn frame_image(data: &Data, sprite: &Sprite, index: usize) -> Option<Canvas> {
    frame_image_of(data, sprite, index, None)
}

/// Frame `index` with layers `only` (by id, shown or hidden) drawn bottom to top at their opacity,
/// or its visible layers when `only` is None.
pub fn frame_image_of(data: &Data, sprite: &Sprite, index: usize, only: Option<&[u32]>) -> Option<Canvas> {
    let frame = sprite.frames.get(index)?;
    let mut out = Canvas::new(sprite.width, sprite.height)?;
    let wanted = |l: &crate::Layer| only.map_or(l.visible, |ids| ids.contains(&l.id)) && l.opacity > 0;
    for l in sprite.layers.iter().filter(|l| wanted(l)) {
        if !data.contains_key(&cel_key(l.id, frame.id)) {
            continue;
        }
        if let Some(c) = cel(data, sprite, l.id, frame.id) {
            out.draw_over(&c, l.opacity);
        }
    }
    Some(out)
}

/// A content hash of what frame `index` shows (its description and the cels of its visible
/// layers), so a picture made from it can be kept while nothing it is made of changed.
pub fn frame_hash(data: &Data, sprite: &Sprite, index: usize) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (sprite.width, sprite.height, index).hash(&mut h);
    if let Some(frame) = sprite.frames.get(index) {
        for l in sprite.layers.iter().filter(|l| l.visible && l.opacity > 0) {
            (l.id, l.opacity).hash(&mut h);
            data.get(&cel_key(l.id, frame.id)).hash(&mut h);
        }
    }
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED: [u8; 4] = [255, 0, 0, 255];

    #[test]
    fn a_sprite_round_trips_through_plugin_data() {
        let mut data = Data::new();
        assert!(!is_pixel(&data));
        let s = Sprite::new(4, 3, 2).unwrap();
        save(&mut data, &s);
        assert!(is_pixel(&data));
        assert_eq!(load(&data), Some(s.clone()));
        let (l, f) = (s.layers[0].id, s.frames[0].id);
        let mut c = Canvas::new(4, 3).unwrap();
        c.set(1, 2, RED);
        put_cel(&mut data, l, f, &c);
        assert_eq!(cel(&data, &s, l, f), Some(c));
        put_cel(&mut data, l, f, &Canvas::new(4, 3).unwrap());
        assert!(!data.contains_key(&cel_key(l, f)), "a transparent cel has no key");
    }

    #[test]
    fn a_damaged_cel_reads_as_transparent() {
        let s = Sprite::new(2, 2, 1).unwrap();
        let mut data = Data::new();
        save(&mut data, &s);
        data.insert(cel_key(s.layers[0].id, s.frames[0].id), "!!not base64".into());
        assert!(cel(&data, &s, s.layers[0].id, s.frames[0].id).unwrap().is_clear());
        data.insert(KEY.into(), "{".into());
        assert_eq!(load(&data), None);
    }

    #[test]
    fn layers_are_drawn_bottom_to_top_and_hidden_ones_left_out() {
        let mut s = Sprite::new(1, 1, 1).unwrap();
        let top = s.take_id();
        s.layers.push(crate::sprite::Layer { id: top, name: "top".into(), visible: true, opacity: 255 });
        let mut data = Data::new();
        save(&mut data, &s);
        let f = s.frames[0].id;
        put_cel(&mut data, s.layers[0].id, f, &Canvas::filled(1, 1, RED).unwrap());
        put_cel(&mut data, top, f, &Canvas::filled(1, 1, [0, 0, 255, 255]).unwrap());
        assert_eq!(frame_image(&data, &s, 0).unwrap().get(0, 0), Some([0, 0, 255, 255]));
        let before = frame_hash(&data, &s, 0);
        s.layers[1].visible = false;
        assert_eq!(frame_image(&data, &s, 0).unwrap().get(0, 0), Some(RED));
        assert_ne!(frame_hash(&data, &s, 0), before);
    }

    #[test]
    fn saving_drops_the_cels_of_removed_layers() {
        let mut s = Sprite::new(1, 1, 1).unwrap();
        let mut data = Data::new();
        save(&mut data, &s);
        put_cel(&mut data, s.layers[0].id, s.frames[0].id, &Canvas::filled(1, 1, RED).unwrap());
        data.insert("other".into(), "kept".into());
        let id = s.take_id();
        s.layers = vec![crate::sprite::Layer { id, name: "new".into(), visible: true, opacity: 255 }];
        save(&mut data, &s);
        assert_eq!(data.keys().filter(|k| k.starts_with(CEL_PREFIX)).count(), 0);
        assert_eq!(data.get("other").map(String::as_str), Some("kept"));
        remove(&mut data);
        assert!(!is_pixel(&data));
        assert!(data.contains_key("other"));
    }
}
