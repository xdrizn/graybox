//! Graybox: the pseudo-localization switch (Prototype tab) and the canvas showing it. While on, the
//! canvas draws every text as a longer, accented stand-in translation
//! ([`figcraft_engine::pseudo`]) with the layout made again around it, and says so at its top. The
//! file is never changed; turning it off shows the real text again.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use figcraft_doc::Document;

static ON: AtomicBool = AtomicBool::new(false);

/// The last document shown and its pseudo-localized copy.
static CACHE: Mutex<Option<(Arc<Document>, Arc<Document>)>> = Mutex::new(None);

pub(crate) fn on() -> bool {
    ON.load(Ordering::Relaxed)
}

pub(crate) fn set(on: bool) {
    ON.store(on, Ordering::Relaxed);
    if !on {
        *CACHE.lock().unwrap_or_else(PoisonError::into_inner) = None;
    }
}

/// What the canvas draws for `doc`: itself, or while the switch is on its pseudo-localized copy
/// (made again only when the document changes).
pub(crate) fn view(doc: Arc<Document>) -> Arc<Document> {
    if !on() {
        return doc;
    }
    let mut cache = CACHE.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some((src, shown)) = cache.as_ref()
        && Arc::ptr_eq(src, &doc)
    {
        return shown.clone();
    }
    let shown = Arc::new(figcraft_engine::pseudo::preview(&doc));
    *cache = Some((doc, shown.clone()));
    shown
}

/// The switch, at the top of the game sections of the Prototype tab.
pub(crate) fn switch(ui: &mut egui::Ui) {
    let mut v = on();
    if crate::widgets::checkbox(ui, v, "Pseudo-localization preview") {
        v = !v;
        set(v);
        ui.ctx().request_repaint();
    }
    crate::widgets::note(
        ui,
        "Shows every text as a longer, accented stand-in translation, to find labels that get cut or overflow. The file isn't changed.",
        crate::theme::Tokens::get(ui.ctx()).text_secondary,
    );
}

/// While on, a label at the top of the canvas says so.
pub(crate) fn banner(painter: &egui::Painter, canvas: egui::Rect) {
    if !on() {
        return;
    }
    let text = "Pseudo-localization preview";
    let center = egui::pos2(canvas.center().x, canvas.min.y + 18.0);
    let galley = painter.layout_no_wrap(text.to_string(), crate::theme::semibold(11.0), egui::Color32::WHITE);
    let rect = egui::Rect::from_center_size(center, galley.size() + egui::vec2(16.0, 8.0));
    painter.rect_filled(rect, 4, crate::theme::ACCENT);
    painter.galley(rect.min + egui::vec2(8.0, 4.0), galley, egui::Color32::WHITE);
}
