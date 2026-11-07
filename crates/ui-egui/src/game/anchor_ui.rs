//! Graybox: the Constraints control, laid out like Figma's: the horizontal and vertical choices
//! stacked on the left, and on the right a box with a bar on each side and a cross in the middle,
//! each one a click (Shift on a side bar stretches to both sides). Layers anchor by where they sit
//! until set by hand ("Auto" shows next to the title); "Auto (by position)" hands them back.

use egui::{CornerRadius, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use figcraft_doc::{Constraints, Document, HConstraint, NodeId, VConstraint};
use serde_json::json;

use crate::FigApp;
use crate::right::{menu_check, menu_popup, menu_row};
use crate::theme::Tokens;
use crate::widgets::{self, FIELD_H};

const H_CHOICES: [(&str, HConstraint, &str); 5] = [
    ("Left", HConstraint::Left, "left"),
    ("Right", HConstraint::Right, "right"),
    ("Left & right", HConstraint::LeftRight, "leftRight"),
    ("Center", HConstraint::Center, "center"),
    ("Scale", HConstraint::Scale, "scale"),
];
const V_CHOICES: [(&str, VConstraint, &str); 5] = [
    ("Top", VConstraint::Top, "top"),
    ("Bottom", VConstraint::Bottom, "bottom"),
    ("Top & bottom", VConstraint::TopBottom, "topBottom"),
    ("Center", VConstraint::Center, "center"),
    ("Scale", VConstraint::Scale, "scale"),
];

/// What a click on the box sets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Part {
    Left,
    Right,
    Top,
    Bottom,
    CenterH,
    CenterV,
}

/// The horizontal constraint a click on a side gives, from `now` (Shift: both sides, or back to
/// the other side when both were on).
fn h_after(now: Option<HConstraint>, left: bool, shift: bool) -> HConstraint {
    let (this, other) = if left { (HConstraint::Left, HConstraint::Right) } else { (HConstraint::Right, HConstraint::Left) };
    match (shift, now) {
        (true, Some(c)) if c == other => HConstraint::LeftRight,
        (true, Some(HConstraint::LeftRight)) => other,
        _ => this,
    }
}

fn v_after(now: Option<VConstraint>, top: bool, shift: bool) -> VConstraint {
    let (this, other) = if top { (VConstraint::Top, VConstraint::Bottom) } else { (VConstraint::Bottom, VConstraint::Top) };
    match (shift, now) {
        (true, Some(c)) if c == other => VConstraint::TopBottom,
        (true, Some(VConstraint::TopBottom)) => other,
        _ => this,
    }
}

fn h_wire(c: HConstraint) -> &'static str {
    H_CHOICES.iter().find(|(_, v, _)| *v == c).map_or("left", |(_, _, w)| w)
}

fn v_wire(c: VConstraint) -> &'static str {
    V_CHOICES.iter().find(|(_, v, _)| *v == c).map_or("top", |(_, _, w)| w)
}

/// The Constraints rows for the selected layers `sel` (all in frames without auto layout); `c` is
/// their common constraints (None: they differ).
pub(crate) fn row(app: &mut FigApp, ui: &mut Ui, doc: &Document, sel: &[NodeId], c: Option<Constraints>, col: f32) {
    let ids = sel.to_vec();
    let auto = !sel.is_empty() && sel.iter().all(|id| !doc.game.anchored.contains(id));
    widgets::sublabels(ui, "Constraints", auto.then_some("Auto"));
    crate::right::block(ui, FIELD_H * 2.0 + 8.0, 0.0, |ui| {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 8.0);
            let h_label = c.map_or("Mixed", |c| H_CHOICES.iter().find(|(_, v, _)| *v == c.horizontal).map_or("Left", |(l, _, _)| l));
            let resp = widgets::dropdown(ui, h_label, col, Some(crate::icons::Icon::PaddingH));
            menu_popup(&resp).show(|ui| {
                for (label, v, wire) in H_CHOICES {
                    if menu_check(ui, c.is_some_and(|c| c.horizontal == v), label, "") {
                        app.set_props(&ids, json!({"constraints": {"horizontal": wire}}));
                        ui.close();
                    }
                }
                auto_item(app, ui, &ids, auto);
            });
            let v_label = c.map_or("Mixed", |c| V_CHOICES.iter().find(|(_, v, _)| *v == c.vertical).map_or("Top", |(l, _, _)| l));
            let resp = widgets::dropdown(ui, v_label, col, Some(crate::icons::Icon::PaddingV));
            menu_popup(&resp).show(|ui| {
                for (label, v, wire) in V_CHOICES {
                    if menu_check(ui, c.is_some_and(|c| c.vertical == v), label, "") {
                        app.set_props(&ids, json!({"constraints": {"vertical": wire}}));
                        ui.close();
                    }
                }
                auto_item(app, ui, &ids, auto);
            });
        });
        let size = vec2(col.min(FIELD_H * 2.0 + 8.0 + 24.0), FIELD_H * 2.0 + 8.0);
        if let Some(part) = diagram(ui, size, c) {
            let shift = ui.input(|i| i.modifiers.shift);
            let props = match part {
                Part::Left | Part::Right => json!({"horizontal": h_wire(h_after(c.map(|c| c.horizontal), part == Part::Left, shift))}),
                Part::Top | Part::Bottom => json!({"vertical": v_wire(v_after(c.map(|c| c.vertical), part == Part::Top, shift))}),
                Part::CenterH => json!({"horizontal": "center"}),
                Part::CenterV => json!({"vertical": "center"}),
            };
            app.set_props(&ids, json!({"constraints": props}));
        }
    });
}

