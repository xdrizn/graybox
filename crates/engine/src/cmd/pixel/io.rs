//! Graybox pixel art (G16.5): exporting a pixel layer's frames as files (PNG, a PNG per frame, a
//! sprite sheet and its JSON, a GIF, a Unity sheet) and importing a PNG or a sheet as a new pixel
//! layer. The pictures and the JSON are made by [`figcraft_pixel::export`]; this module picks the
//! frames, reads the options and writes the files.

use std::path::Path;

use base64::Engine as _;
use figcraft_doc::{Document, Node, NodeId};
use figcraft_pixel::export::{self, Art, Layout, OutFile, PixelExporter};
use figcraft_pixel::sprite::{DURATION_RANGE, MAX_SCALE};
use figcraft_pixel::{Canvas, Sprite, store};
use serde_json::{Value, json};

use super::{show, sprite_of};
use crate::automation::file_access;
use crate::automation::params::{get, req_id};
use crate::cmd::CommandSpec;
use crate::{Result, Session, bad, spec};

/// The largest PNG `pixel.import` reads.
const MAX_IMPORT_BYTES: u64 = 64 * 1024 * 1024;
/// The most art pixels of all the frames drawn for one export (256 MB of colour).
const MAX_DRAWN: u64 = 1 << 26;

pub(super) fn specs() -> Vec<CommandSpec> {
    vec![
        spec!(
            "pixel.export",
            "Export pixel art",
            "{id, format? (png|frames|sheet|gif|unity; default sheet), dir? (a folder to write into; without it the files come back as base64), name? (file name; default the layer's), scale? (1–16), tag? (only its frames), layers? [layer ids, shown or hidden; default the shown ones], trim?, padding? (0–64), extrude? (0–16), pow2?, layout? (packed|grid|row), columns?, pivot? [x, y] (0–1 from the top left), ppu? (art pixels per Unity unit, default 16)}  the layer's 9-slice goes into the JSON as slices and borders → {files [names] , dir} | {files [{name, data}]}",
            false,
            export_cmd
        ),
        spec!(
            "pixel.import",
            "Import pixel art",
            "{path | data (base64 PNG), name?, frameWidth?, frameHeight? (cut a sheet into frames this big, left to right then down), duration? (ms per frame, default 100), scale? (1–64, default 4), x?, y?, parent?}  a new pixel layer → {id, frames}",
            true,
            import_cmd
        ),
    ]
}

/// A whole-number param from `min` to `max` (None when absent).
fn whole(cmd: &str, p: &Value, key: &str, min: u32, max: u32) -> Result<Option<u32>> {
    match get(p, key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_f64()
            .filter(|f| f.is_finite() && *f >= f64::from(min) && *f <= f64::from(max))
            .map(|f| Some(f.round() as u32))
            .ok_or_else(|| bad(cmd, format!("`{key}` must be a whole number from {min} to {max}"))),
    }
}

fn flag(cmd: &str, p: &Value, key: &str) -> Result<bool> {
    match get(p, key) {
        None | Some(Value::Null) => Ok(false),
        Some(v) => v.as_bool().ok_or_else(|| bad(cmd, format!("`{key}` must be true or false"))),
    }
}

fn text(cmd: &str, p: &Value, key: &str) -> Result<Option<String>> {
    match get(p, key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(bad(cmd, format!("`{key}` must be text"))),
    }
}

/// `name` made safe as a file name (letters, digits, spaces, `-` and `_`; at most 64 characters).
fn file_name(name: &str) -> String {
    let safe: String = name.chars().take(64).map(|c| if c.is_alphanumeric() || matches!(c, ' ' | '-' | '_') { c } else { '_' }).collect();
    let safe = safe.trim();
    if safe.is_empty() { "sprite".into() } else { safe.into() }
}

