//! Hotbars in Play: an auto layout frame marked as a hotbar has one selected slot. Keys 1 to 9 pick
//! a slot, the wheel and LB / RB step through them, and the player keeps moving meanwhile. The
//! selected slot shows the variant picked for it (or one named hover, selected…), else an outline.

use std::collections::BTreeMap;

use figcraft_doc::game::HotbarSettings;
use figcraft_doc::{Document, NodeId, NodeKind};

/// One frame of hotbar input.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HotbarInput {
    /// A number key pressed: slot index (key 1 → 0).
    pub slot: Option<usize>,
    /// Steps from the wheel and LB / RB (+1 next, -1 previous).
    pub step: i32,
}

/// The hotbars shown on `roots` (the presented screen and its open overlays).
pub fn in_view(doc: &Document, roots: &[NodeId]) -> Vec<(NodeId, HotbarSettings)> {
    doc.game
        .hotbars
        .iter()
        .filter(|(id, _)| roots.iter().any(|r| *r == **id || doc.is_ancestor(*r, **id)))
        .filter(|(id, _)| doc.node(**id).is_some_and(|n| n.visible))
        .map(|(id, s)| (*id, s.clone()))
        .collect()
}

/// Move `selected` (a slot index) by one frame of `input`, among `count` slots.
pub fn step(selected: usize, count: usize, settings: &HotbarSettings, input: &HotbarInput) -> usize {
    if count == 0 {
        return 0;
    }
    let mut at = selected.min(count - 1);
    if let Some(slot) = input.slot.filter(|s| *s < count) {
        at = slot;
    }
    let n = count as i64;
    let next = at as i64 + i64::from(input.step.clamp(-9, 9));
    at = if settings.wrap { next.rem_euclid(n) as usize } else { next.clamp(0, n - 1) as usize };
    at
}

/// The variant each slot of `bar` should show (the selected one its selected look, the others the
/// variant they have in the file, `original`), and the selected slot when it has no such variant
/// (the player outlines it).
pub fn looks(original: &Document, bar: NodeId, selected: usize, settings: &HotbarSettings) -> (Vec<(NodeId, NodeId)>, Option<NodeId>) {
    let slots = super::menu::items(original, bar);
    let mut outline = None;
    let looks = slots
        .iter()
        .enumerate()
        .filter_map(|(i, slot)| {
            let component = match original.node(*slot)?.kind {
                NodeKind::Instance { component, .. } => Some(component),
                _ => None,
            };
            let shown = if i == selected {
                component.and_then(|c| super::looks::shown(original, c, &settings.selected_look, super::looks::Role::Selected))
            } else {
                component
            };
            if i == selected && (shown.is_none() || shown == component) {
                outline = Some(*slot);
            }
            Some((*slot, shown.or(component)?))
        })
        .collect();
    (looks, outline)
}

/// How a slot looks, without what it holds or where it sits: size, scale and rotation, paints,
/// effects, corners and opacity.
#[derive(Clone, Debug, PartialEq)]
pub struct Look {
    pub width: f64,
    pub height: f64,
    /// Scale, rotation and skew (the transform without its translation).
    pub linear: [f64; 4],
    pub fills: Vec<figcraft_doc::color::Paint>,
    pub strokes: Vec<figcraft_doc::color::Paint>,
    pub stroke: figcraft_doc::StrokeProps,
    pub effects: Vec<figcraft_doc::Effect>,
    pub corners: figcraft_doc::Corners,
    pub opacity: f32,
}

impl Look {
    /// The look of node `id` in `doc`.
    pub fn of(doc: &Document, id: NodeId) -> Option<Look> {
        let n = doc.node(id)?;
        let c = n.transform.as_coeffs();
        Some(Look {
            width: n.width,
            height: n.height,
            linear: [c[0], c[1], c[2], c[3]],
            fills: n.fills.clone(),
            strokes: n.strokes.clone(),
            stroke: n.stroke.clone(),
            effects: n.effects.clone(),
            corners: n.corners,
            opacity: n.opacity,
        })
    }

