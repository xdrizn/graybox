//! Graybox: Play redraws a changed screen off the UI thread. A screen with a heavy effect (a
//! full-screen CRT, a big glass) takes a good part of a second to draw, and the game froze for it
//! on every HUD change (a shot, a slot picked). The last image shows until the new one is ready;
//! the newest request wins. In the browser (no threads) the screen draws at once, as before.

use figcraft_doc::{Document, NodeId};
use figcraft_render::Pixmap;

/// The key a screen's image is drawn for: the player's revision, the pixel scale and the frames
/// its animated GIFs show (see the presenter's texture cache).
pub type Key = (u64, u64, u64);

/// A screen's image, drawn off the UI thread.
pub struct Finished {
    pub key: Key,
    pub pix: Pixmap,
    /// Page-space rect the image covers.
    pub bounds: figcraft_geom::Rect,
}

/// What [`Frames::request`] gives back.
pub enum Asked {
    /// A newer image of the screen is ready (perhaps not of the key asked for yet: then the next
    /// request asks again).
    Ready(Finished),
    /// The image is being drawn: keep showing the last one.
    Pending,
    /// No worker here (the browser, or no thread could start): draw it at once.
    Unavailable,
}

/// The screens Play is redrawing off the UI thread.
#[derive(Default)]
pub struct Frames {
    #[cfg(not(target_arch = "wasm32"))]
    worker: Option<worker::Worker>,
    /// The worker could not start: draw at once from now on.
    #[cfg(not(target_arch = "wasm32"))]
    failed: bool,
}

// A copy of a game (tests, a restarted Play) starts with no redraws of its own.
impl Clone for Frames {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl std::fmt::Debug for Frames {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Frames")
    }
}

impl Frames {
    /// Screen `id` of `doc` drawn at `scale` (GIFs at time `t`) for `key`: an image drawn since the
    /// last request, or [`Asked::Pending`] while it is drawn.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn request(&mut self, ctx: &egui::Context, doc: &Document, id: NodeId, scale: f64, t: f64, key: Key) -> Asked {
        if self.failed {
            return Asked::Unavailable;
        }
        if self.worker.is_none() {
            self.worker = worker::Worker::spawn(ctx.clone());
            self.failed = self.worker.is_none();
        }
        let Some(w) = self.worker.as_mut() else { return Asked::Unavailable };
        w.request(doc, id, scale, t, key)
    }

    #[cfg(target_arch = "wasm32")]
    pub fn request(&mut self, _ctx: &egui::Context, _doc: &Document, _id: NodeId, _scale: f64, _t: f64, _key: Key) -> Asked {
        Asked::Unavailable
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod worker {
    use std::collections::HashMap;
    use std::sync::mpsc::{Receiver, Sender, channel};

    use super::{Asked, Document, Finished, Key, NodeId};

    struct Job {
        doc: Document,
        id: NodeId,
        scale: f64,
        t: f64,
        key: Key,
    }

    pub(super) struct Worker {
        jobs: Sender<Job>,
        done: Receiver<(NodeId, Finished)>,
        /// The key last asked for, per screen (asked once).
        asked: HashMap<NodeId, Key>,
        /// Images ready and not handed out yet, per screen (the newest).
        ready: HashMap<NodeId, Finished>,
    }

    impl Worker {
        /// A thread that draws the screens asked for and wakes the UI when one is ready; `None`
        /// when it can't start. It ends when the worker is dropped (Play ends).
        pub(super) fn spawn(ctx: egui::Context) -> Option<Self> {
            let (jobs, rx) = channel::<Job>();
            let (tx, done) = channel();
            std::thread::Builder::new()
                .name("graybox-frames".into())
                .stack_size(figcraft_render::RENDER_STACK)
                .spawn(move || {
                    while let Ok(first) = rx.recv() {
                        // Only the newest request of each screen is drawn.
                        let mut latest: HashMap<NodeId, Job> = HashMap::from([(first.id, first)]);
                        while let Ok(j) = rx.try_recv() {
                            latest.insert(j.id, j);
                        }
                        for job in latest.into_values() {
                            let drawn = figcraft_render::export_bounds(&job.doc, job.id)
                                .and_then(|b| figcraft_render::render_node_at(&job.doc, job.id, job.scale, job.t).map(|pix| (b, pix)));
                            if let Some((bounds, pix)) = drawn
                                && tx.send((job.id, Finished { key: job.key, pix, bounds })).is_err()
                            {
                                return;
                            }
                            ctx.request_repaint();
                        }
                    }
                })
                .ok()?;
            Some(Self { jobs, done, asked: HashMap::new(), ready: HashMap::new() })
        }

        pub(super) fn request(&mut self, doc: &Document, id: NodeId, scale: f64, t: f64, key: Key) -> Asked {
            while let Ok((done_id, f)) = self.done.try_recv() {
                self.ready.insert(done_id, f);
            }
            if self.asked.get(&id) != Some(&key) {
                if self.jobs.send(Job { doc: doc.clone(), id, scale, t, key }).is_err() {
                    return Asked::Unavailable;
                }
                self.asked.insert(id, key);
            }
            match self.ready.remove(&id) {
                Some(f) => Asked::Ready(f),
                None => Asked::Pending,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use figcraft_doc::NodeKind;

    fn screen(w: f64) -> (Document, NodeId) {
        let mut d = Document::new("frames");
        let p = d.first_page().unwrap();
        let f = d.create(p, NodeKind::Frame, "Screen", 0.0, 0.0, w, 50.0).unwrap();
        (d, f)
    }

    /// Ask until an image comes back (a few seconds at most).
    fn wait(f: &mut Frames, ctx: &egui::Context, doc: &Document, id: NodeId, key: Key) -> Finished {
        for _ in 0..500 {
            if let Asked::Ready(done) = f.request(ctx, doc, id, 1.0, 0.0, key) {
                return done;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("no image came back");
    }

    #[test]
    fn a_redraw_comes_back_off_the_ui_thread_and_the_newest_wins() {
        let ctx = egui::Context::default();
        let mut f = Frames::default();
        let (doc, id) = screen(100.0);
        assert!(matches!(f.request(&ctx, &doc, id, 1.0, 0.0, (1, 0, 0)), Asked::Pending), "asked, not drawn here");
        let done = wait(&mut f, &ctx, &doc, id, (1, 0, 0));
        assert_eq!((done.key, done.pix.width(), done.pix.height()), ((1, 0, 0), 100, 50));
        // A wider screen asked for twice before it is drawn: the newest comes back.
        let (wide, _) = screen(200.0);
        let _ = f.request(&ctx, &wide, id, 1.0, 0.0, (2, 0, 0));
        let _ = f.request(&ctx, &wide, id, 1.0, 0.0, (3, 0, 0));
        let mut last = wait(&mut f, &ctx, &wide, id, (3, 0, 0));
        while last.key != (3, 0, 0) {
            last = wait(&mut f, &ctx, &wide, id, (3, 0, 0));
        }
        assert_eq!(last.pix.width(), 200);
    }
}
