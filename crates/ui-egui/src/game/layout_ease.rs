//! Graybox: when a layer goes into or out of an auto layout frame (or moves to another place in
//! it), the frame's other layers glide to their new places with an ease in and out instead of
//! jumping, so it shows what the drop moves. Only the canvas picture glides: the file already
//! holds the new places, and selection boxes and handles use them.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};

use figcraft_doc::{Document, NodeId};
use figcraft_geom::{Affine, Vec2, translation};

/// How long a glide takes (seconds).
const SECS: f64 = 0.18;

/// A layer drawn `from` away from its place at `start`, closing the gap over [`SECS`].
#[derive(Clone, Copy, Debug)]
struct Glide {
    from: Vec2,
    start: f64,
}

impl Glide {
    /// The gap still left at `now` (zero once the glide is over).
    fn offset(&self, now: f64) -> Vec2 {
        let t = ((now - self.start) / SECS).clamp(0.0, 1.0);
        // Ease in and out (cubic).
        let eased = if t < 0.5 { 4.0 * t * t * t } else { 1.0 - (-2.0 * t + 2.0).powi(3) / 2.0 };
        self.from * (1.0 - eased)
    }
}

/// What the canvas showed last, for one file and page.
#[derive(Default)]
pub(crate) struct Ease {
    shown: Option<(u64, NodeId)>,
    last: Option<Arc<Document>>,
    glides: BTreeMap<NodeId, Glide>,
}

impl Ease {
    /// The document to draw at `now` (seconds) for file `uid`, page `page`: `doc` itself, or a copy
    /// with gliding layers drawn short of their places. `held` (the selection) never glides.
    pub(crate) fn view(&mut self, uid: u64, page: NodeId, doc: Arc<Document>, now: f64, held: &[NodeId]) -> Arc<Document> {
        if self.shown != Some((uid, page)) {
            *self = Self { shown: Some((uid, page)), last: Some(doc.clone()), glides: BTreeMap::new() };
            return doc;
        }
        if let Some(last) = self.last.take()
            && !Arc::ptr_eq(&last, &doc)
        {
            self.start(&last, &doc, now, held);
        }
        self.last = Some(doc.clone());
        self.glides.retain(|_, g| now.is_finite() && (now - g.start) / SECS < 1.0 - 1e-9);
        if self.glides.is_empty() {
            return doc;
        }
        let mut shown = (*doc).clone();
        for (id, g) in &self.glides {
            let off = g.offset(now);
            if let Ok(n) = shown.get_mut(*id) {
                n.transform = Affine::translate(off) * n.transform;
            }
        }
        Arc::new(shown)
    }

    /// Whether layers are still gliding (the canvas keeps redrawing).
    pub(crate) fn moving(&self) -> bool {
        !self.glides.is_empty()
    }

    /// Start glides for the layers an edit from `before` to `after` moved inside auto layout
    /// frames whose layers changed (one came in, went out or moved to another place).
    fn start(&mut self, before: &Document, after: &Document, now: f64, held: &[NodeId]) {
        for frame in after.nodes() {
            if frame.layout.is_none() || !frame.kind.is_frame_like() {
                continue;
            }
            let Some(old) = before.node(frame.id) else { continue };
            if old.children == frame.children {
                continue;
            }
            for child in &frame.children {
                if held.contains(child) || !old.children.contains(child) {
                    continue;
                }
                let (Some(was), Some(is)) = (before.node(*child), after.node(*child)) else { continue };
                let gap = translation(was.transform) - translation(is.transform);
                if !(gap.x.is_finite() && gap.y.is_finite()) || gap.hypot() < 0.5 {
                    continue;
                }
                // A layer already gliding starts from where it is drawn now.
                let from = gap + self.glides.get(child).map_or(Vec2::ZERO, |g| g.offset(now));
                self.glides.insert(*child, Glide { from, start: now });
            }
        }
    }
}

static EASE: Mutex<Option<Ease>> = Mutex::new(None);

/// The clock glides run on (seconds since the first call).
fn clock() -> f64 {
    static START: std::sync::OnceLock<web_time::Instant> = std::sync::OnceLock::new();
    START.get_or_init(web_time::Instant::now).elapsed().as_secs_f64()
}

