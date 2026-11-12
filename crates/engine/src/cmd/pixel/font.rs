//! Graybox pixel art (G16.6): pixel fonts. A pixel layer can be a glyph sheet: its first frame is a
//! grid of cells, one character each ([`figcraft_pixel::font`]). Graybox makes a TrueType font of
//! it and loads it under the font's name, so text layers use it like any font (it lays out, wraps
//! and hugs), and Pixel text draws it sharp. Drawing on the sheet updates the font
//! ([`refresh`], run by the app every frame and by the commands here).

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

use figcraft_doc::{Document, NodeId};
use figcraft_pixel::font::{self, Font};
use figcraft_pixel::{Canvas, Rgba, Sprite, store};
use serde_json::{Value, json};

use super::{show, sprite_of};
use crate::automation::params::{boolean, get, num, opt_id, req_id};
use crate::cmd::CommandSpec;
use crate::{Result, Session, bad, spec};

pub(super) fn specs() -> Vec<CommandSpec> {
    vec![
        spec!(
            "pixel.font.set",
            "Pixel font",
            "{id, name? (default the layer's), chars? (the cells' characters in order; default printable ASCII from the space), cellWidth?, cellHeight? (1–128, default 8), baseline? (rows above the baseline; default three quarters of the cell), spacing? (art pixels between glyphs, default 1), off? (true: no longer a font)}  the sprite's first frame is the glyph sheet, cells left to right then down; text layers use the font by its name → {name, glyphs}",
            true,
            set
        ),
        spec!(
            "pixel.font.sheet",
            "New pixel font",
            "{name? (default Pixel Font), cellWidth?, cellHeight? (default 6 × 8), chars?, columns? (default 16), scale? (default 4), x?, y?, parent?}  a pixel layer with a faint grid of cells (pixels under half opaque aren't glyphs), already a font → {id, name}",
            true,
            sheet
        ),
    ]
}

/// A whole number param from 1 to `max` (None when absent).
fn whole(cmd: &str, p: &Value, key: &str, max: u32) -> Result<Option<u32>> {
    match get(p, key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_f64()
            .filter(|f| f.is_finite() && *f >= 0.0 && *f <= f64::from(max))
            .map(|f| Some(f.round() as u32))
            .ok_or_else(|| bad(cmd, format!("`{key}` must be a whole number from 0 to {max}"))),
    }
}

fn text(cmd: &str, p: &Value, key: &str) -> Result<Option<String>> {
    match get(p, key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(bad(cmd, format!("`{key}` must be text"))),
    }
}

/// The font settings `p` gives over `base`.
fn settings(cmd: &str, p: &Value, mut f: Font) -> Result<Font> {
    if let Some(name) = text(cmd, p, "name")? {
        f.name = name;
    }
    if let Some(chars) = text(cmd, p, "chars")? {
        f.chars = chars;
    }
    let cell_given = get(p, "cellHeight").is_some_and(|v| !v.is_null());
    if let Some(w) = whole(cmd, p, "cellWidth", font::MAX_CELL)? {
        f.cell_width = w;
    }
    if let Some(h) = whole(cmd, p, "cellHeight", font::MAX_CELL)? {
        f.cell_height = h;
    }
    f.baseline = match whole(cmd, p, "baseline", font::MAX_CELL)? {
        Some(b) => b,
        None if cell_given => f.cell_height - f.cell_height / 4,
        None => f.baseline,
    };
    if let Some(s) = whole(cmd, p, "spacing", font::MAX_CELL)? {
        f.spacing = s;
    }
    f.sanitized().ok_or_else(|| bad(cmd, "a pixel font needs a name"))
}

/// The font file of glyph sheet `data` (None when it has no glyphs).
fn font_file(data: &store::Data, sprite: &Sprite, f: &Font) -> Option<Vec<u8>> {
    let sheet = store::frame_image(data, sprite, 0)?;
    font::ttf(f, &font::glyphs(&sheet, f))
}

/// The fonts loaded from glyph sheets: family (lower case) → what they were made from.
static LOADED: Mutex<Option<HashMap<String, u64>>> = Mutex::new(None);

