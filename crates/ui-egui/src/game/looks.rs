//! How menu items and hotbar slots show a state (focused, selected, pressed, disabled) without the
//! designer having to know special names: the look is a variant the designer picked, or one found
//! by its name (hover, selected, pressed…), else an outline.

use figcraft_doc::game::StateLook;
use figcraft_doc::{Document, NodeId, NodeKind};
use figcraft_engine::components::{find_variant, variant_options, variant_props};

/// The state a look shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// Focused in a menu, selected in a hotbar.
    Selected,
    Pressed,
    Disabled,
}

impl Role {
    /// Variant values that mean this state, in order of preference (compared without case).
    fn words(self) -> &'static [&'static str] {
        match self {
            Role::Selected => &["focused", "focus", "selected", "select", "hover", "hovered", "active", "highlighted", "highlight", "on"],
            Role::Pressed => &["pressed", "press", "down", "clicked", "click"],
            Role::Disabled => &["disabled", "locked", "inactive", "unavailable", "off"],
        }
    }
}

/// The component set `component` belongs to.
fn set_of(doc: &Document, component: NodeId) -> Option<NodeId> {
    let set = doc.node(component)?.parent?;
    matches!(doc.node(set)?.kind, NodeKind::ComponentSet { .. }).then_some(set)
}

/// Every (property, value) of `component`'s set, for a picker.
pub fn choices(doc: &Document, component: NodeId) -> Vec<(String, String)> {
    let Some(set) = set_of(doc, component) else { return Vec::new() };
    variant_options(doc, set).into_iter().flat_map(|(k, vals)| vals.into_iter().map(move |v| (k.clone(), v))).collect()
}

/// The (property, value) whose name means `role`, if the set has one.
pub fn guess(doc: &Document, component: NodeId, role: Role) -> Option<(String, String)> {
    let options = choices(doc, component);
    role.words().iter().find_map(|w| options.iter().find(|(_, v)| v.trim().eq_ignore_ascii_case(w)).cloned())
}

/// The (property, value) `look` means for `component`, or None for an outline.
pub fn resolve(doc: &Document, component: NodeId, look: &StateLook, role: Role) -> Option<(String, String)> {
    match look {
        StateLook::Variant { property, value } => Some((property.clone(), value.clone())),
        StateLook::Outline => None,
        StateLook::Auto => guess(doc, component, role),
    }
}

/// The variant of `component`'s set with the same properties but `property` = `value`.
pub fn variant_with(doc: &Document, component: NodeId, property: &str, value: &str) -> Option<NodeId> {
    let set = set_of(doc, component)?;
    let mut wanted = variant_props(&doc.node(component)?.name);
    wanted.get(property)?;
    wanted.insert(property.into(), value.into());
    let found = find_variant(doc, set, &wanted)?;
    (variant_props(&doc.node(found)?.name).get(property).map(String::as_str) == Some(value)).then_some(found)
}

/// The variant an item whose own variant is `component` shows in `role`, or None (an outline).
pub fn shown(doc: &Document, component: NodeId, look: &StateLook, role: Role) -> Option<NodeId> {
    let (property, value) = resolve(doc, component, look, role)?;
    variant_with(doc, component, &property, &value)
}

/// True when `component` (an item's own variant) is in `role`'s state (a disabled item).
pub fn is(doc: &Document, component: NodeId, look: &StateLook, role: Role) -> bool {
    let Some((property, value)) = resolve(doc, component, look, role) else { return false };
    doc.node(component).is_some_and(|c| variant_props(&c.name).get(&property) == Some(&value))
}

/// The component the first instance among `items` shows: the one a picker offers values of.
pub fn first_component(doc: &Document, items: &[NodeId]) -> Option<NodeId> {
    items.iter().find_map(|i| match doc.node(*i).map(|n| &n.kind) {
        Some(NodeKind::Instance { component, .. }) => Some(*component),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A set "Slot" with Property 1 = defaultt / hover.
    fn slot_set() -> (figcraft_engine::Session, NodeId, NodeId) {
        let mut s = figcraft_engine::Session::new();
        let id = |v: serde_json::Value| figcraft_engine::params::id_of(&v["id"]).unwrap();
        let f = id(s.execute("frame.create", &json!({"width": 60, "height": 60})).unwrap());
        let main = id(s.execute("component.create", &json!({"ids": [f]})).unwrap());
        s.execute("node.set", &json!({"id": main, "props": {"name": "Property 1=defaultt"}})).unwrap();
        s.execute("component.addVariant", &json!({"id": main})).unwrap();
        let set = set_of(s.doc().unwrap(), main).unwrap();
        let other = *s.doc().unwrap().node(set).unwrap().children.iter().find(|v| **v != main).unwrap();
        s.execute("node.set", &json!({"id": other, "props": {"name": "Property 1=hover"}})).unwrap();
        (s, main, other)
    }

    #[test]
    fn any_names_work_auto_finds_hover_and_a_pick_wins() {
        let (s, main, hover) = slot_set();
        let doc = s.doc().unwrap();
        assert_eq!(guess(doc, main, Role::Selected), Some(("Property 1".into(), "hover".into())), "hover reads as selected");
        assert_eq!(shown(doc, main, &StateLook::Auto, Role::Selected), Some(hover));
        assert_eq!(shown(doc, main, &StateLook::Auto, Role::Pressed), None, "no pressed variant: an outline");
        let pick = StateLook::Variant { property: "Property 1".into(), value: "defaultt".into() };
        assert_eq!(shown(doc, hover, &pick, Role::Selected), Some(main), "the designer's pick wins");
        assert_eq!(shown(doc, main, &StateLook::Outline, Role::Selected), None);
        assert!(is(doc, hover, &StateLook::Variant { property: "Property 1".into(), value: "hover".into() }, Role::Disabled));
        assert_eq!(choices(doc, main).len(), 2);
    }
}
