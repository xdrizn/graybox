//! The "When selected / focused / pressed / disabled" pickers shared by hotbars and navigable
//! menus: the designer picks one of the items' variants (no special names to know), lets Auto find
//! one by its name, or asks for an orange outline. When there is no variant to pick, one button
//! creates it. Warnings say why items can't change look.

use egui::Ui;
use figcraft_doc::game::StateLook;
use figcraft_doc::{Document, NodeId, NodeKind};
use figcraft_engine::components::{variant_name, variant_options, variant_props, variant_set};
use serde_json::{Value, json};

use super::looks::{self, Role};
use crate::FigApp;
use crate::right::{menu_check, menu_popup, row};
use crate::theme::Tokens;
use crate::widgets;

/// Why items (a hotbar's slots, a menu's items) can't change look: the original component among
/// them, or no component copies at all.
pub fn warnings(ui: &mut Ui, doc: &Document, items: &[NodeId], what: &str) {
    let t = Tokens::get(ui.ctx());
    for (i, item) in items.iter().enumerate() {
        if let Some(n) = doc.node(*item)
            && matches!(n.kind, NodeKind::Component { .. })
        {
            let text = format!("{what} {} (\"{}\") is the original component, so it can't change. Move it out and use copies.", i + 1, n.name);
            widgets::note(ui, &text, t.warning);
        }
    }
    if !items.is_empty() && looks::first_component(doc, items).is_none() {
        widgets::note(ui, "None of them is a copy of a component with variants: the chosen one gets an orange outline.", t.text_secondary);
    }
}

fn label(look: &StateLook, guess: Option<&(String, String)>) -> String {
    match look {
        StateLook::Auto => match guess {
            Some((k, v)) => format!("Auto: {k} = {v}"),
            None => "Auto: orange outline".into(),
        },
        StateLook::Variant { property, value } => format!("{property} = {value}"),
        StateLook::Outline => "Orange outline".into(),
    }
}

/// `title: [look ▾]` for items showing `component`. A pick runs `command` with `params(look)`.
#[allow(clippy::too_many_arguments)]
pub fn picker(
    app: &mut FigApp,
    ui: &mut Ui,
    doc: &Document,
    title: &str,
    current: &StateLook,
    component: Option<NodeId>,
    role: Role,
    command: &str,
    params: &dyn Fn(Value) -> Value,
) {
    let t = Tokens::get(ui.ctx());
    let guess = component.and_then(|c| looks::guess(doc, c, role));
    let choices = component.map(|c| looks::choices(doc, c)).unwrap_or_default();
    row(ui, 32.0, |ui| {
        let (lr, _) = ui.allocate_exact_size(egui::vec2(92.0, widgets::FIELD_H), egui::Sense::hover());
        ui.painter().text(lr.left_center(), egui::Align2::LEFT_CENTER, title, crate::theme::regular(11.0), t.text_secondary);
        let response = widgets::dropdown(ui, &label(current, guess.as_ref()), ui.available_width() - widgets::PAD_RIGHT, None);
        menu_popup(&response).show(|ui| {
            let mut options = vec![StateLook::Auto];
            options.extend(choices.iter().map(|(k, v)| StateLook::Variant { property: k.clone(), value: v.clone() }));
            options.push(StateLook::Outline);
            for look in options {
                if menu_check(ui, *current == look, &label(&look, guess.as_ref()), "") {
                    if let Ok(v) = serde_json::to_value(&look) {
                        app.run(command, params(v));
                    }
                    ui.close();
                }
            }
        });
    });
    let Some(component) = component else { return };
    if looks::shown(doc, component, current, role).is_none() && !matches!(current, StateLook::Outline) {
        row(ui, 32.0, |ui| {
            if widgets::button(ui, "Create a variant for it", false, true)
                .on_hover_text("Adds a variant to the component: draw this state in it")
                .clicked()
                && let Some((property, value)) = create_variant(app, component, role)
            {
                app.run(command, params(json!({"kind": "variant", "property": property, "value": value})));
            }
        });
    }
}

/// Add a variant for `role` to `component`'s set (a lone component becomes a set first). Returns
/// the property and value that select it.
fn create_variant(app: &mut FigApp, component: NodeId, role: Role) -> Option<(String, String)> {
    let value = match role {
        Role::Selected => "Selected",
        Role::Pressed => "Pressed",
        Role::Disabled => "Disabled",
    };
    let set_of = |app: &FigApp| app.session.doc().ok().and_then(|d| variant_set(d, component));
    if set_of(app).is_none() {
        app.run("component.addVariant", json!({"id": component}))?;
        let doc = app.session.doc().ok()?;
        let set = variant_set(doc, component)?;
        let other = doc.node(set)?.children.iter().copied().find(|c| *c != component)?;
        let mut props = variant_props(&doc.node(other)?.name);
        let property = props.keys().next()?.clone();
        props.insert(property.clone(), value.into());
        let order: Vec<String> = props.keys().cloned().collect();
        app.run("node.set", json!({"id": other, "props": {"name": variant_name(&props, &order)}}))?;
        return Some((property, value.into()));
    }
    let set = set_of(app)?;
    let property = app.session.doc().ok().and_then(|d| variant_options(d, set).first().map(|(k, _)| k.clone()))?;
    app.run("component.addVariant", json!({"id": set, "property": property, "value": value}))?;
    Some((property, value.into()))
}
