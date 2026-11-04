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
