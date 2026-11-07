//! Graybox: fonts installed while the app runs show up without restarting it. When the window
//! comes back to the front (after installing a font in Windows, say), the font folders are looked
//! at in the background: if a file came or went since the last look, they are walked again and the
//! new families join the font list. The open file isn't touched (a walk reports most families as
//! changed even when nothing was installed, so nothing is refitted from it). Nothing happens on the
//! web (the browser lists its own fonts).

use crate::FigApp;

#[cfg(not(target_arch = "wasm32"))]
use std::sync::{Arc, Mutex, PoisonError};

/// What the font folders held at the last look: each folder's change time and number of entries.
#[cfg(not(target_arch = "wasm32"))]
type Signature = Vec<(std::path::PathBuf, Option<std::time::SystemTime>, usize)>;

#[derive(Default)]
pub(crate) struct Watch {
    /// Whether the window had the focus last frame (None before the first frame: opening the
    /// window is not coming back to it).
    focused: Option<bool>,
    /// The font folders at the last look (None until the first one, at start).
    #[cfg(not(target_arch = "wasm32"))]
    seen: Arc<Mutex<Option<Signature>>>,
    /// A look (and maybe a walk) is running.
    #[cfg(not(target_arch = "wasm32"))]
    busy: Arc<std::sync::atomic::AtomicBool>,
}

/// Once a frame: look at the font folders at start (to know what was there) and each time the
/// window comes back to the front.
pub(crate) fn tick(app: &mut FigApp, ctx: &egui::Context) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::sync::atomic::Ordering;
        let focused = ctx.input(|i| i.focused);
        let first = app.font_watch.focused.is_none();
        let back = comes_back(app.font_watch.focused, focused);
        app.font_watch.focused = Some(focused);
        if !(first || back) || app.font_watch.busy.swap(true, Ordering::AcqRel) {
            return;
        }
        let (seen, busy, repaint) = (app.font_watch.seen.clone(), app.font_watch.busy.clone(), ctx.clone());
        let started = std::thread::Builder::new().name("graybox-font-watch".into()).spawn(move || {
            let now = signature();
            let before = seen.lock().unwrap_or_else(PoisonError::into_inner).replace(now.clone());
            // At start there is nothing to compare with; later, a folder that changed is walked again.
            if before.is_some_and(|b| b != now) && figcraft_text::system_fonts_scanned() {
                let found = figcraft_text::rescan_system_fonts();
                if !found.added.is_empty() {
                    log::info!("fonts installed while running: {:?}", found.added);
                }
                repaint.request_repaint();
            }
            busy.store(false, Ordering::Release);
        });
        if started.is_err() {
            app.font_watch.busy.store(false, Ordering::Release);
        }
    }
    #[cfg(target_arch = "wasm32")]
    let _ = (app, ctx);
}

/// The folders fonts are installed into on this system, and what each holds now.
#[cfg(not(target_arch = "wasm32"))]
fn signature() -> Signature {
    font_dirs()
        .into_iter()
        .map(|dir| {
            let changed = std::fs::metadata(&dir).and_then(|m| m.modified()).ok();
            let entries = std::fs::read_dir(&dir).map(|d| d.take(1_000_000).count()).unwrap_or(0);
            (dir, changed, entries)
        })
        .collect()
}

/// Where fonts get installed (what a user installing a font changes).
#[cfg(not(target_arch = "wasm32"))]
fn font_dirs() -> Vec<std::path::PathBuf> {
    use std::path::PathBuf;
    let env = |k: &str| std::env::var_os(k).map(PathBuf::from);
    let mut v: Vec<PathBuf> = Vec::new();
    if cfg!(target_os = "windows") {
        v.push(env("WINDIR").unwrap_or_else(|| "C:\\Windows".into()).join("Fonts"));
        v.extend(env("LOCALAPPDATA").map(|l| l.join("Microsoft\\Windows\\Fonts")));
    } else if cfg!(target_os = "macos") {
        v.push("/Library/Fonts".into());
        v.extend(env("HOME").map(|h| h.join("Library/Fonts")));
    } else {
        v.push("/usr/share/fonts".into());
        v.push("/usr/local/share/fonts".into());
        if let Some(h) = env("HOME") {
            v.push(h.join(".fonts"));
            v.push(h.join(".local/share/fonts"));
        }
    }
    v
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

    /// Looking at the font folders twice with nothing installed in between sees the same thing (so
    /// no walk, and the open file is never touched).
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn nothing_installed_looks_the_same() {
        assert_eq!(super::signature(), super::signature());
    }
}
