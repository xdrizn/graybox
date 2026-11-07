//! Graybox: fonts installed while the app runs show up without restarting it. Each time the
//! window comes back to the front (after installing a font in Windows, say), the system font
//! folders are walked again in the background; when a family is new or gained styles, the open
//! file's text is fitted to it. Nothing happens on the web (the browser lists its own fonts).

use crate::FigApp;

#[cfg(not(target_arch = "wasm32"))]
use std::sync::{Arc, Mutex, PoisonError};

/// The walk in flight, if any: its result once it finishes.
#[cfg(not(target_arch = "wasm32"))]
type Pending = Arc<Mutex<Option<figcraft_text::fontdb::Rescan>>>;

#[derive(Default)]
pub(crate) struct Watch {
    /// Whether the window had the focus last frame (None before the first frame: opening the
    /// window is not coming back to it).
    focused: Option<bool>,
    #[cfg(not(target_arch = "wasm32"))]
    pending: Option<Pending>,
}

/// Once a frame: start a walk when the window gains the focus, apply a finished one.
pub(crate) fn tick(app: &mut FigApp, ctx: &egui::Context) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let focused = ctx.input(|i| i.focused);
        let gained = comes_back(app.font_watch.focused, focused);
        app.font_watch.focused = Some(focused);
        if gained && app.font_watch.pending.is_none() && figcraft_text::system_fonts_scanned() {
            let slot: Pending = Arc::new(Mutex::new(None));
            let out = slot.clone();
            let repaint = ctx.clone();
            let started = std::thread::Builder::new().name("graybox-font-rescan".into()).spawn(move || {
                let found = figcraft_text::rescan_system_fonts();
                *out.lock().unwrap_or_else(PoisonError::into_inner) = Some(found);
                repaint.request_repaint();
            });
            if started.is_ok() {
                app.font_watch.pending = Some(slot);
            }
        }
        let done = app.font_watch.pending.as_ref().and_then(|p| p.lock().unwrap_or_else(PoisonError::into_inner).take());
        if let Some(found) = done {
            app.font_watch.pending = None;
            if !found.changed.is_empty() && app.session.doc().is_ok() {
                // New faces: auto-sizing text in the open file fits them, and the canvas redraws.
                app.run("text.refit", serde_json::json!({}));
                ctx.request_repaint();
            }
        }
    }
    #[cfg(target_arch = "wasm32")]
    let _ = (app, ctx);
}

/// Whether the window came back to the front: it had no focus last frame and has it now (the
/// first frame, which opens the window, never counts).
#[cfg(any(test, not(target_arch = "wasm32")))]
fn comes_back(before: Option<bool>, now: bool) -> bool {
    now && before == Some(false)
}

#[cfg(test)]
mod tests {
    use super::comes_back;

    #[test]
    fn only_coming_back_to_the_window_walks_the_fonts_again() {
        assert!(!comes_back(None, true), "opening the window");
        assert!(!comes_back(Some(true), true), "staying in front");
        assert!(!comes_back(Some(true), false), "leaving");
        assert!(comes_back(Some(false), true), "coming back");
    }
}
