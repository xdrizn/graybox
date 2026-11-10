//! Graybox: Unity export in the Prototype tab. With a screen (top-level frame) selected, "Export to
//! Unity" marks it; with nothing selected, the "Unity export" part picks the folder and exports
//! the marked screens now. Each time the file is saved, the marked screens are exported again, and
//! the Graybox Live Link package in Unity (`integrations/unity/`) rebuilds them.

use egui::Ui;
use figcraft_doc::{Document, Node};
use serde_json::json;

use crate::FigApp;
use crate::theme::Tokens;
use crate::widgets;

/// The selected screen's switch.
pub(crate) fn frame_section(app: &mut FigApp, ui: &mut Ui, doc: &Document, node: &Node) {
    if doc.top_level_frame(node.id) != Some(node.id) {
        return;
    }
    let id = node.id;
    widgets::separator(ui);
    widgets::section_header(ui, "Unity", &[], false);
    super::padded(ui, |ui| {
        let marked = doc.game.unity_frames.contains(&id);
        if widgets::checkbox(ui, marked, "Export to Unity") {
            app.run("game.unity.export", json!({"frame": id, "mark": !marked}));
        }
        let t = Tokens::get(ui.ctx());
        let note = if doc.game.unity_dir.is_empty() {
            "Choose the Unity export folder in the Prototype tab (nothing selected)."
        } else {
            "Exported again each time you save; Graybox Live Link rebuilds it in Unity."
        };
        widgets::note(ui, note, t.text_secondary);
    });
}

/// The "Unity export" part of the Prototype tab (nothing selected): the folder and Export now.
pub(crate) fn settings(app: &mut FigApp, ui: &mut Ui) {
    let Ok(doc) = app.session.doc() else { return };
    let dir = doc.game.unity_dir.clone();
    let marked = doc.game.unity_frames.len();
    let t = Tokens::get(ui.ctx());
    let shown = if dir.is_empty() { "No folder chosen".to_string() } else { dir.clone() };
    widgets::label(ui, "Folder", t.text_secondary);
    widgets::label(ui, &shown, if dir.is_empty() { t.text_secondary } else { t.text });
    ui.horizontal(|ui| {
        if widgets::button(ui, "Choose folder…", false, true).clicked() {
            app.pick_folder("Unity export folder", |app, picked| {
                app.run("game.unity.export", json!({"setDir": picked.to_string_lossy()}));
            });
        }
        if widgets::button(ui, "Export now", true, !dir.is_empty() && marked > 0).clicked() {
            export(app, true);
        }
    });
    let note = format!(
        "{marked} screen{} marked (select a screen: Export to Unity). Saved files export again by themselves. In Unity, add the Graybox Live Link package (integrations/unity) and point it at this folder.",
        if marked == 1 { "" } else { "s" }
    );
    widgets::note(ui, &note, t.text_secondary);
}

/// Export the marked screens; `tell` shows how it went.
fn export(app: &mut FigApp, tell: bool) {
    match app.session.execute("game.unity.export", &json!({})) {
        Ok(r) => {
            if tell {
                let n = r["screens"].as_array().map_or(0, Vec::len);
                app.toast(format!("Exported {n} screen{} for Unity", if n == 1 { "" } else { "s" }));
            }
        }
        Err(e) => {
            if tell {
                app.toast(e.to_string());
            }
        }
    }
}

/// Once a frame: when the file has just been saved (it was unsaved last frame and isn't now) and
/// has screens marked and a folder, export them again.
pub(crate) fn tick(app: &mut FigApp) {
    let Ok(ds) = app.session.ds() else { return };
    let key = (ds.uid, ds.is_dirty());
    let was = SAVED.with(|s| s.replace(Some(key)));
    let just_saved = matches!(was, Some((uid, true)) if uid == key.0 && !key.1);
    if just_saved && !ds.doc.game.unity_dir.is_empty() && !ds.doc.game.unity_frames.is_empty() {
        export(app, false);
    }
}

thread_local! {
    /// The file shown last frame and whether it was unsaved.
    static SAVED: std::cell::Cell<Option<(u64, bool)>> = const { std::cell::Cell::new(None) };
}
