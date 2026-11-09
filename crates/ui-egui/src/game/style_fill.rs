//! Graybox: a Fill or Stroke linked to a colour style shows as one row, as in Figma: the style's
//! swatch and name (a click changes the style) and a button to detach it; not the style's colours
//! as rows of their own.

use egui::{CornerRadius, Rect, Sense, Ui, pos2, vec2};
use figcraft_doc::{Document, NodeId, StyleKind};
use serde_json::json;

use crate::FigApp;
use crate::icons::{self, Icon};
use crate::style_picker::StyleSlot;
use crate::theme::{Tokens, regular};
use crate::widgets;

/// The style `fills` (else strokes) of every layer of `ids` link to, when it is one style.
pub(crate) fn linked(doc: &Document, ids: &[NodeId], fills: bool) -> Option<String> {
    crate::right::common(doc, ids, |n| if fills { n.styles.fill.clone() } else { n.styles.stroke.clone() }).flatten()
}

/// Draw the style row when the paints of `ids` link to one style; returns whether it did (the
/// paint rows are left out then).
pub(crate) fn row(app: &mut FigApp, ui: &mut Ui, doc: &Document, ids: &[NodeId], fills: bool) -> bool {
    let Some(id) = linked(doc, ids, fills) else { return false };
    let slot = if fills { StyleSlot::Fill } else { StyleSlot::Stroke };
    let style = doc.style(&id);
    let name = style.map_or_else(|| "Missing style".to_string(), |s| s.name.clone());
    let top = style.and_then(|s| match &s.kind {
        StyleKind::Paint { paints } => paints.last().cloned(),
        _ => None,
    });
    let t = Tokens::get(ui.ctx());
    let mut open = false;
    crate::right::row(ui, 32.0, |ui| {
        let w = (ui.available_width() - 32.0).max(40.0);
        let (r, resp) = ui.allocate_exact_size(vec2(w, widgets::FIELD_H), Sense::click());
        ui.painter().rect_filled(r, CornerRadius::same(5), if resp.hovered() { t.hover } else { t.field });
        let sw = Rect::from_center_size(pos2(r.min.x + 13.0, r.center().y), vec2(14.0, 14.0));
        match &top {
            Some(p) => widgets::paint_swatch(ui.painter(), sw, p),
            None => icons::paint(ui.painter(), sw, Icon::Styles, t.icon_secondary),
        }
        crate::bidi::paint_name(ui.painter(), pos2(r.min.x + 28.0, r.center().y), &name, regular(11.0), t.text, r.width() - 34.0);
        open = resp.on_hover_text("Change style").clicked();
        if icons::button(ui, Icon::Detach, 24.0, false, "Detach style").clicked() {
            app.run("style.detach", json!({"ids": ids, "which": slot.key()}));
        }
    });
    if open {
        crate::style_picker::open(app, ui, ids, slot);
    }
    true
}

#[cfg(test)]
mod tests {
    use figcraft_doc::NodeId;
    use figcraft_engine::Session;
    use serde_json::json;

    /// A rectangle whose fill is a style is linked (one row); after detaching, it isn't.
    #[test]
    fn a_styled_fill_is_one_row_until_detached() {
        let mut s = Session::new();
        let r =
            NodeId(s.execute("shape.rectangle", &json!({"width": 10, "height": 10, "props": {"fill": "#3366ff"}})).unwrap()["id"].as_u64().unwrap());
        let style = s.execute("style.create", &json!({"type": "paint", "name": "Primary", "from": r})).unwrap();
        let sid = style["id"].as_str().map(str::to_string).unwrap_or_default();
        s.execute("style.apply", &json!({"ids": [r], "style": sid, "target": "fill"})).unwrap();
        assert_eq!(super::linked(s.doc().unwrap(), &[r], true).as_deref(), Some(sid.as_str()));
        s.execute("style.detach", &json!({"ids": [r], "which": "fill"})).unwrap();
        assert_eq!(super::linked(s.doc().unwrap(), &[r], true), None);
    }
}