/// The canvas hook: the document to draw for the shown file and page (see [`Ease::view`]).
pub(crate) fn view(uid: u64, page: NodeId, doc: Arc<Document>, held: &[NodeId]) -> Arc<Document> {
    let mut ease = EASE.lock().unwrap_or_else(PoisonError::into_inner);
    ease.get_or_insert_with(Ease::default).view(uid, page, doc, clock(), held)
}

/// Whether the canvas must keep redrawing for a glide.
pub(crate) fn moving() -> bool {
    EASE.lock().unwrap_or_else(PoisonError::into_inner).as_ref().is_some_and(Ease::moving)
}

#[cfg(test)]
mod tests {
    use super::*;
    use figcraft_engine::Session;
    use serde_json::json;

    fn id(v: &serde_json::Value) -> NodeId {
        NodeId(v["id"].as_u64().unwrap())
    }

    fn x(doc: &Document, n: NodeId) -> f64 {
        translation(doc.node(n).unwrap().transform).x
    }

    /// A layer dropped in front of others in an auto layout row: the others glide from their old
    /// places to the new ones (eased, done after the glide), the dropped layer doesn't, and the
    /// file keeps the new places all along.
    #[test]
    fn layers_glide_when_one_joins_an_auto_layout_frame() {
        let mut s = Session::new();
        let row = id(&s.execute("frame.create", &json!({"width": 300, "height": 50, "props": {"layout": "horizontal", "gap": 10}})).unwrap());
        let a = id(&s.execute("shape.rectangle", &json!({"parent": row, "width": 40, "height": 40})).unwrap());
        let b = id(&s.execute("shape.rectangle", &json!({"parent": row, "width": 40, "height": 40})).unwrap());
        let loose = id(&s.execute("shape.rectangle", &json!({"x": 0, "y": 200, "width": 40, "height": 40})).unwrap());
        let page = s.ds().unwrap().page;
        let mut ease = Ease::default();
        let before = s.ds().unwrap().doc.clone();
        assert!(Arc::ptr_eq(&ease.view(1, page, before.clone(), 0.0, &[]), &before), "nothing glides at first");
        s.execute("node.reparent", &json!({"ids": [loose], "parent": row, "index": 0})).unwrap();
        let after = s.ds().unwrap().doc.clone();
        assert_eq!((x(&after, a), x(&after, b)), (50.0, 100.0), "the file has the new places");
        let start = ease.view(1, page, after.clone(), 1.0, &[loose]);
        assert_eq!((x(&start, a), x(&start, b)), (0.0, 50.0), "drawn where they were");
        assert_eq!(x(&start, loose), x(&after, loose), "the dropped layer doesn't glide");
        let mid = ease.view(1, page, after.clone(), 1.0 + SECS / 2.0, &[loose]);
        assert!((x(&mid, a) - 25.0).abs() < 1e-6, "halfway at half time: {}", x(&mid, a));
        assert!(ease.moving());
        let end = ease.view(1, page, after.clone(), 1.0 + SECS, &[loose]);
        assert!(Arc::ptr_eq(&end, &after) && !ease.moving(), "done: the file's own picture");
        // Another file or page starts over without gliding.
        let other = ease.view(2, page, before.clone(), 2.0, &[]);
        assert!(Arc::ptr_eq(&other, &before));
    }

    /// Edits that don't change which layers an auto layout holds (a colour, a resize) don't glide.
    #[test]
    fn other_edits_dont_glide() {
        let mut s = Session::new();
        let row = id(&s.execute("frame.create", &json!({"width": 300, "height": 50, "props": {"layout": "horizontal", "gap": 10}})).unwrap());
        let a = id(&s.execute("shape.rectangle", &json!({"parent": row, "width": 40, "height": 40})).unwrap());
        s.execute("shape.rectangle", &json!({"parent": row, "width": 40, "height": 40})).unwrap();
        let page = s.ds().unwrap().page;
        let mut ease = Ease::default();
        ease.view(1, page, s.ds().unwrap().doc.clone(), 0.0, &[]);
        s.execute("node.set", &json!({"ids": [a], "props": {"width": 80}})).unwrap();
        let after = s.ds().unwrap().doc.clone();
        assert!(Arc::ptr_eq(&ease.view(1, page, after.clone(), 0.1, &[]), &after));
    }
}