    /// Give node `id` of `doc` this look, keeping its position. Whether anything changed.
    pub fn apply(&self, doc: &mut Document, id: NodeId) -> bool {
        if Look::of(doc, id).as_ref() == Some(self) {
            return false;
        }
        let Some(n) = doc.node_mut(id) else { return false };
        let finite = self.width.is_finite() && self.height.is_finite() && self.linear.iter().all(|v| v.is_finite());
        if finite {
            let c = n.transform.as_coeffs();
            let [a, b, cc, d] = self.linear;
            n.transform = figcraft_geom::Affine::new([a, b, cc, d, c[4], c[5]]);
            n.width = self.width.max(0.0);
            n.height = self.height.max(0.0);
        }
        n.fills = self.fills.clone();
        n.strokes = self.strokes.clone();
        n.stroke = self.stroke.clone();
        n.effects = self.effects.clone();
        n.corners = self.corners;
        n.opacity = self.opacity;
        true
    }
}

/// Wireframe hotbars: when no slot of `bar` is a component copy and the first slot is drawn
/// differently from the second, the first slot's look is the selected look and the second's the
/// plain one. Returns the look each slot should have with `selected` picked (the selected slot the
/// first's, the first — when not selected — the second's, the rest their own), or None when the bar
/// isn't one of these (components use their variants, identical slots the outline).
pub fn base_looks(original: &Document, bar: NodeId, selected: usize) -> Option<Vec<(NodeId, Look)>> {
    let slots = super::menu::items(original, bar);
    if slots.len() < 2 || slots.iter().any(|s| original.node(*s).is_none_or(|n| matches!(n.kind, NodeKind::Instance { .. }))) {
        return None;
    }
    let active = Look::of(original, *slots.first()?)?;
    let plain = Look::of(original, *slots.get(1)?)?;
    if active == plain {
        return None;
    }
    let selected = selected.min(slots.len() - 1);
    slots
        .iter()
        .enumerate()
        .map(|(i, slot)| {
            let look = if i == selected {
                active.clone()
            } else if i == 0 {
                plain.clone()
            } else {
                Look::of(original, *slot)?
            };
            Some((*slot, look))
        })
        .collect()
}

/// Number keys pressed this frame as a slot index (1 → 0 … 9 → 8).
pub fn digit(i: &egui::InputState) -> Option<usize> {
    const KEYS: [egui::Key; 9] = [
        egui::Key::Num1,
        egui::Key::Num2,
        egui::Key::Num3,
        egui::Key::Num4,
        egui::Key::Num5,
        egui::Key::Num6,
        egui::Key::Num7,
        egui::Key::Num8,
        egui::Key::Num9,
    ];
    KEYS.iter().position(|k| i.key_pressed(*k))
}