/// The menus' last row: hand the constraints back to the layers' position.
fn auto_item(app: &mut FigApp, ui: &mut Ui, ids: &[NodeId], auto: bool) {
    ui.separator();
    let resp = menu_row(ui, &format!("{}Auto (by position)", if auto { "✓ " } else { "    " }), "", 0.0, |_, _, _| {});
    if resp.clicked() {
        app.run("game.anchors.auto", json!({"ids": ids}));
        ui.close();
    }
}

/// The box: an outer square, an inner one, a bar from each side of the outer to the inner, and a
/// cross in the inner; what `c` sets is drawn in the accent colour. Returns the part clicked.
fn diagram(ui: &mut Ui, size: egui::Vec2, c: Option<Constraints>) -> Option<Part> {
    let t = Tokens::get(ui.ctx());
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let p = ui.painter().clone();
    p.rect_filled(rect, CornerRadius::same(5), t.field);
    let inner = Rect::from_center_size(rect.center(), vec2(rect.width() * 0.36, rect.height() * 0.36));
    p.rect_stroke(inner, CornerRadius::same(2), Stroke::new(1.0, t.text_secondary.gamma_multiply(0.6)), StrokeKind::Inside);
    let gap = 4.0;
    let mid = rect.center();
    let segs: [(Part, Pos2, Pos2); 6] = [
        (Part::Left, pos2(rect.min.x + gap, mid.y), pos2(inner.min.x - gap, mid.y)),
        (Part::Right, pos2(inner.max.x + gap, mid.y), pos2(rect.max.x - gap, mid.y)),
        (Part::Top, pos2(mid.x, rect.min.y + gap), pos2(mid.x, inner.min.y - gap)),
        (Part::Bottom, pos2(mid.x, inner.max.y + gap), pos2(mid.x, rect.max.y - gap)),
        (Part::CenterH, pos2(mid.x, inner.min.y + 3.0), pos2(mid.x, inner.max.y - 3.0)),
        (Part::CenterV, pos2(inner.min.x + 3.0, mid.y), pos2(inner.max.x - 3.0, mid.y)),
    ];
    let on = |part: Part| {
        c.is_some_and(|c| match part {
            Part::Left => matches!(c.horizontal, HConstraint::Left | HConstraint::LeftRight),
            Part::Right => matches!(c.horizontal, HConstraint::Right | HConstraint::LeftRight),
            Part::Top => matches!(c.vertical, VConstraint::Top | VConstraint::TopBottom),
            Part::Bottom => matches!(c.vertical, VConstraint::Bottom | VConstraint::TopBottom),
            Part::CenterH => c.horizontal == HConstraint::Center,
            Part::CenterV => c.vertical == VConstraint::Center,
        })
    };
    let mut clicked = None;
    for (part, a, b) in segs {
        // A generous hit area around each bar.
        let hit = Rect::from_two_pos(a, b).expand(5.0);
        let resp = ui.interact(hit, ui.scope_id().with(("graybox_anchor", part as u8)), Sense::click());
        let col = if on(part) {
            t.accent
        } else if resp.hovered() {
            t.text
        } else {
            t.text_secondary.gamma_multiply(0.7)
        };
        p.line_segment([a, b], Stroke::new(if on(part) { 2.0 } else { 1.5 }, col));
        if resp.clicked() {
            clicked = Some(part);
        }
    }
    clicked
}

/// One frame without auto layout selected: a button anchoring every layer in it by position (for
/// layers drawn before anchoring followed position, or set by hand since).
pub(crate) fn frame_button(app: &mut FigApp, ui: &mut Ui, doc: &Document, sel: &[NodeId]) {
    let [id] = sel else { return };
    if !doc.node(*id).is_some_and(|n| n.kind.is_frame_like() && n.layout.is_none() && !n.children.is_empty() && !n.kind.bounds_follow_children()) {
        return;
    }
    crate::right::row(ui, 32.0, |ui| {
        if widgets::button(ui, "Anchor layers by position", false, true)
            .on_hover_text("Each layer inside takes the anchoring of where it sits (thirds, like a 9-slice)")
            .clicked()
        {
            app.run("game.anchors.auto", json!({"ids": [id]}));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn side_clicks_set_one_side_and_shift_stretches() {
        assert_eq!(h_after(Some(HConstraint::Center), true, false), HConstraint::Left);
        assert_eq!(h_after(Some(HConstraint::Left), false, true), HConstraint::LeftRight, "Shift on the other side: both");
        assert_eq!(h_after(Some(HConstraint::LeftRight), true, true), HConstraint::Right, "Shift on a side of both: the other one");
        assert_eq!(v_after(Some(VConstraint::Top), false, true), VConstraint::TopBottom);
        assert_eq!(v_after(None, false, false), VConstraint::Bottom);
    }
}
