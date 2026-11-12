//! Graybox pixel art (G16): a pixel layer's image fill draws its sprite instead of the stored
//! picture, sampled nearest-neighbour so every art pixel stays a sharp square at any zoom. A layer
//! shown at another size than its sprite never scales the art: it is regrown by whole pixels
//! ([`figcraft_pixel::resize`]), by its 9-slice cuts when it has some.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use figcraft_color::PaintKind;
use figcraft_doc::{Document, Node, NodeKind};
use figcraft_geom::Affine;
use figcraft_pixel::resize::{self, Insets, SliceFill};
use figcraft_pixel::{Canvas, Sprite, store};
use tiny_skia::{FillRule, FilterQuality, Pattern, Pixmap, SpreadMode, Transform};

use crate::draw::{Ctx, ts_transform};

/// Pictures kept between renders, at most this many bytes.
const MAX_CACHED_BYTES: usize = 64 << 20;

#[derive(Default)]
struct Cache {
    pictures: HashMap<u64, Arc<Pixmap>>,
    bytes: usize,
}

fn cache() -> &'static Mutex<Cache> {
    static C: OnceLock<Mutex<Cache>> = OnceLock::new();
    C.get_or_init(Mutex::default)
}

/// What a pixel layer shows: its sprite, the frame and the art size it is drawn at.
pub struct Shown {
    pub sprite: Sprite,
    pub frame: usize,
    /// Art pixels across and down at the layer's current size.
    pub size: (u32, u32),
    /// Its 9-slice cuts in art pixels and how the rest fills, when it has one.
    pub slice: Option<(Insets, SliceFill)>,
}

/// The layer whose sprite `n` shows: `n` itself when it is a pixel layer, or for an instance its
/// main component's (so every instance follows the main's pixels).
pub fn source<'a>(doc: &'a Document, n: &'a Node) -> Option<&'a Node> {
    if let NodeKind::Instance { component, .. } = n.kind
        && let Some(main) = doc.node(component)
        && store::is_pixel(&main.plugin_data)
    {
        return Some(main);
    }
    store::is_pixel(&n.plugin_data).then_some(n)
}

/// What pixel layer `n` shows now (None for any other layer).
pub fn shown(doc: &Document, n: &Node, time: f64) -> Option<Shown> {
    let src = source(doc, n)?;
    let sprite = store::load(&src.plugin_data)?;
    let scale = f64::from(sprite.scale.max(1));
    let art = |len: f64| if len.is_finite() { (len / scale).round().clamp(1.0, f64::from(Canvas::MAX_SIDE)) as u32 } else { 1 };
    let size = (art(n.width), art(n.height));
    let slice = figcraft_doc::nine_slice::effective(doc, n.id).map(|(s, _)| {
        let cut = |v: f64| if v.is_finite() { (v / scale).round().clamp(0.0, f64::from(Canvas::MAX_SIDE)) as u32 } else { 0 };
        (s.insets.map(cut), sprite.slice_fill)
    });
    let _ = time;
    Some(Shown { sprite, frame: 0, size, slice })
}

/// The picture of `shown` at its art size, premultiplied for drawing (cached while its cels, size
/// and resize settings are unchanged). `src` is the layer holding the sprite ([`source`]).
pub fn picture(src: &Node, shown: &Shown) -> Option<Arc<Pixmap>> {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    store::frame_hash(&src.plugin_data, &shown.sprite, shown.frame).hash(&mut h);
    shown.size.hash(&mut h);
    shown.slice.hash(&mut h);
    shown.sprite.resize.hash(&mut h);
    let key = h.finish();
    if let Ok(c) = cache().lock()
        && let Some(p) = c.pictures.get(&key)
    {
        return Some(p.clone());
    }
    let frame = store::frame_image(&src.plugin_data, &shown.sprite, shown.frame)?;
    let fitted = resize::fit(&frame, shown.size, shown.sprite.resize, shown.slice)?;
    let pm = Arc::new(to_pixmap(&fitted)?);
    if let Ok(mut c) = cache().lock() {
        let bytes = pm.data().len();
        if c.bytes + bytes > MAX_CACHED_BYTES {
            c.pictures.clear();
            c.bytes = 0;
        }
        c.bytes += bytes;
        c.pictures.insert(key, pm.clone());
    }
    Some(pm)
}

