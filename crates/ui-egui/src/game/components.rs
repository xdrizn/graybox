//! Graybox's component properties in the Design panel, closer to how designers expect them:
//!
//! - A component set (or a lone main component): **Properties** lists each property on one compact
//!   row (`◇ State · Default, Focused, Pressed`). A click opens its editor (name, values, default,
//!   delete). One **Add variant** button, nothing else.
//! - A variant inside a set: **Current variant**, one dropdown per variant property showing this
//!   variant's own value. Picking another value moves this variant to it. The property
//!   definitions are not editable here; select the set for that.

use egui::{Id, PopupCloseBehavior, Sense, Ui, vec2};
use figcraft_doc::{Document, NodeId, NodeKind, PropDef};
use figcraft_engine::cmd::component::can_add_variant;
use figcraft_engine::components::{default_variant, prop_list, variant_conflicts, variant_name, variant_options, variant_props, variant_set};
use serde_json::{Value, json};

use crate::FigApp;
use crate::icons::{self, Icon};
use crate::right::{menu_item, menu_popup, row};
use crate::theme::{COMPONENT, Tokens, regular, semibold};
use crate::widgets;

thread_local! {
    /// On in the app. Upstream's own panel tests run with it off, so upstream's matrix (still the
    /// fallback) keeps its coverage; Graybox's tests switch it on with [`set_compact`].
    static COMPACT: std::cell::Cell<bool> = const { std::cell::Cell::new(!cfg!(test)) };
}

/// Use the compact panel on this thread (tests).
#[cfg(test)]
pub fn set_compact(on: bool) {
    COMPACT.with(|c| c.set(on));
}

/// The Design panel section for a selected component set or main component. Returns true when it
/// drew the section (the caller then skips upstream's property matrix).
pub fn section(app: &mut FigApp, ui: &mut Ui, doc: &Document, id: NodeId) -> bool {
    if !COMPACT.with(std::cell::Cell::get) {
        return false;
    }
    let Some(n) = doc.node(id) else { return false };
    match &n.kind {
        NodeKind::ComponentSet { .. } => properties(app, ui, doc, id),
        NodeKind::Component { .. } => match variant_set(doc, id) {
            Some(set) => current_variant(app, ui, doc, set, id),
            None => properties(app, ui, doc, id),
        },
        _ => return false,
    }
    true
}

fn header(ui: &mut Ui, title: &str, plus_tip: Option<&str>) -> Option<egui::Response> {
    let t = Tokens::get(ui.ctx());
    let mut plus = None;
    row(ui, 36.0, |ui| {
        ui.label(egui::RichText::new(title).font(semibold(11.0)).color(t.text));
        if let Some(tip) = plus_tip {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                plus = Some(icons::button(ui, Icon::Plus, 24.0, false, tip));
            });
        }
    });
    plus
}

/// Definitions of a set's (or lone component's) properties, variant ones first.
fn properties(app: &mut FigApp, ui: &mut Ui, doc: &Document, owner: NodeId) {
    let t = Tokens::get(ui.ctx());
    let is_set = matches!(doc.node(owner).map(|n| &n.kind), Some(NodeKind::ComponentSet { .. }));
    let first = if is_set {
        doc.node(owner).and_then(|s| s.children.iter().copied().find(|c| matches!(doc.node(*c).map(|n| &n.kind), Some(NodeKind::Component { .. }))))
    } else {
        Some(owner)
    };
    let defs: Vec<(String, PropDef)> =
        first.map(|c| prop_list(doc, c)).unwrap_or_default().into_iter().filter(|(_, d)| !matches!(d, PropDef::Variant { .. })).collect();
    let variants = if is_set { variant_options(doc, owner) } else { Vec::new() };
    widgets::separator(ui);
    if let Some(plus) = header(ui, "Properties", Some("Create component property")) {
        add_menu(app, &plus, owner, &defs, &variants);
    }
    if defs.is_empty() && variants.is_empty() {
        widgets::note(ui, "Add properties to make instances configurable", t.text_tertiary);
    }
    if is_set && !variant_conflicts(doc, owner).is_empty() {
        widgets::note(ui, "Some variants have the same property values: give each one a unique combination", t.warning);
    }
    let defaults = if is_set {
        default_variant(doc, owner).and_then(|d| doc.node(d)).map(|n| variant_props(&n.name)).unwrap_or_default()
    } else {
        Default::default()
    };
    for (k, vals) in &variants {
        let summary = vals.join(", ");
        let resp = compact_row(ui, Icon::ComponentSet, k, &summary);
        editor_popup(&resp, |ui| variant_editor(app, ui, owner, k, vals, defaults.get(k).or(vals.first())));
    }
    for (k, d) in &defs {
        let summary = match d {
            PropDef::Boolean { default } => (if *default { "True" } else { "False" }).to_string(),
            PropDef::Text { default } => default.clone(),
            PropDef::InstanceSwap { default, .. } => doc.node(*default).map(|n| n.name.clone()).unwrap_or_else(|| "Missing".into()),
            PropDef::Slot { .. } => "Slot".into(),
            PropDef::Variant { .. } => String::new(),
        };
        let resp = compact_row(ui, type_icon(d), k, &summary);
        editor_popup(&resp, |ui| shared_editor(app, ui, owner, k, d));
    }
    if can_add_variant(doc, owner) {
        row(ui, 32.0, |ui| {
            if widgets::dropdown(ui, "Add variant", 120.0, Some(Icon::Plus)).clicked() {
                app.run("component.addVariant", json!({"id": owner}));
            }
        });
    }
}

