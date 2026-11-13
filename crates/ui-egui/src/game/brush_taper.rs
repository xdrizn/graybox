//! Graybox: Stroke settings ▸ Brush ▸ Taper: turn off the thinning at a brush stroke's ends (a
//! rectangle then has no thin corner where its path starts), or set how long each end tapers.

use egui::{Id, Ui};
use figcraft_doc::{Brush, NodeId};
use serde_json::json;

use crate::FigApp;
use crate::widgets::{self, Lead};

/// The panel's row layout: a row `h` tall with the section's margins, its controls added by the
/// closure.
pub(crate) type RowFn<'a> = &'a dyn Fn(&mut Ui, f32, &mut dyn FnMut(&mut Ui));

/// The Taper rows for brush `b` on layers `ids`.
pub(crate) fn rows(app: &mut FigApp, ui: &mut Ui, ids: &[NodeId], b: &Brush, panel_w: f32, row: RowFn) {
    let half = widgets::col_width(panel_w);
    let with = |f: &dyn Fn(&mut Brush)| {
        let mut nb = b.clone();
        f(&mut nb);
        serde_json::to_value(nb.sanitized()).unwrap_or_default()
    };
    let mut toggled = false;
    row(ui, 32.0, &mut |ui| toggled = widgets::checkbox(ui, b.taper, "Taper the ends"));
    if toggled {
        app.run("node.set", json!({"ids": ids, "props": {"brush": with(&|nb| nb.taper = !nb.taper)}}));
    }
    if !b.taper {
        return;
    }
    let (start, end) = figcraft_pathops::organic::taper_of(b);
    widgets::sublabels(ui, "Taper start", Some("Taper end"));
    row(ui, 32.0, &mut |ui| {
        let e = widgets::number_field(ui, Id::unique("brush_taper_start"), Lead::Named("Taper start"), Some(start * 100.0), "%", half, 1.0);
        app.field(ids, "brush", e, |v| with(&|nb| nb.taper_start = Some(v / 100.0)));
        let e = widgets::number_field(ui, Id::unique("brush_taper_end"), Lead::Named("Taper end"), Some(end * 100.0), "%", half, 1.0);
        app.field(ids, "brush", e, |v| with(&|nb| nb.taper_end = Some(v / 100.0)));
    });
}

/// The organic brushes' names (`figcraft_pathops::brush::PRESETS`), shown through `tr` by the
/// Brush menu.
#[allow(dead_code)]
const PRESET_LABELS: [&str; 3] = [crate::i18n::tr_noop("Brush pen"), crate::i18n::tr_noop("Dry brush"), crate::i18n::tr_noop("Chalk")];