/// The export options `p` gives for layer `id`.
fn options(cmd: &str, s: &Session, id: NodeId, sprite: &Sprite, p: &Value) -> Result<export::Options> {
    let doc = s.doc()?;
    let n = doc.get(id)?;
    let d = export::Options::default();
    let tag = text(cmd, p, "tag")?.filter(|t| !t.is_empty());
    if let Some(t) = &tag
        && sprite.tag(t).is_none()
    {
        return Err(bad(cmd, format!("there's no tag named {t}")));
    }
    let layout = match text(cmd, p, "layout")? {
        None => d.layout,
        Some(l) => Layout::parse(&l).ok_or_else(|| bad(cmd, "`layout` must be packed, grid or row"))?,
    };
    let pivot = match get(p, "pivot") {
        None | Some(Value::Null) => d.pivot,
        Some(v) => {
            let xy: Option<Vec<f64>> = v.as_array().map(|a| a.iter().filter_map(Value::as_f64).filter(|f| f.is_finite()).collect());
            match xy.as_deref() {
                Some([x, y]) => [x.clamp(-10.0, 10.0), y.clamp(-10.0, 10.0)],
                _ => return Err(bad(cmd, "`pivot` must be [x, y], 0 to 1 from the top left")),
            }
        }
    };
    let ppu = match get(p, "ppu") {
        None | Some(Value::Null) => d.ppu,
        Some(v) => {
            v.as_f64().filter(|f| f.is_finite() && *f >= 1.0 && *f <= 1000.0).ok_or_else(|| bad(cmd, "`ppu` must be a number from 1 to 1000"))?
        }
    };
    // G16.7: the cuts are measured on the art the layer shows; this exports the whole board.
    let [x, y, w, h] = store::shown_box(&n.plugin_data, sprite);
    let (right, bottom) = (sprite.width.saturating_sub(x + w), sprite.height.saturating_sub(y + h));
    let slice = slice_of(doc, id, sprite).map(|[t, r, b, l]| [t + y, r + right, b + bottom, l + x]);
    Ok(export::Options {
        name: file_name(&text(cmd, p, "name")?.unwrap_or_else(|| n.name.clone())),
        scale: whole(cmd, p, "scale", 1, export::MAX_SCALE)?.unwrap_or(d.scale),
        tag,
        trim: flag(cmd, p, "trim")?,
        padding: whole(cmd, p, "padding", 0, export::MAX_PADDING)?.unwrap_or(d.padding),
        extrude: whole(cmd, p, "extrude", 0, export::MAX_EXTRUDE)?.unwrap_or(d.extrude),
        pow2: flag(cmd, p, "pow2")?,
        layout,
        columns: whole(cmd, p, "columns", 1, figcraft_pixel::sprite::MAX_FRAMES as u32)?,
        slice,
        pivot,
        ppu,
    })
}

/// Layer `id`'s frames drawn with layers `p.layers` (default the shown ones).
fn drawn(cmd: &str, s: &Session, id: NodeId, sprite: &Sprite, p: &Value) -> Result<Vec<Canvas>> {
    let only: Option<Vec<u32>> = match get(p, "layers") {
        None | Some(Value::Null) => None,
        Some(Value::Array(a)) if !a.is_empty() => Some(
            a.iter()
                .take(figcraft_pixel::sprite::MAX_LAYERS)
                .map(|v| v.as_u64().and_then(|l| u32::try_from(l).ok()).filter(|l| sprite.layer_index(*l).is_some()))
                .collect::<Option<_>>()
                .ok_or_else(|| bad(cmd, "`layers` must be the sprite's layer ids"))?,
        ),
        Some(_) => return Err(bad(cmd, "`layers` must be the sprite's layer ids")),
    };
    // Every frame is drawn at once: refuse what would not fit in memory.
    if (sprite.frames.len() as u64) * u64::from(sprite.width) * u64::from(sprite.height) > MAX_DRAWN {
        return Err(bad(cmd, "the sprite has too many big frames to export at once"));
    }
    let data = &s.doc()?.get(id)?.plugin_data;
    (0..sprite.frames.len())
        .map(|i| store::frame_image_of(data, sprite, i, only.as_deref()).ok_or_else(|| bad(cmd, "a frame couldn't be drawn")))
        .collect()
}

/// The files layer `p.id` exports as, made but not written.
fn files(s: &Session, p: &Value) -> Result<Vec<OutFile>> {
    let cmd = "pixel.export";
    let id = req_id(cmd, p, "id")?;
    let sprite = sprite_of(cmd, s.doc()?, id)?;
    let format = text(cmd, p, "format")?.unwrap_or_else(|| "sheet".into());
    let exporter = export::exporter(&format).ok_or_else(|| bad(cmd, format!("`format` must be one of {}", export::FORMATS.join(", "))))?;
    let opts = options(cmd, s, id, &sprite, p)?;
    let frames = drawn(cmd, s, id, &sprite, p)?;
    exporter.export(&Art { sprite: &sprite, frames }, &opts).map_err(|e| bad(cmd, e))
}

/// Writes `files` into folder `dir` (made when missing); returns their names.
fn write(dir: &Path, files: &[OutFile]) -> Result<Vec<String>> {
    file_access::create_dir_all(dir)?;
    files
        .iter()
        .map(|f| {
            file_access::write(dir.join(&f.name), &f.bytes)?;
            Ok(f.name.clone())
        })
        .collect()
}

fn export_cmd(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "pixel.export";
    let dir = text(cmd, p, "dir")?.filter(|d| !d.trim().is_empty());
    let out = files(s, p)?;
    match dir {
        Some(dir) => Ok(json!({"files": write(Path::new(&dir), &out)?, "dir": dir})),
        None => {
            let b64 = base64::engine::general_purpose::STANDARD;
            Ok(json!({"files": out.iter().map(|f| json!({"name": f.name, "data": b64.encode(&f.bytes)})).collect::<Vec<_>>()}))
        }
    }
}

