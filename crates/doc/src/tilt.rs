//! Graybox: a layer's 3D rotation (G17), stored in its plugin data under [`KEY`] (see
//! [`figcraft_tilt`]). A tilted layer is drawn flat and warped; clicks go back through the warp so
//! what is under the pointer is what gets hit. While a layer inside a tilted one is selected, the
//! tilted one shows flat ("edit flat"): every editing tool then works on it as usual.

use std::cell::RefCell;
use std::sync::Arc;

use figcraft_geom::Point;
pub use figcraft_tilt::{Homography, KEY, NEAR_W, Resize, Tilt};

use crate::{Document, Node, NodeId};

thread_local! {
    /// Tilted layers shown flat for editing while a hit test runs ([`with_flat`]).
    static FLAT: RefCell<Vec<NodeId>> = const { RefCell::new(Vec::new()) };
}

/// The tilt of `n`, if it has one that turns it.
pub fn of(n: &Node) -> Option<Tilt> {
    n.plugin_data.get(KEY).and_then(|s| Tilt::decode(s)).filter(|t| !t.is_flat())
}

/// The warp of `n` in its own space ([`Tilt::local`]), if it is tilted.
pub fn local(n: &Node) -> Option<Homography> {
    of(n).map(|t| t.local(n.transform, n.width, n.height)).filter(|h| !h.is_identity())
}

/// The tilted layers that show flat while `selection` is selected: those with a selected layer
/// inside them (not a selected tilted layer itself, which shows tilted).
pub fn editing_flat(doc: &Document, selection: &[NodeId]) -> Vec<NodeId> {
    let mut out = Vec::new();
    for id in selection {
        for a in doc.ancestors(*id) {
            if !out.contains(&a) && doc.node(a).is_some_and(|n| of(n).is_some()) {
                out.push(a);
            }
        }
    }
    out
}

/// `doc` with the layers of [`editing_flat`] shown flat (their tilt left out); `doc` itself when
/// none is.
pub fn view(doc: Arc<Document>, selection: &[NodeId]) -> Arc<Document> {
    let flat = editing_flat(&doc, selection);
    if flat.is_empty() {
        return doc;
    }
    let mut d = (*doc).clone();
    for id in flat {
        if let Some(n) = d.node_mut(id) {
            n.plugin_data.remove(KEY);
        }
    }
    Arc::new(d)
}

/// The page-space warp a page point of layer `id` (or of what lies inside it) goes through to
/// where it is drawn: its own tilt and each tilted ancestor's, inner first, those in `flat` (shown
/// flat for editing) left out. `None` when nothing tilts it.
pub fn page_warp(doc: &Document, id: NodeId, flat: &[NodeId]) -> Option<Homography> {
    let mut out: Option<Homography> = None;
    for a in std::iter::once(id).chain(doc.ancestors(id)) {
        if flat.contains(&a) {
            continue;
        }
        let Some(h) = doc.node(a).and_then(local) else { continue };
        let world = doc.world_transform(a);
        let page = Homography::from_affine(world) * h * Homography::from_affine(world.inverse());
        out = Some(out.map_or(page, |inner| page * inner));
    }
    out
}

/// Run `f` (a hit test) with the layers `flat` shown flat.
pub fn with_flat<T>(flat: &[NodeId], f: impl FnOnce() -> T) -> T {
    struct Restore(Vec<NodeId>);
    impl Drop for Restore {
        fn drop(&mut self) {
            let old = std::mem::take(&mut self.0);
            FLAT.with(|c| *c.borrow_mut() = old);
        }
    }
    let _restore = Restore(FLAT.with(|c| std::mem::replace(&mut *c.borrow_mut(), flat.to_vec())));
    f()
}

/// Page point `pt` as it lies on tilted layer `n` (`id`) laid flat: the point a click there means
/// for `n` and its layers. `pt` itself when `n` isn't tilted (or shows flat); `None` where nothing
/// of `n` can be (behind the eye).
pub fn flat_point(doc: &Document, id: NodeId, n: &Node, pt: Point) -> Option<Point> {
    let Some(h) = local(n) else { return Some(pt) };
    if FLAT.with(|c| c.borrow().contains(&id)) {
        return Some(pt);
    }
    let world = doc.world_transform(id);
    let inv = h.inverse()?;
    let (x, y, w) = inv.apply_h(world.inverse() * pt);
    let flat = Point::new(x / w, y / w);
    (w > 0.0 && flat.x.is_finite() && flat.y.is_finite() && h.in_front(flat)).then(|| world * flat)
}

#[cfg(test)]
#[path = "tilt_tests.rs"]
mod tests;