/// A canvas as a tiny-skia pixmap (premultiplied).
pub fn to_pixmap(c: &Canvas) -> Option<Pixmap> {
    let mut pm = Pixmap::new(c.width(), c.height())?;
    for (dst, src) in pm.pixels_mut().iter_mut().zip(c.pixels()) {
        let a = u16::from(src[3]);
        let pre = |v: u8| ((u16::from(v) * a + 127) / 255) as u8;
        if let Some(p) = tiny_skia::PremultipliedColorU8::from_rgba(pre(src[0]), pre(src[1]), pre(src[2]), src[3]) {
            *dst = p;
        }
    }
    Some(pm)
}

/// Draws pixel layer `n`'s fills inside `path`: its image fills as the sprite, sharp, the others
/// as usual. Any other layer is filled as usual.
pub(crate) fn fill(ctx: &Ctx, pix: &mut Pixmap, path: &tiny_skia::Path, n: &Node, xf: Affine, rule: FillRule) {
    let Some((src, shown)) = source(ctx.doc, n).zip(shown(ctx.doc, n, ctx.time)) else {
        crate::draw::fill_paints(pix, path, &n.fills, n, xf, rule, ctx);
        return;
    };
    let picture = picture(src, &shown);
    let (aw, ah) = shown.size;
    for p in &n.fills {
        if !p.visible {
            continue;
        }
        match (&p.kind, &picture) {
            (PaintKind::Image(_), Some(pm)) => {
                let (sx, sy) = (n.width / f64::from(aw.max(1)), n.height / f64::from(ah.max(1)));
                let pattern = Pattern::new(
                    (**pm).as_ref(),
                    SpreadMode::Pad,
                    FilterQuality::Nearest,
                    p.opacity.clamp(0.0, 1.0),
                    Transform::from_scale(sx as f32, sy as f32),
                );
                let paint = tiny_skia::Paint { shader: pattern, anti_alias: true, blend_mode: crate::paint::ts_blend(p.blend), ..Default::default() };
                pix.fill_path(path, &paint, rule, ts_transform(xf), None);
            }
            (PaintKind::Image(_), None) => {}
            _ => crate::draw::fill_paints(pix, path, std::slice::from_ref(p), n, xf, rule, ctx),
        }
    }
}

/// The plugin data key of Pixel text: the art scale (page units per art pixel) its glyphs snap to.
pub const TEXT_KEY: &str = "graybox.pixelText";

/// The largest Pixel text drawing (art pixels a side).
const MAX_TEXT_SIDE: f64 = 2048.0;

/// The art scale text layer `n` is drawn at as Pixel text, if it is.
pub fn text_scale(n: &Node) -> Option<f64> {
    let s: f64 = n.plugin_data.get(TEXT_KEY)?.trim().parse().ok()?;
    (s.is_finite() && s >= 1.0).then(|| s.min(64.0))
}

/// Graybox's own ways of drawing a whole layer (the `draw_layer` hook): Pixel text, then a
/// 9-sliced layer at another size than its base. False when neither applies.
pub(crate) fn layer(ctx: &Ctx, pix: &mut Pixmap, n: &Node, xf: Affine, depth: usize) -> bool {
    text(ctx, pix, n, xf, depth) || crate::nine_slice::draw(ctx, pix, n, xf, depth)
}

