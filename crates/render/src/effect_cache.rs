//! Graybox: costly layers kept between renders (a layer blur, an image fill). A prototype redraws its whole screen when one
//! button changes variant on hover; a large blurred image behind it was blurred again every time
//! (most of a hover's cost). An unchanged layer — the same node (its `Arc` shared by every
//! document copy until it is edited), drawn at the same place, size and scale — reuses its blurred
//! pixels. The cache holds the node's `Arc`, so an entry can never match a different node that
//! happens to reuse its memory.

use std::sync::{Arc, Mutex, PoisonError};

use figcraft_doc::{EffectKind, Node};
use figcraft_geom::Affine;
use tiny_skia::Pixmap;

/// Entries kept, most recently used last.
const MAX_ENTRIES: usize = 8;
/// Pixels kept in all (4 bytes each): about 128 MB.
const MAX_PIXELS: u64 = 32 * 1024 * 1024;

/// Which costly layer a key is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Blur,
    Image,
}

/// What a cached layer was drawn with.
#[derive(Clone)]
pub(crate) struct Key {
    node: Arc<Node>,
    xf: [u64; 6],
    size: (u32, u32),
    scale: u64,
    kind: Kind,
    flags: (bool, bool, bool),
    /// The file's colour profile (images are managed into it).
    profile: String,
    /// The bytes of the images it paints (held: never confused with other bytes at the same place).
    images: Vec<Arc<Vec<u8>>>,
    /// The frames its animated GIF fills show (None: nothing animates).
    frames: Option<u64>,
}

impl Key {
    /// A key for `node`'s layer when its pixels depend only on itself: no children, and a visible
    /// layer blur without backdrop effects ([`Kind::Blur`]) or an image fill ([`Kind::Image`], drawn
    /// straight, checked by the caller). An animated GIF fill keys on the frame it shows at `time`.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        kind: Kind,

        doc: &figcraft_doc::Document,
        id: figcraft_doc::NodeId,
        xf: Affine,
        size: (u32, u32),
        scale: f64,
        flags: (bool, bool, bool),
        time: f64,
    ) -> Option<Self> {
        let node = doc.node_arc(id)?;
        let blurs = node.effects.iter().any(|e| e.visible && matches!(e.kind, EffectKind::LayerBlur(_)));
        let backdrop =
            node.effects.iter().any(|e| e.visible && matches!(e.kind, EffectKind::BackgroundBlur(_) | EffectKind::Glass(_) | EffectKind::Crt(_)));
        let wanted = match kind {
            Kind::Blur => blurs && !backdrop,
            Kind::Image => node.fills.iter().any(|p| p.visible && matches!(p.kind, figcraft_color::PaintKind::Image(_))),
        };
        if !wanted || !node.children.is_empty() || !scale.is_finite() {
            return None;
        }
        let frames = crate::images::animation_clock(doc, id, time).map(|(frames, _)| frames);
        let images = node
            .fills
            .iter()
            .chain(&node.strokes)
            .filter_map(|p| match &p.kind {
                figcraft_color::PaintKind::Image(img) => doc.images.get(&img.image).map(|data| data.bytes.clone()),
                _ => None,
            })
            .collect();
        let profile = format!("{:?}", doc.color_profile);
        let c = xf.as_coeffs();
        Some(Self { node: node.clone(), xf: c.map(f64::to_bits), size, scale: scale.to_bits(), flags, kind, profile, images, frames })
    }

    fn matches(&self, other: &Key) -> bool {
        Arc::ptr_eq(&self.node, &other.node)
            && self.xf == other.xf
            && self.size == other.size
            && self.scale == other.scale
            && self.flags == other.flags
            && self.kind == other.kind
            && self.profile == other.profile
            && self.images.len() == other.images.len()
            && self.images.iter().zip(&other.images).all(|(a, b)| Arc::ptr_eq(a, b))
            && self.frames == other.frames
    }
}

static CACHE: Mutex<Vec<(Key, Pixmap)>> = Mutex::new(Vec::new());

/// The pixels drawn last time for `key`, if any.
pub(crate) fn get(key: &Key) -> Option<Pixmap> {
    let mut cache = CACHE.lock().unwrap_or_else(PoisonError::into_inner);
    let i = cache.iter().position(|(k, _)| k.matches(key))?;
    let entry = cache.remove(i);
    let pixels = entry.1.clone();
    cache.push(entry);
    Some(pixels)
}

/// Keep `layer`'s pixels for `key`, forgetting the least recently used beyond the limits.
pub(crate) fn put(key: Key, layer: &Pixmap) {
    let pixels = u64::from(layer.width()) * u64::from(layer.height());
    if pixels > MAX_PIXELS {
        return;
    }
    let mut cache = CACHE.lock().unwrap_or_else(PoisonError::into_inner);
    cache.retain(|(k, _)| !k.matches(&key));
    cache.push((key, layer.clone()));
    let total = |c: &Vec<(Key, Pixmap)>| c.iter().map(|(_, p)| u64::from(p.width()) * u64::from(p.height())).sum::<u64>();
    while cache.len() > MAX_ENTRIES || (cache.len() > 1 && total(&cache) > MAX_PIXELS) {
        cache.remove(0);
    }
}

#[cfg(test)]
mod tests {
    use crate::paint_behaviour_tests::{page, rect, render};
    use figcraft_color::{Color, Paint};
    use figcraft_doc::{Blur, Effect, EffectKind};

    /// A cached blur draws exactly what a fresh one does, and any change to the layer (or the
    /// document copy it is drawn from) is drawn anew, never from a stale entry.
    #[test]
    fn a_reused_blur_matches_a_fresh_one_and_never_goes_stale() {
        let (mut d, p) = page();
        let r = rect(&mut d, p, (50.0, 50.0, 40.0, 40.0), vec![Paint::solid(Color::BLACK)]);
        d.get_mut(r).unwrap().effects = vec![Effect { kind: EffectKind::LayerBlur(Blur::uniform(8.0)), visible: true }];
        let first = render(&d);
        let copy = d.clone();
        assert_eq!(render(&copy).data(), first.data(), "a document copy sharing the layer reuses it, pixel for pixel");
        let mut edited = d.clone();
        edited.get_mut(r).unwrap().effects = vec![Effect { kind: EffectKind::LayerBlur(Blur::uniform(2.0)), visible: true }];
        let sharper = render(&edited);
        assert_ne!(sharper.data(), first.data(), "an edited blur is drawn again");
        let mut moved = d.clone();
        moved.get_mut(r).unwrap().transform = figcraft_geom::Affine::translate((60.0, 50.0));
        assert_ne!(render(&moved).data(), first.data(), "a moved layer is drawn again");
        assert_eq!(render(&d).data(), first.data(), "the original still matches");
        // The presenter draws with a clock (for GIFs): a layer without animation still reuses its blur.
        let at = |t: f64| crate::render_node_at(&d, r, 1.0, t).unwrap();
        assert_eq!(at(0.0).data(), at(12.5).data());
        assert!(
            super::Key::new(super::Kind::Blur, &d, r, figcraft_geom::Affine::IDENTITY, (40, 40), 1.0, (false, false, false), 12.5).is_some(),
            "no GIF: time doesn't matter"
        );
    }
}
