//! Getting a document out of the editor (G13.3): a PDF or Markdown file, or a card on the canvas.
//! Both go through the `doc.export` and `doc.card.create` commands.

use egui::Pos2;
use figcraft_doc::NodeId;
use figcraft_doc::docs::DocId;
use serde_json::json;

use super::state::State;
use crate::FigApp;
use crate::i18n::{tr, trf};

/// A file name for a document title: no characters a file system refuses.
pub(super) fn file_stem(title: &str) -> String {
    let clean: String = title.chars().map(|c| if c.is_control() || "/\\:*?\"<>|".contains(c) { '-' } else { c }).collect();
    let clean = clean.trim().trim_matches('.').to_string();
    if clean.is_empty() { "Untitled".to_string() } else { clean.chars().take(80).collect() }
}

fn title_of(app: &FigApp, doc: DocId) -> String {
    app.session.doc().ok().and_then(|d| d.docs.get(doc)).map(|d| d.title.clone()).unwrap_or_default()
}

/// Ask where to save the document as `format` (`pdf` or `md`) and write it there.
pub(super) fn export(app: &mut FigApp, doc: DocId, format: &'static str) {
    let name = format!("{}.{format}", file_stem(&title_of(app, doc)));
    if app.services.downloads() {
        // The browser keeps no files of its own: the bytes are handed over as a download.
        let Some(r) = app.run("doc.export", json!({"doc": doc.0, "format": format})) else { return };
        let data = if format == "pdf" {
            r
        } else {
            use base64::Engine as _;
            json!({"data": base64::engine::general_purpose::STANDARD.encode(r["text"].as_str().unwrap_or_default())})
        };
        if app.download(&name, &data) {
            app.toast(trf("Exported {name}", &[("name", &name)]));
        }
        return;
    }
    app.pick_save(tr("Export"), &name, &[format], move |app, p| {
        if app.run("doc.export", json!({"doc": doc.0, "format": format, "path": p.to_string_lossy()})).is_some() {
            let shown = p.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
            app.toast(trf("Exported {name}", &[("name", &shown)]));
        }
    });
}

/// Put the document on the canvas as a card at `at` (a point of the window; the middle of the
/// canvas when none), select it, and show the canvas.
pub(super) fn put_on_canvas(app: &mut FigApp, st: &mut State, doc: DocId, at: Option<Pos2>) {
    let p = at.unwrap_or_else(|| app.canvas.rect.center());
    let pt = app.to_page(p);
    // The point is where the middle of the first page goes.
    let (x, y) = (pt.x - 297.0, pt.y - 60.0);
    let Some(r) = app.run("doc.card.create", json!({"doc": doc.0, "x": x, "y": y})) else { return };
    if let Some(id) = r["id"].as_u64() {
        app.session.set_selection(vec![NodeId(id)]);
    }
    // The canvas was never drawn (its area is unknown), so the middle above is a guess: show the card.
    if !app.canvas.rect.is_positive() {
        app.run("view.zoomSelection", json!({}));
    }
    st.open = None;
    app.toast(tr("Document put on the canvas"));
}