/// The selection kept per hotbar while playing.
pub type Selection = BTreeMap<NodeId, usize>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_pick_and_steps_cycle() {
        let wrap = HotbarSettings { wrap: true, ..Default::default() };
        let stop = HotbarSettings { wrap: false, ..Default::default() };
        assert_eq!(step(0, 5, &wrap, &HotbarInput { slot: Some(3), step: 0 }), 3, "key 4 picks the fourth slot");
        assert_eq!(step(2, 5, &wrap, &HotbarInput { slot: Some(7), step: 0 }), 2, "a key past the last slot does nothing");
        assert_eq!(step(4, 5, &wrap, &HotbarInput { slot: None, step: 1 }), 0, "next from the last wraps");
        assert_eq!(step(0, 5, &wrap, &HotbarInput { slot: None, step: -1 }), 4, "previous from the first wraps");
        assert_eq!(step(4, 5, &stop, &HotbarInput { slot: None, step: 1 }), 4, "without wrap it stops");
        assert_eq!(step(9, 3, &wrap, &HotbarInput::default()), 2, "a stale selection stays in range");
        assert_eq!(step(0, 0, &wrap, &HotbarInput { slot: Some(0), step: 1 }), 0, "an empty hotbar");
    }

    /// A row of five plain frames, each holding a label; the first drawn bigger, red and rounded.
    fn wireframe() -> (Document, NodeId, Vec<NodeId>, Vec<NodeId>) {
        let mut doc = Document::new("Hotbar");
        let page = doc.first_page().unwrap();
        let bar = doc.create(page, NodeKind::Frame, "Hotbar", 0.0, 0.0, 600.0, 100.0).unwrap();
        let (mut slots, mut labels) = (Vec::new(), Vec::new());
        for i in 0..5 {
            let slot = doc.create(bar, NodeKind::Frame, &format!("Slot {}", i + 1), i as f64 * 70.0, 0.0, 60.0, 60.0).unwrap();
            labels.push(doc.create(slot, NodeKind::Rectangle, "Label", 10.0, 10.0, 10.0 + i as f64, 10.0).unwrap());
            slots.push(slot);
        }
        let first = doc.node_mut(slots[0]).unwrap();
        first.width = 80.0;
        first.height = 90.0;
        first.transform = figcraft_geom::Affine::new([1.2, 0.0, 0.0, 1.2, 0.0, 0.0]);
        first.fills = vec![figcraft_doc::color::Paint::solid(figcraft_doc::color::Color::new(1.0, 0.0, 0.0, 1.0))];
        first.opacity = 0.8;
        first.corners = figcraft_doc::Corners { radii: [12.0; 4], ..Default::default() };
        doc.game.hotbars.insert(bar, HotbarSettings::default());
        (doc, bar, slots, labels)
    }

    #[test]
    fn first_slot_look_moves_to_the_picked_one() {
        let (original, bar, slots, labels) = wireframe();
        let active = Look::of(&original, slots[0]).unwrap();
        let plain = Look::of(&original, slots[1]).unwrap();
        let picked = base_looks(&original, bar, 3).unwrap();
        let look = |id: NodeId| picked.iter().find(|(s, _)| *s == id).map(|(_, l)| l.clone()).unwrap();
        assert_eq!(look(slots[3]), active, "key 4: the fourth slot looks like the first did");
        assert_eq!(look(slots[0]), plain, "and the first looks like the others");
        assert_eq!(look(slots[2]), plain, "the rest keep their own look");

        let mut shown = original.clone();
        for (slot, l) in &picked {
            l.apply(&mut shown, *slot);
        }
        let n = shown.node(slots[3]).unwrap();
        assert_eq!((n.width, n.height, n.opacity), (80.0, 90.0, 0.8), "size and opacity move");
        assert_eq!(n.transform.as_coeffs()[0], 1.2, "scale moves");
        assert_eq!(n.transform.as_coeffs()[4], original.node(slots[3]).unwrap().transform.as_coeffs()[4], "the position stays");
        assert_eq!(n.children, vec![labels[3]], "what the slot holds stays");
        assert_eq!(shown.node(labels[3]), original.node(labels[3]), "and is untouched");

        let first = base_looks(&original, bar, 0).unwrap();
        assert_eq!(first.iter().find(|(s, _)| *s == slots[0]).map(|(_, l)| l), Some(&active), "the first picked looks as drawn");
    }

    #[test]
    fn components_and_identical_slots_keep_the_old_ways() {
        let (mut doc, bar, slots, _) = wireframe();
        let plain = Look::of(&doc, slots[1]).unwrap();
        let mut same = doc.clone();
        plain.apply(&mut same, slots[0]);
        assert!(base_looks(&same, bar, 2).is_none(), "first slot drawn like the others: the outline stays");

        let component = doc
            .create(
                doc.first_page().unwrap(),
                NodeKind::Component { description: String::new(), props: Default::default(), prop_order: Vec::new() },
                "Slot",
                0.0,
                300.0,
                60.0,
                60.0,
            )
            .unwrap();
        doc.node_mut(slots[2]).unwrap().kind = NodeKind::Instance { component, props: Default::default() };
        assert!(base_looks(&doc, bar, 2).is_none(), "a component copy in the row: variants decide");

        let (one, bar_one, _, _) = {
            let mut d = Document::new("One");
            let page = d.first_page().unwrap();
            let b = d.create(page, NodeKind::Frame, "Bar", 0.0, 0.0, 100.0, 100.0).unwrap();
            let s = d.create(b, NodeKind::Frame, "Slot", 0.0, 0.0, 60.0, 60.0).unwrap();
            (d, b, s, ())
        };
        assert!(base_looks(&one, bar_one, 0).is_none(), "a single slot has no plain look");
        assert!(base_looks(&one, NodeId(u64::MAX), 0).is_none(), "a missing bar");
    }

    #[test]
    fn junk_looks_never_break_a_slot() {
        let (mut doc, _, slots, _) = wireframe();
        let mut junk = Look::of(&doc, slots[0]).unwrap();
        junk.width = f64::NAN;
        junk.linear = [f64::INFINITY, 0.0, 0.0, 1.0];
        junk.apply(&mut doc, slots[1]);
        let n = doc.node(slots[1]).unwrap();
        assert!(n.width.is_finite() && n.transform.as_coeffs().iter().all(|v| v.is_finite()));
    }
}