/// Draws text layer `n` (at `xf`, its own transform included) as Pixel text when it is one: drawn
/// at one device pixel per art pixel on its parent's art grid (whole art pixels from the parent's
/// top left), each pixel ink or not (no anti-aliasing), then scaled up sharp. Its effects are drawn
/// the same way; its opacity applies to the result. False when it isn't Pixel text.
pub(crate) fn text(ctx: &Ctx, pix: &mut Pixmap, n: &Node, xf: Affine, depth: usize) -> bool {
    if !matches!(n.kind, NodeKind::Text(_)) {
        return false;
    }
    let Some(s) = text_scale(n) else { return false };
    let mut plain = n.clone();
    plain.plugin_data.remove(TEXT_KEY);
    plain.transform = Affine::IDENTITY;
    plain.opacity = 1.0;
    // Where the parent's art grid falls in the layer's own space.
    let [.., e, f] = n.transform.as_coeffs();
    let (fx, fy) = (e.rem_euclid(s), f.rem_euclid(s));
    // One art pixel of margin round the text (glyphs and effects can overhang its box).
    let margin = 1.0 + (crate::draw::effect_margin(n) / s).ceil().min(64.0);
    let (w, h) = (((n.width + fx) / s).ceil() + 2.0 * margin, ((n.height + fy) / s).ceil() + 2.0 * margin);
    if !(w.is_finite() && h.is_finite()) || w > MAX_TEXT_SIDE || h > MAX_TEXT_SIDE {
        return false;
    }
    let Some(mut off) = Pixmap::new(w.max(1.0) as u32, h.max(1.0) as u32) else { return false };
    let to_art = Affine::translate((margin, margin)) * Affine::scale(1.0 / s) * Affine::translate((fx, fy));
    crate::draw::draw_layer(ctx, &mut off, &plain, to_art, depth);
    for p in off.pixels_mut() {
        *p = if p.alpha() >= 128 {
            let c = p.demultiply();
            tiny_skia::PremultipliedColorU8::from_rgba(c.red(), c.green(), c.blue(), 255).unwrap_or(*p)
        } else {
            tiny_skia::PremultipliedColorU8::TRANSPARENT
        };
    }
    let back = xf * Affine::translate((-fx, -fy)) * Affine::scale(s) * Affine::translate((-margin, -margin));
    let paint = tiny_skia::PixmapPaint { quality: FilterQuality::Nearest, opacity: n.opacity.clamp(0.0, 1.0), ..Default::default() };
    pix.draw_pixmap(0, 0, off.as_ref(), &paint, ts_transform(back), None);
    true
}

#[cfg(test)]
mod tests {
    use crate::paint_behaviour_tests::{page, rect};
    use crate::{View, render_nodes_into};
    use figcraft_color::{ImageScaleMode, Paint};
    use figcraft_pixel::{Canvas, Sprite, store};

    /// A 2×1 sprite (red, blue) at scale 10 drawn at 1.5× zoom: each art pixel a hard-edged block,
    /// no blend where they meet.
    #[test]
    fn a_pixel_layer_draws_hard_edged_pixels() {
        let (mut d, p) = page();
        let id = rect(&mut d, p, (0.0, 0.0, 20.0, 10.0), vec![Paint::image("unused", ImageScaleMode::Fill)]);
        let s = Sprite::new(2, 1, 10).unwrap();
        let cel = Canvas::from_pixels(2, 1, vec![[255, 0, 0, 255], [0, 0, 255, 255]]).unwrap();
        {
            let n = d.get_mut(id).unwrap();
            store::save(&mut n.plugin_data, &s);
            store::put_cel(&mut n.plugin_data, s.layers[0].id, s.frames[0].id, &cel);
        }
        let mut pix = tiny_skia::Pixmap::new(30, 15).unwrap();
        render_nodes_into(&d, &[id], &View { origin: (0.0, 0.0), scale: 1.5, width: 30, height: 15 }, &mut pix);
        let px = |x: u32| {
            let c = pix.pixel(x, 7).unwrap();
            (c.red(), c.green(), c.blue(), c.alpha())
        };
        assert_eq!(px(14), (255, 0, 0, 255), "red to the last device pixel of art pixel 0");
        assert_eq!(px(15), (0, 0, 255, 255), "blue from the first of art pixel 1");
        assert_eq!(px(0), (255, 0, 0, 255));
        assert_eq!(px(29), (0, 0, 255, 255));
    }
}

#[cfg(test)]
mod resize_tests {
    use crate::paint_behaviour_tests::{page, rect};
    use crate::{View, render_nodes_into};
    use figcraft_color::{ImageScaleMode, Paint};
    use figcraft_doc::nine_slice::{KEY, NineSlice};
    use figcraft_pixel::{Canvas, Sprite, store};

    const R: [u8; 4] = [255, 0, 0, 255];
    const G: [u8; 4] = [0, 255, 0, 255];
    const B: [u8; 4] = [0, 0, 255, 255];
    const W: [u8; 4] = [255, 255, 255, 255];