/// Loads the pixel fonts of `doc` whose sheet or settings changed since they were last loaded
/// (cheap when none did). Returns how many it loaded.
pub fn refresh(doc: &Document) -> usize {
    use std::hash::{Hash, Hasher};
    let mut loaded = LOADED.lock().unwrap_or_else(PoisonError::into_inner);
    let map = loaded.get_or_insert_with(HashMap::new);
    let mut count = 0;
    // One sheet per family: the first in the file (a copy with the same name never takes turns with it).
    let mut seen = std::collections::HashSet::new();
    for n in doc.nodes() {
        let Some(f) = n.plugin_data.get(font::KEY).and_then(|t| Font::load(t)) else { continue };
        if !seen.insert(f.name.to_lowercase()) {
            continue;
        }
        let Some(sprite) = store::load(&n.plugin_data) else { continue };
        let mut h = std::collections::hash_map::DefaultHasher::new();
        (f.save(), store::frame_hash(&n.plugin_data, &sprite, 0)).hash(&mut h);
        let key = f.name.to_lowercase();
        if map.get(&key) == Some(&h.finish()) {
            continue;
        }
        map.insert(key, h.finish());
        if let Some(bytes) = font_file(&n.plugin_data, &sprite, &f) {
            figcraft_text::replace_font_bytes(bytes);
            count += 1;
        }
    }
    count
}

/// `wanted`, or with a number after it, so that no other glyph sheet of `doc` (but `own`) has it
/// and no installed font has it (a pixel font never takes the place of an installed family).
fn unique_name(doc: &Document, own: Option<NodeId>, wanted: &str) -> String {
    let ours: Vec<String> = LOADED.lock().unwrap_or_else(PoisonError::into_inner).as_ref().map(|m| m.keys().cloned().collect()).unwrap_or_default();
    let others: Vec<String> = doc
        .nodes()
        .filter(|n| Some(n.id) != own)
        .filter_map(|n| n.plugin_data.get(font::KEY).and_then(|t| Font::load(t)))
        .map(|f| f.name.to_lowercase())
        .collect();
    // Installed families by their exact names (has_family would also take "Name 2" for Name).
    let installed: std::collections::HashSet<String> =
        figcraft_text::family_names().into_iter().map(|f| f.to_lowercase()).filter(|f| !ours.contains(f)).collect();
    let taken = |name: &str| {
        let key = name.to_lowercase();
        others.contains(&key) || installed.contains(&key)
    };
    let mut name = wanted.to_string();
    let mut k = 2;
    while taken(&name) && k < 1000 {
        name = format!("{wanted} {k}");
        k += 1;
    }
    name
}

/// The glyph sheet font `name` is made from: the first in the file with that name (a copied sheet
/// keeps the name but isn't the font until renamed).
pub fn sheet_of(doc: &Document, name: &str) -> Option<NodeId> {
    doc.nodes()
        .find(|n| n.plugin_data.get(font::KEY).and_then(|t| Font::load(t)).is_some_and(|f| f.name.to_lowercase() == name.to_lowercase()))
        .map(|n| n.id)
}

/// The pixel fonts in `doc`: name and em (art pixels).
pub fn fonts(doc: &Document) -> Vec<(String, u32)> {
    let mut out: Vec<(String, u32)> =
        doc.nodes().filter_map(|n| n.plugin_data.get(font::KEY).and_then(|t| Font::load(t))).map(|f| (f.name.clone(), f.em())).collect();
    out.sort();
    out.dedup_by(|a, b| a.0.eq_ignore_ascii_case(&b.0));
    out
}

fn set(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "pixel.font.set";
    let id = req_id(cmd, p, "id")?;
    let sprite = sprite_of(cmd, s.doc()?, id)?;
    let doc = s.doc_mut()?;
    if boolean(p, "off") == Some(true) {
        doc.get_mut(id)?.plugin_data.remove(font::KEY);
        return Ok(json!({"name": Value::Null, "glyphs": 0}));
    }
    let n = doc.get(id)?;
    let base = n.plugin_data.get(font::KEY).and_then(|t| Font::load(t)).unwrap_or_else(|| Font::new(&n.name, 8, 8));
    let mut f = settings(cmd, p, base)?;
    f.name = unique_name(doc, Some(id), &f.name);
    let glyphs = store::frame_image(&n.plugin_data, &sprite, 0).map_or(0, |c| font::glyphs(&c, &f).iter().filter(|g| !g.rects.is_empty()).count());
    doc.get_mut(id)?.plugin_data.insert(font::KEY.into(), f.save());
    changed(doc, id);
    Ok(json!({"name": f.name, "glyphs": glyphs}))
}

