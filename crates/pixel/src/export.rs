//! Graybox pixel art export (G16.5). An exporter takes a sprite's frames already drawn (one picture
//! per frame, the chosen layers composited, see [`Art`]) and the options, and returns the files to
//! write. It never touches the disk, so the engine, the CLI and Claude share it, and other engines
//! (Godot's `SpriteFrames`, GameMaker, …) are new [`PixelExporter`]s later.
//!
//! The sheet's JSON follows the common sprite-sheet layout (a `frames` array with `frame`,
//! `spriteSourceSize`, `sourceSize`, `duration`, and a `meta` with `frameTags` and `slices`), so
//! tools that read those sheets read ours. The schema is in `docs/game/pixel-art.md`.

use serde_json::{Value, json};

use crate::canvas::Canvas;
use crate::sprite::{Direction, Sprite};

/// The largest export scale.
pub const MAX_SCALE: u32 = 16;
/// The most transparent pixels between frames (and around the sheet).
pub const MAX_PADDING: u32 = 64;
/// The most times a frame's edge pixels are repeated outwards.
pub const MAX_EXTRUDE: u32 = 16;
/// The longest side of an exported picture or sheet.
pub const MAX_SIDE: u32 = 8192;
/// The most pixels in one exported picture (128 MB of colour).
const MAX_PIXELS: u64 = 1 << 25;
/// The most pixels of all the frames of one export together (256 MB of colour).
const MAX_TOTAL: u64 = 1 << 26;

/// The export formats, by the name commands take.
pub const FORMATS: [&str; 5] = ["png", "frames", "sheet", "gif", "unity"];

/// How a sheet places its frames.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Layout {
    /// Near square, and frames that look the same share one cell.
    #[default]
    Packed,
    /// Rows of `columns` (near square when not given), one cell per frame.
    Grid,
    /// One row.
    Row,
}

impl Layout {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "packed" => Some(Self::Packed),
            "grid" => Some(Self::Grid),
            "row" => Some(Self::Row),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Packed => "packed",
            Self::Grid => "grid",
            Self::Row => "row",
        }
    }
}

/// What to export and how.
#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    /// The files' base name.
    pub name: String,
    /// Each art pixel becomes `scale` × `scale` pixels.
    pub scale: u32,
    /// Only this tag's frames (and, for a GIF, in its direction and repeats).
    pub tag: Option<String>,
    /// Cut the empty border every exported frame shares.
    pub trim: bool,
    /// Transparent pixels between frames and around a sheet.
    pub padding: u32,
    /// Repeat each frame's edge pixels outwards this many times (stops bleeding when filtered).
    pub extrude: u32,
    /// Round a sheet's sides up to powers of two.
    pub pow2: bool,
    pub layout: Layout,
    /// A grid's columns.
    pub columns: Option<u32>,
    /// The 9-slice cuts in art pixels: top, right, bottom, left.
    pub slice: Option<[u32; 4]>,
    /// The pivot, 0 to 1 from the sprite's top left.
    pub pivot: [f64; 2],
    /// Art pixels per Unity unit.
    pub ppu: f64,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            name: "sprite".into(),
            scale: 1,
            tag: None,
            trim: false,
            padding: 0,
            extrude: 0,
            pow2: false,
            layout: Layout::Packed,
            columns: None,
            slice: None,
            pivot: [0.5, 0.5],
            ppu: 16.0,
        }
    }
}

/// A sprite and its frames drawn (`frames[i]` is the sprite's frame `i`).
pub struct Art<'a> {
    pub sprite: &'a Sprite,
    pub frames: Vec<Canvas>,
}

/// A file to write: a name (no folders) and its bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutFile {
    pub name: String,
    pub bytes: Vec<u8>,
}

/// Turns a sprite into files.
pub trait PixelExporter {
    fn export(&self, art: &Art<'_>, opts: &Options) -> Result<Vec<OutFile>, String>;
}

/// The exporter for a format in [`FORMATS`].
pub fn exporter(format: &str) -> Option<&'static dyn PixelExporter> {
    match format {
        "png" => Some(&Png),
        "frames" => Some(&Frames),
        "sheet" => Some(&Sheet),
        "gif" => Some(&Gif),
        "unity" => Some(&Unity),
        _ => None,
    }
}

