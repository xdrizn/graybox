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
}