/// After pixel layer `id` changed: if it is a glyph sheet, its font loads again and the text
/// that uses it measures its box again (hugging text follows the new glyphs).
pub(crate) fn changed(doc: &mut Document, id: NodeId) {
    let Some(name) = doc.node(id).and_then(|n| n.plugin_data.get(font::KEY)).and_then(|t| Font::load(t)).map(|f| f.name) else {
        return;
    };
    if refresh(doc) > 0 {
        crate::fonts::refit(doc, None, Some(&[name]));
    }
}

/// The faint cell grid: every other cell tinted, and the baseline row a little stronger (both
/// under half opaque, so not glyphs).
fn grid(f: &Font, cols: u32, rows: u32) -> Option<Canvas> {
    let mut c = Canvas::new(cols * f.cell_width, rows * f.cell_height)?;
    const CELL: Rgba = [120, 140, 200, 28];
    const BASE: Rgba = [200, 120, 120, 56];
    for r in 0..rows {
        for col in 0..cols {
            for y in 0..f.cell_height {
                for x in 0..f.cell_width {
                    let tint = if y + 1 == f.baseline {
                        Some(BASE)
                    } else if (r + col) % 2 == 0 {
                        Some(CELL)
                    } else {
                        None
                    };
                    if let Some(t) = tint {
                        c.set(i64::from(col * f.cell_width + x), i64::from(r * f.cell_height + y), t);
                    }
                }
            }
        }
    }
    Some(c)
}

fn sheet(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "pixel.font.sheet";
    let mut f = settings(cmd, p, Font::new("Pixel Font", 6, 8))?;
    let count = f.chars.chars().count().max(1) as u32;
    let cols = whole(cmd, p, "columns", 256)?.unwrap_or(16).clamp(1, count);
    let rows = count.div_ceil(cols);
    let (w, h) = (cols * f.cell_width, rows * f.cell_height);
    if w > Canvas::MAX_SIDE || h > Canvas::MAX_SIDE {
        return Err(bad(cmd, "that many cells don't fit in one sprite: use smaller cells, fewer characters or more columns"));
    }
    f.name = unique_name(s.doc()?, None, &f.name);
    let mut made = json!({"width": w, "height": h, "scale": whole(cmd, p, "scale", 64)?.unwrap_or(4).max(1), "name": f.name});
    if let Some(o) = made.as_object_mut() {
        for key in ["x", "y"] {
            if let Some(v) = num(p, key) {
                o.insert(key.into(), json!(v));
            }
        }
        if let Some(parent) = opt_id(cmd, p, "parent")? {
            o.insert("parent".into(), json!(parent.0));
        }
    }
    let create = crate::cmd::find_command("pixel.create").ok_or_else(|| bad(cmd, "can't make pixel layers"))?;
    let id = crate::automation::params::id(&(create.run)(s, &made)?, "id").ok_or_else(|| bad(cmd, "the layer wasn't made"))?;
    let sprite = sprite_of(cmd, s.doc()?, id)?;
    let layer = sprite.layers.first().map(|l| l.id).ok_or_else(|| bad(cmd, "the sprite has no layer"))?;
    let frame = sprite.frames.first().map(|fr| fr.id).ok_or_else(|| bad(cmd, "the sprite has no frame"))?;
    let doc = s.doc_mut()?;
    {
        let data = &mut doc.get_mut(id)?.plugin_data;
        if let Some(g) = grid(&f, cols, rows) {
            store::put_cel(data, layer, frame, &g);
        }
        data.insert(font::KEY.into(), f.save());
    }
    show(doc, id, &sprite);
    Ok(json!({"id": id.0, "name": f.name}))
}

/// The `fontFamily`, `fontStyle` and `fontSize` that set a text layer in pixel font `name` at art
/// scale `scale` (one glyph pixel one art pixel), if the file has it.
pub(crate) fn text_props(doc: &Document, name: &str, scale: u32) -> Option<serde_json::Map<String, Value>> {
    let (family, em) = fonts(doc).into_iter().find(|(n, _)| n.eq_ignore_ascii_case(name))?;
    refresh(doc);
    let mut m = serde_json::Map::new();
    m.insert("fontFamily".into(), json!(family));
    m.insert("fontStyle".into(), json!("Regular"));
    m.insert("fontSize".into(), json!(f64::from(em * scale)));
    Some(m)
}
