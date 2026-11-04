//! Prototype connections between variants, as in Figma: dragging the connection handle of a
//! variant (or a layer inside one) onto another variant of the same set connects them with
//! "Change to" (interactive components). Upstream's canvas only drops on top-level frames, and the
//! whole set is one of those.

use figcraft_doc::{Document, NodeId};
use figcraft_engine::components::{owning_component, variant_set};
use figcraft_geom::Point;

/// The other variant of `source`'s component set under `pt` (page coordinates), if any.
pub fn variant_at(doc: &Document, source: NodeId, pt: Point) -> Option<NodeId> {
    let own = owning_component(doc, source)?;
    let set = variant_set(doc, own)?;
    let variants = doc.node(set)?.children.clone();
    variants.into_iter().rev().filter(|v| *v != own).find(|v| doc.world_bounds(*v).is_some_and(|b| b.contains(pt)))
}

/// With something selected, the canvas only draws the connections of the selected screens: those
/// leaving them (from any layer inside) or arriving at them. Nothing selected: all of the page's.
pub fn shows_connection(doc: &Document, selection: &[NodeId], source: NodeId, dest: NodeId) -> bool {
    if selection.is_empty() {
        return true;
    }
    let screen = |id: NodeId| doc.top_level_frame(id).unwrap_or(id);
    let (from, to) = (screen(source), screen(dest));
    selection.iter().map(|s| screen(*s)).any(|s| s == from || s == to)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_variant_connects_to_its_siblings_only() {
        let mut s = figcraft_engine::Session::new();
        let id = |v: serde_json::Value| figcraft_engine::params::id_of(&v["id"]).unwrap();
        let frame = id(s.execute("frame.create", &json!({"width": 100, "height": 100})).unwrap());
        let inner = id(s.execute("shape.rectangle", &json!({"parent": frame, "width": 20, "height": 20})).unwrap());
        let main = id(s.execute("component.create", &json!({"ids": [frame]})).unwrap());
        s.execute("component.addVariant", &json!({"id": main})).unwrap();
        let doc = s.doc().unwrap();
        let set = variant_set(doc, main).unwrap();
        let other = *doc.node(set).unwrap().children.iter().find(|v| **v != main).unwrap();
        let centre = |n: NodeId| doc.world_bounds(n).unwrap().center();
        assert_eq!(variant_at(doc, main, centre(other)), Some(other), "variant to its sibling");
        assert_eq!(variant_at(doc, inner, centre(other)), Some(other), "a layer inside a variant too");
        assert_eq!(variant_at(doc, main, centre(main)), None, "not to itself");
        let outside = Point::new(centre(other).x + 10_000.0, centre(other).y);
        assert_eq!(variant_at(doc, main, outside), None);
    }

    #[test]
    fn only_the_selected_screens_connections_show() {
        let mut s = figcraft_engine::Session::new();
        let id = |v: serde_json::Value| figcraft_engine::params::id_of(&v["id"]).unwrap();
        let [a, b, c] = [0.0, 400.0, 800.0].map(|x| id(s.execute("frame.create", &json!({"x": x, "width": 300, "height": 300})).unwrap()));
        let button = id(s.execute("shape.rectangle", &json!({"parent": a, "width": 20, "height": 20})).unwrap());
        let doc = s.doc().unwrap();
        assert!(shows_connection(doc, &[], button, b), "nothing selected: every connection");
        assert!(shows_connection(doc, &[a], button, b), "leaving the selected screen, from a layer in it");
        assert!(shows_connection(doc, &[b], button, b), "arriving at the selected screen");
        assert!(!shows_connection(doc, &[c], button, b), "another screen's connection is hidden");
        assert!(shows_connection(doc, &[button], button, b), "a selected layer counts as its screen");
    }
}
