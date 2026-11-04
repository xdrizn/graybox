//! Maps in Play: a frame marked as a map (`GameSettings::maps`) shows its content moved and zoomed
//! by the player. The frame keeps clipping as designed; only its children move. The view is Play
//! state, never written to the file.

use std::collections::BTreeMap;

use figcraft_doc::game::{MapSettings, MapZoomInput};
use figcraft_doc::{Document, NodeId};
use figcraft_geom::Affine;

/// Pan speed of a full stick push, in page units per second at zoom 1.
const STICK_PAN: f64 = 700.0;
/// Zoom rate of a full stick push or a held shoulder button (factor e^rate per second).
const STICK_ZOOM: f64 = 1.6;
/// Zoom per wheel point.
const WHEEL_ZOOM: f64 = 0.0015;
/// Sticks below this are at rest.
const DEAD_ZONE: f32 = 0.18;

/// How far a map's content is moved (page units, before zoom) and zoomed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub offset: [f64; 2],
    pub zoom: f64,
}

impl Default for View {
    fn default() -> Self {
        Self { offset: [0.0, 0.0], zoom: 1.0 }
    }
}

/// One frame of input for the active map.
#[derive(Clone, Copy, Debug, Default)]
pub struct MapInput {
    /// Pointer drag this frame, in page units.
    pub drag: [f64; 2],
    /// Wheel this frame, in points (up is positive).
    pub wheel: f64,
    /// Left stick: pans.
    pub stick: [f32; 2],
    /// Right stick vertical (up is negative), for [`MapZoomInput::RightStick`].
    pub zoom_stick: f32,
    /// RT / R2 and RB / R1 held, for [`MapZoomInput::Shoulders`].
    pub zoom_in: bool,
    pub zoom_out: bool,
    /// Seconds since the last frame.
    pub dt: f64,
}

/// The map in the screen `root` (the screen on top), if it has one.
pub fn active(doc: &Document, root: NodeId) -> Option<(NodeId, MapSettings)> {
    doc.game.maps.iter().find(|(id, _)| **id == root || doc.is_ancestor(root, **id)).map(|(id, m)| (*id, *m))
}

fn live(v: f32) -> f64 {
    if v.is_finite() && v.abs() > DEAD_ZONE { f64::from(v) } else { 0.0 }
}

/// Move and zoom `view` by one frame of `input`.
pub fn step(view: &mut View, settings: &MapSettings, input: &MapInput) {
    let dt = if input.dt.is_finite() { input.dt.clamp(0.0, 0.1) } else { 0.0 };
    let zoom = view.zoom.max(0.01);
    let mut offset = view.offset;
    for (k, d) in input.drag.iter().enumerate() {
        if d.is_finite() {
            offset[k] += d / zoom;
        }
    }
    // A stick moves the view: the content goes the other way.
    offset[0] -= live(input.stick[0]) * STICK_PAN * dt / zoom;
    offset[1] -= live(input.stick[1]) * STICK_PAN * dt / zoom;
    let mut rate = 0.0;
    match settings.zoom_input {
        MapZoomInput::RightStick => rate -= live(input.zoom_stick) * STICK_ZOOM,
        MapZoomInput::Shoulders => {
            rate += if input.zoom_in { STICK_ZOOM } else { 0.0 };
            rate -= if input.zoom_out { STICK_ZOOM } else { 0.0 };
        }
    }
    let wheel = if input.wheel.is_finite() { input.wheel } else { 0.0 };
    let mut next = zoom * (rate * dt).exp() * (1.0 + wheel * WHEEL_ZOOM).max(0.1);
    if !next.is_finite() {
        next = zoom;
    }
    view.zoom = next.clamp(settings.zoom_min, settings.zoom_max);
    if offset.iter().all(|v| v.is_finite()) {
        view.offset = offset;
    }
}

/// Show each map's content as its view has it: moved, then zoomed about the frame's centre.
pub fn apply(doc: &mut Document, views: &BTreeMap<NodeId, View>) {
    for (map, view) in views {
        if *view == View::default() {
            continue;
        }
        let Some(frame) = doc.node(*map) else { continue };
        let (cx, cy) = (frame.width / 2.0, frame.height / 2.0);
        let around = Affine::translate((cx, cy))
            * Affine::scale(view.zoom)
            * Affine::translate((-cx, -cy))
            * Affine::translate((view.offset[0], view.offset[1]));
        for child in frame.children.clone() {
            if let Some(n) = doc.node_mut(child) {
                n.transform = around * n.transform;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mouse_and_sticks_pan_and_zoom_within_the_limits() {
        let settings = MapSettings::default();
        let mut view = View::default();
        step(&mut view, &settings, &MapInput { drag: [30.0, -10.0], ..Default::default() });
        assert_eq!(view.offset, [30.0, -10.0], "the content follows the pointer");
        step(&mut view, &settings, &MapInput { stick: [1.0, 0.0], dt: 0.1, ..Default::default() });
        assert!(view.offset[0] < 30.0, "stick right looks right: the content moves left");
        step(&mut view, &settings, &MapInput { zoom_stick: -1.0, dt: 0.1, ..Default::default() });
        assert!(view.zoom > 1.0, "right stick up zooms in");
        step(&mut view, &settings, &MapInput { stick: [0.1, 0.1], zoom_stick: 0.1, dt: 0.1, ..Default::default() });
        let resting = view;
        step(&mut view, &settings, &MapInput { stick: [0.1, 0.1], zoom_stick: 0.1, dt: 0.1, ..Default::default() });
        assert_eq!(view, resting, "a stick at rest does nothing");
        for _ in 0..200 {
            step(&mut view, &settings, &MapInput { wheel: 120.0, ..Default::default() });
        }
        assert_eq!(view.zoom, settings.zoom_max);
        let shoulders = MapSettings { zoom_input: MapZoomInput::Shoulders, ..settings };
        for _ in 0..200 {
            step(&mut view, &shoulders, &MapInput { zoom_out: true, dt: 0.1, ..Default::default() });
        }
        assert_eq!(view.zoom, settings.zoom_min);
        step(&mut view, &settings, &MapInput { drag: [f64::NAN, 0.0], wheel: f64::INFINITY, dt: f64::NAN, ..Default::default() });
        assert!(view.offset.iter().all(|v| v.is_finite()) && view.zoom.is_finite(), "junk input changes nothing");
    }

    #[test]
    fn only_the_maps_content_moves() {
        let mut doc = Document::new("Map");
        let page = doc.first_page().unwrap();
        let screen = doc.create(page, figcraft_doc::NodeKind::Frame, "Map screen", 0.0, 0.0, 1280.0, 720.0).unwrap();
        let map = doc.create(screen, figcraft_doc::NodeKind::Frame, "Map", 100.0, 100.0, 400.0, 200.0).unwrap();
        let image = doc.create(map, figcraft_doc::NodeKind::Rectangle, "Image", 0.0, 0.0, 1600.0, 800.0).unwrap();
        doc.game.maps.insert(map, MapSettings::default());
        assert_eq!(active(&doc, screen).map(|(id, _)| id), Some(map));
        let map_before = doc.node(map).unwrap().transform;
        let views = BTreeMap::from([(map, View { offset: [-50.0, 0.0], zoom: 2.0 })]);
        apply(&mut doc, &views);
        assert_eq!(doc.node(map).unwrap().transform, map_before, "the frame stays");
        let moved = doc.node(image).unwrap().transform;
        // Moved 50 left, then doubled about the frame's centre (200, 100).
        assert_eq!(moved * figcraft_geom::Point::new(0.0, 0.0), figcraft_geom::Point::new(-300.0, -100.0));
    }
}
