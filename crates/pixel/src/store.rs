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
/// The box the layer shows outside Pixel mode (G16.7): `x,y,w,h` in art pixels of the board.
pub const TRIM_KEY: &str = "graybox.pixel.trim";

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
    data.retain(|k, _| k != KEY && k != SELECTION_KEY && k != TRIM_KEY && !k.starts_with(CEL_PREFIX));
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

/// The box `[x, y, w, h]` round every pixel the sprite shows in any frame (its visible layers), in
/// art pixels of the board (G16.7). None when nothing is drawn.
pub fn used_box(data: &Data, sprite: &Sprite) -> Option<[u32; 4]> {
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0u32, 0u32);
    for l in sprite.layers.iter().filter(|l| l.visible && l.opacity > 0) {
        for f in &sprite.frames {
            let Some(text) = data.get(&cel_key(l.id, f.id)) else { continue };
            let Some([x, y, w, h]) = cel_box(text, sprite.width, sprite.height) else { continue };
            (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x + w), y1.max(y + h));
        }
    }
    (x1 > x0 && y1 > y0).then(|| [x0, y0, x1 - x0, y1 - y0])
}

/// The box round the pixels of the cel stored as `text` on a `width` × `height` board (None when
/// it has none). Kept by the text's hash, so an edit decodes only the cel it changed.
fn cel_box(text: &str, width: u32, height: u32) -> Option<[u32; 4]> {
    use std::hash::{Hash, Hasher};
    /// How many cels' boxes are kept.
    const KEPT: usize = 4096;
    static BOXES: std::sync::Mutex<BTreeMap<u64, Option<[u32; 4]>>> = std::sync::Mutex::new(BTreeMap::new());
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (text, width, height).hash(&mut h);
    let key = h.finish();
    if let Ok(boxes) = BOXES.lock()
        && let Some(b) = boxes.get(&key)
    {
        return *b;
    }
    let found = decode_cel(text, width, height).and_then(|c| {
        let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0u32, 0u32);
        let w = c.width().max(1) as usize;
        for (at, p) in c.pixels().iter().enumerate() {
            if p[3] > 0 {
                let (x, y) = ((at % w) as u32, (at / w) as u32);
                (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x + 1), y1.max(y + 1));
            }
        }
        (x1 > x0 && y1 > y0).then(|| [x0, y0, x1 - x0, y1 - y0])
    });
    if let Ok(mut boxes) = BOXES.lock() {
        if boxes.len() >= KEPT {
            boxes.clear();
        }
        boxes.insert(key, found);
    }
    found
}

/// The box `[x, y, w, h]` (art pixels of the board) the layer shows outside Pixel mode: the one
/// stored round its used pixels, or the whole board (empty, or never trimmed).
pub fn shown_box(data: &Data, sprite: &Sprite) -> [u32; 4] {
    let stored = data.get(TRIM_KEY).and_then(|t| {
        let v: Vec<u32> = t.split(',').filter_map(|p| p.trim().parse().ok()).collect();
        match v.as_slice() {
            [x, y, w, h] => Some([*x, *y, *w, *h]),
            _ => None,
        }
    });
    match stored {
        Some([x, y, w, h]) if w > 0 && h > 0 && x.saturating_add(w) <= sprite.width && y.saturating_add(h) <= sprite.height => [x, y, w, h],
        _ => [0, 0, sprite.width, sprite.height],
    }
}

/// Stores the box the layer shows (None: the whole board).
pub fn put_trim(data: &mut Data, trim: Option<[u32; 4]>) {
    match trim {
        Some([x, y, w, h]) => {
            data.insert(TRIM_KEY.into(), format!("{x},{y},{w},{h}"));
        }
        None => {
            data.remove(TRIM_KEY);
        }
    }
}

/// Frame `index` as the layer shows it outside Pixel mode: cut to the sprite's shown box.
pub fn shown_image(data: &Data, sprite: &Sprite, index: usize) -> Option<Canvas> {
    let full = frame_image(data, sprite, index)?;
    let [x, y, w, h] = shown_box(data, sprite);
    if [x, y, w, h] == [0, 0, sprite.width, sprite.height] {
        return Some(full);
    }
    full.resized(w, h, -i64::from(x), -i64::from(y))
}

/// A content hash of what frame `index` shows (its description and the cels of its visible
/// layers), so a picture made from it can be kept while nothing it is made of changed.
pub fn frame_hash(data: &Data, sprite: &Sprite, index: usize) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (sprite.width, sprite.height, index, data.get(TRIM_KEY)).hash(&mut h);
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