    /// A one-row sprite at scale 10 in a layer `width` page units wide, drawn at zoom 1: the colour
    /// of each art pixel (sampled in its middle).
    fn drawn(pixels: Vec<[u8; 4]>, width: f64, slice: Option<[f64; 4]>, edit: impl FnOnce(&mut Sprite)) -> Vec<[u8; 4]> {
        let (mut d, p) = page();
        let id = rect(&mut d, p, (0.0, 0.0, width, 10.0), vec![Paint::image("unused", ImageScaleMode::Fill)]);
        let mut s = Sprite::new(pixels.len() as u32, 1, 10).unwrap();
        edit(&mut s);
        let cel = Canvas::from_pixels(pixels.len() as u32, 1, pixels).unwrap();
        {
            let n = d.get_mut(id).unwrap();
            store::save(&mut n.plugin_data, &s);
            store::put_cel(&mut n.plugin_data, s.layers[0].id, s.frames[0].id, &cel);
            if let Some(insets) = slice {
                let base = (f64::from(s.width) * 10.0, 10.0);
                n.plugin_data.insert(KEY.into(), NineSlice { insets, base }.encode());
            }
        }
        let w = width as u32;
        let mut pix = tiny_skia::Pixmap::new(w, 10).unwrap();
        render_nodes_into(&d, &[id], &View { origin: (0.0, 0.0), scale: 1.0, width: w, height: 10 }, &mut pix);
        (0..w / 10)
            .map(|i| {
                let c = pix.pixel(i * 10 + 5, 5).unwrap();
                [c.red(), c.green(), c.blue(), c.alpha()]
            })
            .collect()
    }

    #[test]
    fn a_resized_pixel_layer_repeats_its_middle_instead_of_scaling() {
        assert_eq!(drawn(vec![R, G, B], 50.0, None, |_| {}), [R, G, G, G, B]);
    }

    #[test]
    fn a_nine_sliced_pixel_layer_tiles_or_stretches_by_whole_art_pixels() {
        let cuts = Some([0.0, 10.0, 0.0, 10.0]);
        assert_eq!(drawn(vec![R, G, B, W], 70.0, cuts, |_| {}), [R, G, B, G, B, G, W]);
        let stretched = drawn(vec![R, G, B, W], 70.0, cuts, |s| s.slice_fill = figcraft_pixel::resize::SliceFill::Stretch);
        assert_eq!(stretched, [R, G, G, G, B, B, W]);
    }
}

#[cfg(test)]
mod text_tests {
    use crate::{View, render_nodes_into};
    use figcraft_doc::{Document, NodeKind, TextData};

    /// "Hi" at 32 px drawn at zoom 1 on nothing; `pixel` makes it Pixel text at scale 4.
    fn drawn(pixel: bool) -> tiny_skia::Pixmap {
        static FONTS: std::sync::Once = std::sync::Once::new();
        FONTS.call_once(figcraft_text::scan_system_fonts);
        let mut d = Document::new("pixel text");
        let p = d.first_page().unwrap();
        let mut t = TextData::new("Hi");
        t.style.font_size = 32.0;
        let id = d.create(p, NodeKind::Text(t), "T", 10.0, 10.0, 60.0, 44.0).unwrap();
        d.get_mut(id).unwrap().fills = vec![figcraft_color::Paint::solid(figcraft_color::Color::from_hex("#000000").unwrap())];
        if pixel {
            d.get_mut(id).unwrap().plugin_data.insert(super::TEXT_KEY.into(), "4".into());
        }
        let mut pix = tiny_skia::Pixmap::new(80, 64).unwrap();
        render_nodes_into(&d, &[id], &View { origin: (0.0, 0.0), scale: 1.0, width: 80, height: 64 }, &mut pix);
        pix
    }

    #[test]
    fn pixel_text_is_whole_art_pixels_on_the_grid_with_no_soft_edges() {
        let soft = drawn(false);
        assert!(soft.pixels().iter().any(|p| p.alpha() > 0 && p.alpha() < 255), "plain text is anti-aliased");
        let pix = drawn(true);
        assert!(pix.pixels().iter().all(|p| p.alpha() == 0 || p.alpha() == 255), "no soft edges");
        assert!(pix.pixels().iter().filter(|p| p.alpha() == 255).count() > 64, "the text is drawn");
        // Every 4 × 4 block from the page's origin is one colour (one art pixel).
        for by in 0..16 {
            for bx in 0..20 {
                let first = pix.pixel(bx * 4, by * 4).unwrap();
                for y in 0..4 {
                    for x in 0..4 {
                        assert_eq!(pix.pixel(bx * 4 + x, by * 4 + y).unwrap(), first, "block {bx},{by}");
                    }
                }
            }
        }
    }
}