/// The add-property menu under the header's +.
fn add_menu(app: &mut FigApp, plus: &egui::Response, owner: NodeId, defs: &[(String, PropDef)], variants: &[(String, Vec<String>)]) {
    let existing: Vec<&str> = defs.iter().map(|(k, _)| k.as_str()).chain(variants.iter().map(|(k, _)| k.as_str())).collect();
    let fresh = (1..).map(|k| format!("Property {k}")).find(|n| !existing.contains(&n.as_str())).unwrap_or_else(|| "Property".into());
    menu_popup(plus).width(180.0).show(|ui| {
        let items: [(&str, &str, Value); 3] =
            [("Variant", "variant", json!("Default")), ("Boolean", "boolean", json!(true)), ("Text", "text", json!("Text"))];
        for (label, ty, default) in items {
            if menu_item(ui, label, "") {
                app.run("component.addProperty", json!({"id": owner, "name": fresh, "type": ty, "default": default}));
                ui.close();
            }
        }
    });
}

/// `◇ Name · summary` on one line; clicking it opens the property's editor.
fn compact_row(ui: &mut Ui, icon: Icon, name: &str, summary: &str) -> egui::Response {
    let t = Tokens::get(ui.ctx());
    let w = ui.available_width() - widgets::PAD - widgets::PAD_RIGHT;
    let mut out = None;
    row(ui, 32.0, |ui| {
        let (r, resp) = ui.allocate_exact_size(vec2(w.max(40.0), widgets::FIELD_H), Sense::click());
        let resp = resp.on_hover_text("Edit property");
        let fill = if resp.hovered() { t.hover } else { t.field };
        ui.painter().rect_filled(r, 4, fill);
        let ir = egui::Rect::from_center_size(egui::pos2(r.min.x + 14.0, r.center().y), vec2(14.0, 14.0));
        icons::paint(ui.painter(), ir, icon, COMPONENT);
        let name_galley = ui.painter().layout_no_wrap(name.to_string(), regular(11.0), t.text);
        let name_w = name_galley.size().x;
        ui.painter().galley(egui::pos2(r.min.x + 28.0, r.center().y - name_galley.size().y / 2.0), name_galley, t.text);
        let room = (r.max.x - 8.0) - (r.min.x + 28.0 + name_w + 4.0);
        if room > 16.0 && !summary.is_empty() {
            let shown = widgets::elide_font(ui, &format!("· {summary}"), room, regular(11.0));
            ui.painter().text(egui::pos2(r.min.x + 32.0 + name_w, r.center().y), egui::Align2::LEFT_CENTER, shown, regular(11.0), t.text_secondary);
        }
        crate::a11y::text(&resp, name, summary);
        out = Some(resp);
    });
    out.unwrap_or_else(|| ui.allocate_response(vec2(0.0, 0.0), Sense::hover()))
}

/// A property editor that stays open while its fields are edited; a click outside closes it.
fn editor_popup(anchor: &egui::Response, body: impl FnOnce(&mut Ui)) {
    egui::Popup::from_toggle_button_response(anchor)
        .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
        .width(anchor.rect.width().max(220.0))
        .show(|ui| {
            ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
            body(ui);
        });
}