fn import_cmd(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "pixel.import";
    let (bytes, stem) = match (text(cmd, p, "path")?, text(cmd, p, "data")?) {
        (Some(path), _) => {
            let stem = Path::new(&path).file_stem().map(|f| f.to_string_lossy().into_owned());
            (file_access::read_capped(&path, MAX_IMPORT_BYTES)?, stem)
        }
        (None, Some(data)) => {
            (base64::engine::general_purpose::STANDARD.decode(data.trim()).map_err(|_| bad(cmd, "`data` must be a base64 PNG"))?, None)
        }
        (None, None) => return Err(bad(cmd, "`path` or `data` must give the PNG")),
    };
    let cell = match (whole(cmd, p, "frameWidth", 1, Canvas::MAX_SIDE)?, whole(cmd, p, "frameHeight", 1, Canvas::MAX_SIDE)?) {
        (None, None) => None,
        (w, h) => Some((w.or(h).unwrap_or(1), h.or(w).unwrap_or(1))),
    };
    let duration = whole(cmd, p, "duration", DURATION_RANGE.0, DURATION_RANGE.1)?.unwrap_or(100);
    let cels = figcraft_pixel::import::frames(&bytes, cell).map_err(|e| bad(cmd, e))?;
    let first = cels.first().ok_or_else(|| bad(cmd, "there's no frame in the picture"))?;
    let name = text(cmd, p, "name")?.or(stem).unwrap_or_else(|| "Imported sprite".into());
    let mut made = json!({
        "width": first.width(),
        "height": first.height(),
        "scale": whole(cmd, p, "scale", 1, MAX_SCALE)?.unwrap_or(4),
        "name": name,
    });
    for key in ["x", "y", "parent"] {
        if let (Some(v), Some(o)) = (get(p, key), made.as_object_mut()) {
            o.insert(key.into(), v.clone());
        }
    }
    let create = crate::cmd::find_command("pixel.create").ok_or_else(|| bad(cmd, "can't make pixel layers"))?;
    let id = crate::automation::params::id(&(create.run)(s, &made)?, "id").ok_or_else(|| bad(cmd, "the layer wasn't made"))?;
    let mut sprite = sprite_of(cmd, s.doc()?, id)?;
    for i in 1..cels.len() {
        sprite.insert_frame(i, duration).ok_or_else(|| bad(cmd, "too many frames"))?;
    }
    for f in &mut sprite.frames {
        f.duration = duration;
    }
    let layer = sprite.layers.first().map(|l| l.id).ok_or_else(|| bad(cmd, "the sprite has no layer"))?;
    let doc = s.doc_mut()?;
    {
        let data = &mut doc.get_mut(id)?.plugin_data;
        store::save(data, &sprite);
        for (cel, frame) in cels.iter().zip(&sprite.frames) {
            store::put_cel(data, layer, frame.id, cel);
        }
    }
    show(doc, id, &sprite);
    Ok(json!({"id": id.0, "frames": sprite.frames.len()}))
}

/// Layer `id`'s 9-slice in art pixels (top, right, bottom, left).
fn slice_of(doc: &Document, id: NodeId, sprite: &Sprite) -> Option<[u32; 4]> {
    let unit = f64::from(sprite.scale.max(1));
    let art = |v: f64| if v.is_finite() { (v / unit).round().clamp(0.0, f64::from(Canvas::MAX_SIDE)) as u32 } else { 0 };
    figcraft_doc::nine_slice::effective(doc, id).map(|(sl, _)| sl.insets.map(art))
}

/// The Unity screen export ([`crate::unity_export`]) for pixel layer `n`: its entry `v` says
/// `pixel` (Live Link imports its sprite with the Point filter and no compression), and one with
/// more than one frame also gets a Unity sheet and its JSON in `sprites` (`animation`), each art
/// pixel as big as on the screen (a sheet too big to make is left out; the still sprite stays).
/// Nothing for other layers.
pub(crate) fn unity_layer(doc: &Document, n: &Node, sprites: &Path, scale: f64, v: &mut Value) -> Result<()> {
    let Some(src) = figcraft_render::pixel::source(doc, n) else { return Ok(()) };
    let Some(sprite) = store::load(&src.plugin_data) else { return Ok(()) };
    if let Some(o) = v.as_object_mut() {
        o.insert("pixel".into(), json!(true));
    }
    if sprite.frames.len() < 2 {
        return Ok(());
    }
    // G16.7: the frames as the layer shows them, round the pixels it uses.
    let frames = (0..sprite.frames.len()).filter_map(|i| store::shown_image(&src.plugin_data, &sprite, i)).collect();
    let [_, _, tw, th] = store::shown_box(&src.plugin_data, &sprite);
    let sprite = Sprite { width: tw, height: th, ..sprite };
    let size = (f64::from(sprite.scale.max(1)) * scale).round();
    let opts = export::Options {
        name: format!("{}_anim", n.id.0),
        scale: if size.is_finite() { (size.max(1.0) as u32).min(export::MAX_SCALE) } else { 1 },
        slice: slice_of(doc, n.id, &sprite),
        ..export::Options::default()
    };
    let Ok(out) = export::Unity.export(&Art { sprite: &sprite, frames }, &opts) else { return Ok(()) };
    write(sprites, &out)?;
    if let Some(o) = v.as_object_mut() {
        o.insert("animation".into(), json!(format!("sprites/{}.json", opts.name)));
    }
    Ok(())
}
