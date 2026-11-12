//! Graybox pixel art (G16): a pixel layer's image fill draws its sprite instead of the stored
//! picture, sampled nearest-neighbour so every art pixel stays a sharp square at any zoom.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use figcraft_color::PaintKind;
use figcraft_doc::Node;
use figcraft_geom::Affine;
use figcraft_pixel::{Canvas, Sprite, store};
use tiny_skia::{FillRule, FilterQuality, Pattern, Pixmap, SpreadMode, Transform};

use crate::draw::{Ctx, ts_transform};

/// Pictures kept between renders, at most this many bytes.
const MAX_CACHED_BYTES: usize = 64 << 20;
/// The largest picture a layer is drawn from (art pixels a side).
const MAX_SIDE: u32 = 4096;

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
}

/// What pixel layer `n` shows now (None for any other layer).
pub fn shown(n: &Node, time: f64) -> Option<Shown> {
    if !store::is_pixel(&n.plugin_data) {
        return None;
    }
    let sprite = store::load(&n.plugin_data)?;
    let scale = f64::from(sprite.scale.max(1));
    let art = |len: f64| if len.is_finite() { (len / scale).round().clamp(1.0, f64::from(MAX_SIDE)) as u32 } else { 1 };
    let size = (art(n.width), art(n.height));
    let _ = time;
    Some(Shown { sprite, frame: 0, size })
}

/// The picture of `shown` at its art size, premultiplied for drawing (cached while its cels and
/// size are unchanged).
pub fn picture(n: &Node, shown: &Shown) -> Option<Arc<Pixmap>> {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    store::frame_hash(&n.plugin_data, &shown.sprite, shown.frame).hash(&mut h);
    shown.size.hash(&mut h);
    let key = h.finish();
    if let Ok(c) = cache().lock()
        && let Some(p) = c.pictures.get(&key)
    {
        return Some(p.clone());
    }
    let frame = store::frame_image(&n.plugin_data, &shown.sprite, shown.frame)?;
    let fitted = fit(&frame, shown.size)?;
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

/// The frame at the layer's art size. (G16.3 makes this the smart resize; for now the frame is
/// drawn at its own size from the top left, cut or left empty past it.)
fn fit(frame: &Canvas, (w, h): (u32, u32)) -> Option<Canvas> {
    if (frame.width(), frame.height()) == (w, h) {
        return Some(frame.clone());
    }
    frame.resized(w.min(Canvas::MAX_SIDE), h.min(Canvas::MAX_SIDE), 0, 0)
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
    let Some(shown) = shown(n, ctx.time) else {
        crate::draw::fill_paints(pix, path, &n.fills, n, xf, rule, ctx);
        return;
    };
    let picture = picture(n, &shown);
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