/// The first exported frame as one PNG.
pub struct Png;
/// Each exported frame as a PNG (`name_0.png`, `name_1.png`, …).
pub struct Frames;
/// One PNG holding every exported frame, and a JSON saying where each is.
pub struct Sheet;
/// An animated GIF (exact colours up to 256; partly transparent pixels become solid).
pub struct Gif;
/// A sheet whose JSON also carries Unity's import settings (Point filter, no compression, pixels
/// per unit, 9-slice borders), which the Graybox Live Link package applies.
pub struct Unity;

impl PixelExporter for Png {
    fn export(&self, art: &Art<'_>, opts: &Options) -> Result<Vec<OutFile>, String> {
        let idx = selected(art, opts)?;
        let first = idx.get(..1).unwrap_or_default();
        let (pics, _) = cut(art, first, opts)?;
        let pic = pics.first().ok_or("there's no frame to export")?;
        Ok(vec![OutFile { name: format!("{}.png", opts.name), bytes: pic.png()? }])
    }
}

impl PixelExporter for Frames {
    fn export(&self, art: &Art<'_>, opts: &Options) -> Result<Vec<OutFile>, String> {
        let idx = selected(art, opts)?;
        let (pics, _) = cut(art, &idx, opts)?;
        let digits = idx.len().saturating_sub(1).to_string().len();
        pics.iter().enumerate().map(|(k, p)| Ok(OutFile { name: format!("{}_{k:0digits$}.png", opts.name), bytes: p.png()? })).collect()
    }
}

impl PixelExporter for Sheet {
    fn export(&self, art: &Art<'_>, opts: &Options) -> Result<Vec<OutFile>, String> {
        sheet_files(art, opts, false)
    }
}

impl PixelExporter for Unity {
    fn export(&self, art: &Art<'_>, opts: &Options) -> Result<Vec<OutFile>, String> {
        sheet_files(art, opts, true)
    }
}

impl PixelExporter for Gif {
    fn export(&self, art: &Art<'_>, opts: &Options) -> Result<Vec<OutFile>, String> {
        use image::codecs::gif::{GifEncoder, Repeat};
        selected(art, opts)?;
        let tag = opts.tag.as_deref().and_then(|t| art.sprite.tag(t));
        let seq: Vec<usize> = crate::anim::sequence(art.sprite, tag).into_iter().filter(|i| *i < art.frames.len()).collect();
        let (pics, _) = cut(art, &seq, opts)?;
        let mut bytes = Vec::new();
        {
            let mut enc = GifEncoder::new_with_speed(&mut bytes, 10);
            let fail = |e: image::ImageError| format!("the GIF couldn't be made: {e}");
            match tag.map_or(0, |t| t.repeat) {
                0 => enc.set_repeat(Repeat::Infinite).map_err(fail)?,
                1 => {}
                n => enc.set_repeat(Repeat::Finite(u16::try_from(n - 1).unwrap_or(u16::MAX))).map_err(fail)?,
            }
            for (pic, i) in pics.into_iter().zip(&seq) {
                let ms = art.sprite.frames.get(*i).map_or(100, |f| f.duration);
                let img = image::RgbaImage::from_raw(pic.w, pic.h, pic.px).ok_or("a frame has the wrong size")?;
                enc.encode_frame(image::Frame::from_parts(img, 0, 0, image::Delay::from_numer_denom_ms(ms, 1))).map_err(fail)?;
            }
        }
        Ok(vec![OutFile { name: format!("{}.gif", opts.name), bytes }])
    }
}

/// The frames `opts` exports, in order.
fn selected(art: &Art<'_>, opts: &Options) -> Result<Vec<usize>, String> {
    let n = art.frames.len().min(art.sprite.frames.len());
    let idx: Vec<usize> = match &opts.tag {
        None => (0..n).collect(),
        Some(name) => {
            let t = art.sprite.tag(name).ok_or_else(|| format!("there's no tag named {name}"))?;
            (t.from..=t.to).filter(|i| *i < n).collect()
        }
    };
    if idx.is_empty() {
        return Err("there's no frame to export".into());
    }
    Ok(idx)
}