fn variant_editor(app: &mut FigApp, ui: &mut Ui, owner: NodeId, k: &str, vals: &[String], default: Option<&String>) {
    let t = Tokens::get(ui.ctx());
    widgets::label(ui, "Name", t.text_secondary);
    if let Some(nn) = widgets::text_field(ui, Id::unique(("gb_vprop_name", owner, k)), k, ui.available_width(), "Name") {
        app.run("component.editProperty", json!({"id": owner, "name": k, "newName": nn}));
    }
    widgets::label(ui, "Values", t.text_secondary);
    for v in vals {
        if let Some(nv) = widgets::text_field(ui, Id::unique(("gb_vprop_val", owner, k, v.as_str())), v, ui.available_width(), "")
            && nv != *v
        {
            app.run("component.editProperty", json!({"id": owner, "name": k, "value": v, "newValue": nv}));
        }
    }
    if widgets::button(ui, "Add value", false, true).on_hover_text("Adds a variant with a new value").clicked() {
        let nv = (vals.len() + 1..).map(|n| format!("Value {n}")).find(|n| !vals.contains(n)).unwrap_or_default();
        app.run("component.addVariant", json!({"id": owner, "property": k, "value": nv}));
    }
    widgets::label(ui, "Default", t.text_secondary);
    let def = default.cloned().unwrap_or_default();
    let resp = widgets::dropdown(ui, &def, ui.available_width(), None).on_hover_text("Default value (the top-left variant)");
    menu_popup(&resp).show(|ui| {
        for v in vals {
            if menu_item(ui, v, "") {
                app.run("component.editProperty", json!({"id": owner, "name": k, "default": v}));
                ui.close();
            }
        }
    });
    ui.add_space(4.0);
    if widgets::button(ui, "Delete property", false, true).clicked() {
        app.run("component.deleteProperty", json!({"id": owner, "name": k}));
    }
}

fn shared_editor(app: &mut FigApp, ui: &mut Ui, owner: NodeId, k: &str, d: &PropDef) {
    let t = Tokens::get(ui.ctx());
    widgets::label(ui, "Name", t.text_secondary);
    if let Some(nn) = widgets::text_field(ui, Id::unique(("gb_prop_name", owner, k)), k, ui.available_width(), "Name") {
        app.run("component.editProperty", json!({"id": owner, "name": k, "newName": nn}));
    }
    match d {
        PropDef::Boolean { default } => {
            if widgets::checkbox(ui, *default, "Default: true") {
                app.run("component.editProperty", json!({"id": owner, "name": k, "default": !default}));
            }
        }
        PropDef::Text { default } => {
            widgets::label(ui, "Default", t.text_secondary);
            if let Some(v) = widgets::text_field(ui, Id::unique(("gb_prop_def", owner, k)), default, ui.available_width(), "Default") {
                app.run("component.editProperty", json!({"id": owner, "name": k, "default": v}));
            }
        }
        _ => widgets::label(ui, "Change its default from an instance or the layer it controls.", t.text_secondary),
    }
    ui.add_space(4.0);
    if widgets::button(ui, "Delete property", false, true).clicked() {
        app.run("component.deleteProperty", json!({"id": owner, "name": k}));
    }
}

/// A variant: its own value for each variant property. Picking another value renames this
/// variant (its properties live in its name), so it moves to that value.
fn current_variant(app: &mut FigApp, ui: &mut Ui, doc: &Document, set: NodeId, variant: NodeId) {
    let t = Tokens::get(ui.ctx());
    let options = variant_options(doc, set);
    let Some(name) = doc.node(variant).map(|n| n.name.clone()) else { return };
    let mine = variant_props(&name);
    widgets::separator(ui);
    header(ui, "Current variant", None);
    for (k, vals) in &options {
        let current = mine.get(k).cloned().unwrap_or_default();
        row(ui, 32.0, |ui| {
            let label_w = 96.0;
            let (lr, _) = ui.allocate_exact_size(vec2(label_w, widgets::FIELD_H), Sense::hover());
            let shown = widgets::elide_font(ui, k, label_w - 4.0, regular(11.0));
            ui.painter().text(lr.left_center(), egui::Align2::LEFT_CENTER, shown, regular(11.0), t.text_secondary);
            let w = (ui.available_width() - widgets::PAD_RIGHT).max(60.0);
            let resp = widgets::dropdown(ui, &current, w, None);
            crate::a11y::combo(&resp, k, &current, crate::a11y::menu_open(&resp));
            menu_popup(&resp).show(|ui| {
                for v in vals {
                    if menu_item(ui, v, "") && *v != current {
                        let mut pairs = mine.clone();
                        pairs.insert(k.clone(), v.clone());
                        let order: Vec<String> = options.iter().map(|(k, _)| k.clone()).collect();
                        app.run("node.set", json!({"id": variant, "props": {"name": variant_name(&pairs, &order)}}));
                        ui.close();
                    }
                }
            });
        });
    }
    if !variant_conflicts(doc, set).is_empty() {
        widgets::note(ui, "Another variant already has these values", t.warning);
    }
    widgets::note(ui, "Select the component set to edit its properties.", t.text_tertiary);
}

fn type_icon(d: &PropDef) -> Icon {
    match d {
        PropDef::Boolean { .. } => Icon::Eye,
        PropDef::Text { .. } => Icon::Text,
        PropDef::InstanceSwap { .. } => Icon::Instance,
        PropDef::Slot { .. } => Icon::Slot,
        PropDef::Variant { .. } => Icon::ComponentSet,
    }
}
