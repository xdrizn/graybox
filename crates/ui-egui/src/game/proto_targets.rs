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
}