/// An RGBA picture of any size up to [`MAX_SIDE`].
struct Pic {
    w: u32,
    h: u32,
    px: Vec<u8>,
}

impl Pic {
    fn new(w: u64, h: u64) -> Result<Self, String> {
        if w == 0 || h == 0 || w > u64::from(MAX_SIDE) || h > u64::from(MAX_SIDE) || w * h > MAX_PIXELS {
            return Err(format!(
                "that would be {w}×{h} pixels, more than Graybox exports ({MAX_SIDE} on a side): lower the scale, export fewer frames or pick another layout"
            ));
        }
        Ok(Self { w: w as u32, h: h as u32, px: vec![0; (w * h * 4) as usize] })
    }

    fn at(&self, x: u32, y: u32) -> usize {
        (y as usize * self.w as usize + x as usize) * 4
    }

    fn get(&self, x: u32, y: u32) -> [u8; 4] {
        let i = self.at(x, y);
        self.px.get(i..i + 4).and_then(|s| s.try_into().ok()).unwrap_or([0; 4])
    }

    fn set(&mut self, x: u32, y: u32, c: [u8; 4]) {
        let i = self.at(x, y);
        if let Some(s) = self.px.get_mut(i..i + 4) {
            s.copy_from_slice(&c);
        }
    }

    /// Draws `src` with its top left at `x`, `y`, its edge pixels repeated `extrude` times around.
    fn blit(&mut self, src: &Pic, x: u32, y: u32, extrude: u32) {
        let e = i64::from(extrude);
        for dy in -e..i64::from(src.h) + e {
            for dx in -e..i64::from(src.w) + e {
                let (sx, sy) = (dx.clamp(0, i64::from(src.w) - 1) as u32, dy.clamp(0, i64::from(src.h) - 1) as u32);
                let (tx, ty) = (i64::from(x) + dx, i64::from(y) + dy);
                if tx >= 0 && ty >= 0 && tx < i64::from(self.w) && ty < i64::from(self.h) {
                    self.set(tx as u32, ty as u32, src.get(sx, sy));
                }
            }
        }
    }

    fn png(&self) -> Result<Vec<u8>, String> {
        crate::canvas::encode_png(self.w, self.h, &self.px).ok_or_else(|| "the PNG couldn't be made".into())
    }
}

/// The part of the sprite that is exported (x, y, width, height in art pixels).
type Rect = (u32, u32, u32, u32);

