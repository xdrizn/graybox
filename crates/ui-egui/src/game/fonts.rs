//! Fonts picked from the font store's list (Google Fonts, Noto) are downloaded to this computer
//! when picked: only the families used, only locally. While downloading is off, the Missing fonts
//! window asks the user to allow it first (the only place that turns it on).

use crate::FigApp;
use serde_json::json;

/// Start downloading `family` if this computer doesn't have it and the font store does.
pub fn fetch(app: &mut FigApp, family: &str) {
    if figcraft_text::has_family(family) || figcraft_engine::fonts::source(family).is_none() {
        return;
    }
    if figcraft_engine::fonts::downloads_allowed() {
        app.run("font.download", json!({"family": family}));
    } else {
        // Lists the file's missing families, this one among them, with Download (after Allow).
        crate::dialogs::missing_fonts::request(app);
    }
}

/// Fonts that came inside the file (a shared prototype): load the ones this computer or browser
/// doesn't have, once each, so its text shows as designed. Runs every frame; cheap when done.
pub fn load_shared(app: &FigApp) {
    use base64::Engine as _;
    use std::collections::BTreeSet;
    use std::sync::{Mutex, PoisonError};
    static LOADED: Mutex<BTreeSet<(String, usize)>> = Mutex::new(BTreeSet::new());
    let Ok(doc) = app.session.doc() else { return };
    if doc.game.fonts.is_empty() {
        return;
    }
    let mut loaded = LOADED.lock().unwrap_or_else(PoisonError::into_inner);
    for (family, files) in &doc.game.fonts {
        let lower = family.to_lowercase();
        // Installed here (not by us): this computer's own files win.
        if figcraft_text::has_family(family) && !loaded.iter().any(|(f, _)| *f == lower) {
            continue;
        }
        for (i, text) in files.iter().enumerate() {
            let key = (lower.clone(), i);
            if loaded.contains(&key) {
                continue;
            }
            loaded.insert(key);
            if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(text) {
                figcraft_text::load_font_bytes(bytes);
            }
        }
    }
}
