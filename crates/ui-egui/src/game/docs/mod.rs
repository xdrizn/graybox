//! Documents (G13) in the app: the Documents tab (the tree of documents) and the editor that takes
//! the canvas' place when a document is open. The model and the `doc.*` commands live in
//! `figcraft_doc::docs` and `figcraft_engine::cmd::docs`; everything here goes through those
//! commands, so each edit is undoable.
//!
//! Hooks in upstream files are listed in `docs/game/STATUS.md` (Upstream hooks).

mod blocks;
mod compare;
mod drag;
mod editor;
mod embed;
mod export;
mod inline;
mod media;
mod select;
mod sign;
mod slash;
mod state;
mod table;
mod tree;
mod versions;

use egui::Ui;

use crate::FigApp;

/// The Documents tab's content (below the left panel's header).
pub(crate) fn panel(app: &mut FigApp, ui: &mut Ui) {
    tree::panel(app, ui);
}

/// Draw the open document in the canvas' place. False when none is open: the canvas shows then.
pub(crate) fn show_editor(app: &mut FigApp, ui: &mut Ui) -> bool {
    editor::show(app, ui)
}

/// Called before the UI runs any command: text typed in a document and not yet sent goes first, so
/// a save or an undo never misses the last words. The `doc.*` commands themselves skip it.
pub(crate) fn before_run(app: &mut FigApp, id: &str) {
    if id.starts_with("doc.block.") || matches!(id, "doc.rename" | "doc.create" | "doc.move" | "doc.delete" | "doc.set_icon") {
        return;
    }
    let Some(uid) = state::uid(app) else { return };
    let mut st = state::take(uid);
    let pending = st.pending.take();
    state::put(uid, st);
    if let Some(p) = pending {
        editor::commit(app, &p);
    }
    // Files that were closed no longer need their editor state.
    if id.starts_with("file.") {
        let open: Vec<u64> = app.session.docs.iter().map(|d| d.uid).collect();
        state::prune(&open);
    }
}

/// The document open in the editor of the file in front (for tests).
#[cfg(test)]
pub(crate) fn open_document(app: &FigApp) -> Option<u64> {
    let uid = state::uid(app)?;
    let st = state::take(uid);
    let open = st.open.map(|d| d.0);
    state::put(uid, st);
    open
}

/// Open document `id` in the editor (the Documents tab shows), for tests.
#[cfg(test)]
pub(crate) fn open(app: &mut FigApp, id: u64) {
    let Some(uid) = state::uid(app) else { return };
    let mut st = state::take(uid);
    st.open = Some(figcraft_doc::docs::DocId(id));
    state::put(uid, st);
    app.ui.left_tab = crate::LeftTab::Documents;
    app.ui.open_left_panel();
}

#[cfg(test)]
mod blocks_tests;
#[cfg(test)]
mod drag_tests;
#[cfg(test)]
mod embed_tests;
#[cfg(test)]
mod format_tests;
#[cfg(test)]
mod media_tests;
#[cfg(test)]
mod mention_tests;
#[cfg(test)]
mod select_tests;
#[cfg(test)]
mod slash_tests;
#[cfg(test)]
mod swatch_tests;
#[cfg(test)]
mod table_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod version_tests;