/// Frames `idx` cut to the shared trim box (when trimming) and scaled; and that box.
fn cut(art: &Art<'_>, idx: &[usize], opts: &Options) -> Result<(Vec<Pic>, Rect), String> {
    let frames: Vec<&Canvas> = idx.iter().filter_map(|i| art.frames.get(*i)).collect();
    let whole = (0, 0, art.sprite.width, art.sprite.height);
    let rect = if opts.trim { bounds(&frames).unwrap_or(whole) } else { whole };
    let s = opts.scale.clamp(1, MAX_SCALE);
    let each = u64::from(rect.2) * u64::from(rect.3) * u64::from(s) * u64::from(s);
    if each.saturating_mul(frames.len() as u64) > MAX_TOTAL {
        return Err("that is more pixels than Graybox exports at once: lower the scale or export one tag at a time".into());
    }
    let pics = frames
        .iter()
        .map(|c| {
            let mut p = Pic::new(u64::from(rect.2) * u64::from(s), u64::from(rect.3) * u64::from(s))?;
            for y in 0..p.h {
                for x in 0..p.w {
                    let at = c.get(i64::from(rect.0 + x / s), i64::from(rect.1 + y / s));
                    p.set(x, y, at.unwrap_or([0; 4]));
                }
            }
            Ok(p)
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok((pics, rect))
}

/// The smallest box holding every painted pixel of `frames` (None when all are empty).
fn bounds(frames: &[&Canvas]) -> Option<Rect> {
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
    for c in frames {
        for y in 0..c.height() {
            for x in 0..c.width() {
                if c.get(i64::from(x), i64::from(y)).is_some_and(|p| p[3] > 0) {
                    (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
                }
            }
        }
    }
    (x0 <= x1 && y0 <= y1).then(|| (x0, y0, x1 - x0 + 1, y1 - y0 + 1))
}

fn direction(d: Direction) -> &'static str {
    match d {
        Direction::Forward => "forward",
        Direction::Reverse => "reverse",
        Direction::PingPong => "pingpong",
    }
}

/// The sheet PNG and its JSON.
fn sheet_files(art: &Art<'_>, opts: &Options, unity: bool) -> Result<Vec<OutFile>, String> {
    let idx = selected(art, opts)?;
    let (pics, rect) = cut(art, &idx, opts)?;
    let s = opts.scale.clamp(1, MAX_SCALE);
    let (cw, ch) = (rect.2 * s, rect.3 * s);
    // Frames that look the same share a cell when packed.
    let mut cell_of = Vec::with_capacity(pics.len());
    let mut uniques: Vec<&Pic> = Vec::new();
    let mut seen: std::collections::HashMap<&[u8], usize> = std::collections::HashMap::new();
    for p in &pics {
        let same = if opts.layout == Layout::Packed { seen.get(p.px.as_slice()).copied() } else { None };
        cell_of.push(same.unwrap_or_else(|| {
            uniques.push(p);
            seen.insert(&p.px, uniques.len() - 1);
            uniques.len() - 1
        }));
    }
    let n = uniques.len().max(1) as u32;
    let cols = match opts.layout {
        Layout::Row => n,
        Layout::Grid => opts.columns.unwrap_or_else(|| near_square(n, 1.0)),
        Layout::Packed => near_square(n, f64::from(ch) / f64::from(cw.max(1))),
    }
    .clamp(1, n);
    let rows = n.div_ceil(cols);
    let (pad, e) = (opts.padding.min(MAX_PADDING), opts.extrude.min(MAX_EXTRUDE));
    let (step_x, step_y) = (u64::from(cw + 2 * e + pad), u64::from(ch + 2 * e + pad));
    let (mut w, mut h) = (u64::from(pad) + u64::from(cols) * step_x, u64::from(pad) + u64::from(rows) * step_y);
    if opts.pow2 {
        (w, h) = (w.next_power_of_two(), h.next_power_of_two());
    }
    let mut sheet = Pic::new(w, h)?;
    let cells: Vec<(u32, u32)> = uniques
        .iter()
        .enumerate()
        .map(|(c, p)| {
            let (col, row) = (c as u32 % cols, c as u32 / cols);
            let (x, y) = (pad + col * step_x as u32 + e, pad + row * step_y as u32 + e);
            sheet.blit(p, x, y, e);
            (x, y)
        })
        .collect();

    let (sw, sh) = (art.sprite.width, art.sprite.height);
    let trimmed = rect != (0, 0, sw, sh);
    let pivot = [
        round4((opts.pivot[0] * f64::from(sw) - f64::from(rect.0)) / f64::from(rect.2.max(1))),
        round4((opts.pivot[1] * f64::from(sh) - f64::from(rect.1)) / f64::from(rect.3.max(1))),
    ];
    // Unity's border (left, bottom, right, top) in the cut frame's pixels.
    let border = opts.slice.map(|[t, r, b, l]| {
        let side = |inset: u32, gone: u32, len: u32| inset.saturating_sub(gone).min(len) * s;
        let left = side(l, rect.0, rect.2);
        let top = side(t, rect.1, rect.3);
        let right = side(r, sw.saturating_sub(rect.0 + rect.2), rect.2).min(cw - left);
        let bottom = side(b, sh.saturating_sub(rect.1 + rect.3), rect.3).min(ch - top);
        json!({"left": left, "bottom": bottom, "right": right, "top": top})
    });
    let frames: Vec<Value> = idx
        .iter()
        .zip(&cell_of)
        .enumerate()
        .map(|(k, (i, c))| {
            let (x, y) = cells.get(*c).copied().unwrap_or_default();
            let mut f = json!({
                "filename": format!("{} {k}", opts.name),
                "frame": {"x": x, "y": y, "w": cw, "h": ch},
                "rotated": false,
                "trimmed": trimmed,
                "spriteSourceSize": {"x": rect.0 * s, "y": rect.1 * s, "w": cw, "h": ch},
                "sourceSize": {"w": sw * s, "h": sh * s},
                "duration": art.sprite.frames.get(*i).map_or(100, |f| f.duration),
                "pivot": {"x": pivot[0], "y": pivot[1]},
            });
            if let (Some(b), Some(o)) = (&border, f.as_object_mut()) {
                o.insert("border".into(), b.clone());
            }
            f
        })
        .collect();
    let (first, last) = (idx.first().copied().unwrap_or(0), idx.last().copied().unwrap_or(0));
    let tags: Vec<Value> = art
        .sprite
        .tags
        .iter()
        .filter(|t| t.to >= first && t.from <= last)
        .map(|t| {
            json!({
                "name": t.name,
                "from": t.from.max(first) - first,
                "to": t.to.min(last) - first,
                "direction": direction(t.direction),
                "repeat": t.repeat,
            })
        })
        .collect();
    let slices: Vec<Value> = opts
        .slice
        .map(|[t, r, b, l]| {
            json!({
                "name": opts.name,
                "keys": [{
                    "frame": 0,
                    "bounds": {"x": 0, "y": 0, "w": sw * s, "h": sh * s},
                    "center": {"x": l * s, "y": t * s, "w": sw.saturating_sub(l + r) * s, "h": sh.saturating_sub(t + b) * s},
                }],
            })
        })
        .into_iter()
        .collect();
    let image = format!("{}.png", opts.name);
    let mut meta = json!({
        "app": "Graybox",
        "version": "1",
        "image": image,
        "format": "RGBA8888",
        "size": {"w": sheet.w, "h": sheet.h},
        "scale": s,
        "frameTags": tags,
        "slices": slices,
    });
    if unity && let Some(o) = meta.as_object_mut() {
        let ppu = if opts.ppu.is_finite() { opts.ppu.clamp(1.0, 1000.0) } else { 16.0 };
        o.insert(
            "unity".into(),
            json!({
                "spriteMode": "Multiple",
                "filterMode": "Point",
                "compression": "None",
                "pixelsPerUnit": ppu * f64::from(s),
                "play": art.sprite.play,
            }),
        );
    }
    let doc = json!({"frames": frames, "meta": meta});
    let text = serde_json::to_vec_pretty(&doc).map_err(|e| format!("the JSON couldn't be made: {e}"))?;
    Ok(vec![OutFile { name: image, bytes: sheet.png()? }, OutFile { name: format!("{}.json", opts.name), bytes: text }])
}

/// Columns for `n` cells whose height is `aspect` × their width, so the sheet comes out near square.
fn near_square(n: u32, aspect: f64) -> u32 {
    let cols = (f64::from(n) * aspect).sqrt().ceil();
    if cols.is_finite() { (cols as u32).clamp(1, n.max(1)) } else { 1 }
}

fn round4(v: f64) -> f64 {
    if v.is_finite() { (v * 10_000.0).round() / 10_000.0 } else { 0.5 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sprite::Tag;

    const RED: [u8; 4] = [200, 30, 40, 255];
    const BLUE: [u8; 4] = [30, 60, 220, 255];

    /// A `w` × `h` sprite with one frame per dot list (each dot `(x, y, colour)`).
    fn art(w: u32, h: u32, dots: &[&[(i64, i64, [u8; 4])]]) -> (Sprite, Vec<Canvas>) {
        let mut sprite = Sprite::new(w, h, 1).unwrap();
        let frames = dots
            .iter()
            .enumerate()
            .map(|(i, ds)| {
                if i > 0 {
                    sprite.insert_frame(i, 100).unwrap();
                }
                let mut c = Canvas::new(w, h).unwrap();
                for (x, y, col) in *ds {
                    c.set(*x, *y, *col);
                }
                c
            })
            .collect();
        (sprite, frames)
    }

    fn run(format: &str, sprite: &Sprite, frames: &[Canvas], opts: &Options) -> Result<Vec<OutFile>, String> {
        exporter(format).unwrap().export(&Art { sprite, frames: frames.to_vec() }, opts)
    }

    fn json_of(files: &[OutFile]) -> Value {
        serde_json::from_slice(&files.iter().find(|f| f.name.ends_with(".json")).unwrap().bytes).unwrap()
    }

    fn png_of(files: &[OutFile]) -> (u32, u32, Vec<u8>) {
        crate::canvas::decode_png(&files.iter().find(|f| f.name.ends_with(".png")).unwrap().bytes, MAX_SIDE).unwrap()
    }

    fn px((w, _, bytes): &(u32, u32, Vec<u8>), x: u32, y: u32) -> [u8; 4] {
        let i = ((y * w + x) * 4) as usize;
        bytes[i..i + 4].try_into().unwrap()
    }

    #[test]
    fn a_packed_sheet_puts_frames_that_look_the_same_in_one_cell() {
        let (sprite, frames) = art(4, 2, &[&[(0, 0, RED)], &[(0, 0, RED)], &[(3, 1, BLUE)]]);
        let files = run("sheet", &sprite, &frames, &Options::default()).unwrap();
        assert_eq!(files.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["sprite.png", "sprite.json"]);
        let j = json_of(&files);
        let at = |k: usize| j["frames"][k]["frame"].clone();
        assert_eq!(at(0), json!({"x": 0, "y": 0, "w": 4, "h": 2}));
        assert_eq!(at(1), at(0), "the same picture is stored once");
        assert_eq!(at(2), json!({"x": 0, "y": 2, "w": 4, "h": 2}), "two wide cells stack, near square");
        assert_eq!(j["meta"]["size"], json!({"w": 4, "h": 4}));
        assert_eq!(j["frames"][2]["duration"], 100);
        let png = png_of(&files);
        assert_eq!((px(&png, 0, 0), px(&png, 3, 3)), (RED, BLUE));
    }

    #[test]
    fn a_grid_keeps_every_frame_with_padding_and_extruded_edges() {
        let (sprite, frames) = art(2, 2, &[&[(0, 0, RED)], &[(0, 0, RED)]]);
        let opts = Options { layout: Layout::Grid, columns: Some(2), padding: 1, extrude: 1, ..Options::default() };
        let files = run("sheet", &sprite, &frames, &opts).unwrap();
        let j = json_of(&files);
        // A cell is 2 + 2 × 1 extruded pixels, plus 1 of padding: 1 + 2 × 5 wide, 1 + 5 high.
        assert_eq!(j["meta"]["size"], json!({"w": 11, "h": 6}));
        assert_eq!(j["frames"][0]["frame"], json!({"x": 2, "y": 2, "w": 2, "h": 2}));
        assert_eq!(j["frames"][1]["frame"], json!({"x": 7, "y": 2, "w": 2, "h": 2}));
        let png = png_of(&files);
        assert_eq!((px(&png, 1, 1), px(&png, 1, 2), px(&png, 0, 0)), (RED, RED, [0; 4]), "the corner pixel bleeds out; padding stays clear");
        let pow2 = run("sheet", &sprite, &frames, &Options { pow2: true, ..opts }).unwrap();
        assert_eq!(json_of(&pow2)["meta"]["size"], json!({"w": 16, "h": 8}));
    }

    #[test]
    fn trimming_cuts_the_border_every_frame_shares_and_keeps_where_the_art_sat() {
        let (sprite, frames) = art(8, 8, &[&[(2, 3, RED)], &[(5, 4, BLUE)]]);
        let opts = Options { trim: true, scale: 2, slice: Some([4, 2, 2, 3]), layout: Layout::Row, ..Options::default() };
        let j = json_of(&run("unity", &sprite, &frames, &opts).unwrap());
        let f = &j["frames"][0];
        assert_eq!(f["frame"], json!({"x": 0, "y": 0, "w": 8, "h": 4}));
        assert_eq!(f["trimmed"], true);
        assert_eq!(f["spriteSourceSize"], json!({"x": 4, "y": 6, "w": 8, "h": 4}));
        assert_eq!(f["sourceSize"], json!({"w": 16, "h": 16}));
        assert_eq!(f["pivot"], json!({"x": 0.5, "y": 0.5}), "the sprite's middle is the box's middle here");
        // The cuts were 3 in from the left and 4 from the top; 2 and 3 of those were trimmed off.
        assert_eq!(f["border"], json!({"left": 2, "bottom": 0, "right": 0, "top": 2}));
        assert_eq!(j["meta"]["slices"][0]["keys"][0]["center"], json!({"x": 6, "y": 8, "w": 6, "h": 4}));
        let unity = &j["meta"]["unity"];
        assert_eq!((unity["filterMode"].clone(), unity["pixelsPerUnit"].clone()), (json!("Point"), json!(32.0)));
    }

    #[test]
    fn one_tag_exports_its_own_frames_and_tags_count_from_the_first_exported_frame() {
        let (mut sprite, frames) = art(2, 2, &[&[], &[(0, 0, RED)], &[(1, 1, BLUE)], &[]]);
        sprite.tags.push(Tag { name: "idle".into(), from: 0, to: 1, direction: Direction::Forward, repeat: 0 });
        sprite.tags.push(Tag { name: "hit".into(), from: 1, to: 2, direction: Direction::PingPong, repeat: 3 });
        let opts = Options { tag: Some("hit".into()), layout: Layout::Row, ..Options::default() };
        let j = json_of(&run("sheet", &sprite, &frames, &opts).unwrap());
        assert_eq!(j["frames"].as_array().unwrap().len(), 2);
        assert_eq!(
            j["meta"]["frameTags"],
            json!([
                {"name": "idle", "from": 0, "to": 0, "direction": "forward", "repeat": 0},
                {"name": "hit", "from": 0, "to": 1, "direction": "pingpong", "repeat": 3},
            ])
        );
        assert!(run("sheet", &sprite, &frames, &Options { tag: Some("nope".into()), ..Options::default() }).unwrap_err().contains("nope"));
    }

    #[test]
    fn png_and_frames_write_sharp_scaled_pictures() {
        let (sprite, frames) = art(2, 1, &[&[(0, 0, RED)], &[(1, 0, BLUE)]]);
        let opts = Options { scale: 3, name: "coin".into(), ..Options::default() };
        let one = run("png", &sprite, &frames, &opts).unwrap();
        assert_eq!(one.len(), 1);
        let png = png_of(&one);
        assert_eq!((png.0, png.1), (6, 3));
        assert_eq!((px(&png, 2, 2), px(&png, 3, 0)), (RED, [0; 4]), "each art pixel is a 3 × 3 block, no blending");
        let each = run("frames", &sprite, &frames, &opts).unwrap();
        assert_eq!(each.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["coin_0.png", "coin_1.png"]);
        let (sprite, frames) = art(1024, 1024, &[&[]]);
        let big = run("png", &sprite, &frames, &Options { scale: 16, ..Options::default() }).unwrap_err();
        assert!(big.contains("lower the scale"), "{big}");
    }

    #[test]
    fn a_gif_plays_the_tag_in_its_direction_with_exact_colours() {
        use image::AnimationDecoder;
        let (mut sprite, frames) = art(2, 2, &[&[(0, 0, RED)], &[(1, 0, BLUE)], &[(1, 1, RED)]]);
        sprite.tags.push(Tag { name: "bob".into(), from: 0, to: 2, direction: Direction::PingPong, repeat: 0 });
        sprite.frames[1].duration = 250;
        let files = run("gif", &sprite, &frames, &Options { tag: Some("bob".into()), ..Options::default() }).unwrap();
        assert_eq!(files[0].name, "sprite.gif");
        let dec = image::codecs::gif::GifDecoder::new(std::io::Cursor::new(files[0].bytes.clone())).unwrap();
        let out: Vec<image::Frame> = dec.into_frames().collect::<Result<_, _>>().unwrap();
        assert_eq!(out.len(), 4, "0, 1, 2, 1: ping-pong without repeating the ends");
        assert_eq!(out[1].delay().numer_denom_ms(), (250, 1));
        assert_eq!(out[1].buffer().get_pixel(1, 0).0, BLUE);
        assert_eq!(out[1].buffer().get_pixel(0, 0).0[3], 0, "empty pixels stay see-through");
    }
}
