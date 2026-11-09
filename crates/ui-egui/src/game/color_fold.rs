//! Graybox: the Selection colors header folds like Figma's. Folded, it previews the first three
//! colours and "+N"; a click on the header opens or closes the list. Up to three colours start
//! open, more start folded; the choice is kept while the app runs.

use egui::{Align2, Id, Rect, Sense, Ui, pos2, vec2};
use figcraft_doc::NodeId;
use figcraft_engine::selection_colors::SelectionColor;

use crate::icons::{self, Icon};
use crate::theme::Tokens;
use crate::widgets;

/// Colours shown in the folded header.
const PREVIEW: usize = 3;

/// Whether the list starts open for `count` colours (before the player opens or closes it).
pub(crate) fn opens_by_default(count: usize) -> bool {
    count <= PREVIEW
}

/// Whether the section shows for `colors` of selection `ids`: two colours or more, or one that
/// comes from more than the one layer selected (a frame's layers, or several layers). One colour on
/// the one selected layer is its Fill or Stroke already.
pub(crate) fn shown(colors: &[SelectionColor], ids: &[NodeId]) -> bool {
    match colors {
        [] => false,
        [one] => ids.len() > 1 || one.targets.iter().any(|t| !ids.contains(&t.id)),
        _ => true,
    }
}

/// Draw the header for `colors`; returns whether the list below it is open.
pub(crate) fn header(ui: &mut Ui, title: &str, colors: &[SelectionColor]) -> bool {
    let key = Id::unique("graybox_selection_colors_open");
    let open = ui.data(|d| d.get_temp::<bool>(key)).unwrap_or_else(|| opens_by_default(colors.len()));
    let t = Tokens::get(ui.ctx());
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 40.0), Sense::click());
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    if resp.clicked() {
        ui.data_mut(|d| d.insert_temp(key, !open));
    }
    let chevron = Rect::from_center_size(pos2(rect.min.x + widgets::PAD - 6.0, rect.center().y), vec2(10.0, 10.0));
    if resp.hovered() || !open {
        icons::paint(ui.painter(), chevron, if open { Icon::Chevron } else { Icon::ChevronRight }, t.icon_secondary);
    }
    ui.painter().text(pos2(rect.min.x + widgets::PAD + 2.0, rect.center().y), Align2::LEFT_CENTER, title, crate::theme::semibold(11.0), t.text);
    if !open {
        let more = colors.len().saturating_sub(PREVIEW);
        let mut x = rect.max.x - widgets::PAD_RIGHT;
        if more > 0 {
            let r =
                ui.painter().text(pos2(x, rect.center().y), Align2::RIGHT_CENTER, format!("+{more}"), crate::theme::regular(11.0), t.text_secondary);
            x = r.min.x - 8.0;
        }
        for c in colors.iter().take(PREVIEW).rev() {
            let r = Rect::from_min_size(pos2(x - 16.0, rect.center().y - 8.0), vec2(16.0, 16.0));
            widgets::paint_swatch(ui.painter(), r, &c.paint);
            x = r.min.x - 6.0;
        }
    }
    let toggled = resp.clicked();
    open != toggled
}

#[cfg(test)]
mod tests {
    use figcraft_doc::NodeId;
    use figcraft_engine::Session;
    use serde_json::json;

    /// A frame (no fill) holding layers of one colour shows that colour; a lone layer doesn't.
    #[test]
    fn one_colour_inside_a_frame_shows() {
        let mut s = Session::new();
        let f = NodeId(s.execute("frame.create", &json!({"width": 100, "height": 100, "props": {"fill": "none"}})).unwrap()["id"].as_u64().unwrap());
        let mut kids = Vec::new();
        for x in [0, 30] {
            let r = s.execute("shape.rectangle", &json!({"parent": f, "x": x, "width": 20, "height": 20, "props": {"fill": "#5effd0"}})).unwrap();
            kids.push(NodeId(r["id"].as_u64().unwrap()));
        }
        let doc = s.doc().unwrap();
        let colors = figcraft_engine::selection_colors::collect(doc, &[f]);
        assert_eq!(colors.len(), 1);
        assert!(super::shown(&colors, &[f]), "a frame's one colour");
        let own = figcraft_engine::selection_colors::collect(doc, &kids[..1]);
        assert!(!super::shown(&own, &kids[..1]), "one layer's own colour is its Fill already");
        assert!(super::shown(&figcraft_engine::selection_colors::collect(doc, &kids), &kids), "two layers of one colour");
    }
}
