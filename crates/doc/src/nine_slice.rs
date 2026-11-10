//! Graybox: 9-slice scaling. A layer (an image, a shape, a frame or a component) remembers the size
//! it was drawn at (`base`) and four cuts in from its sides (`insets`: top, right, bottom, left).
//! Drawn at another size, its corners keep their size, its edges stretch along one axis and its
//! middle along both, as a 9-sliced sprite does in a game engine. An instance takes its main
//! component's 9-slice. Stored in the layer's plugin data, so copies and duplicates keep it.

use crate::{Document, Node, NodeId, NodeKind};

/// The plugin data key the 9-slice is stored under.
pub const KEY: &str = "graybox.nineSlice";

/// The four cuts and the size they were set for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NineSlice {
    /// Top, right, bottom, left, in the layer's units at `base`.
    pub insets: [f64; 4],
    /// The layer's width and height when its 9-slice was set.
    pub base: (f64, f64),
}

impl NineSlice {
    /// A 9-slice for a `w` × `h` layer, cut a quarter of its shorter side in from each side.
    pub fn new(w: f64, h: f64) -> Option<Self> {
        let s = Self { insets: [w.min(h) / 4.0; 4], base: (w, h) };
        s.valid().then_some(s)
    }

    /// The stored form: `t,r,b,l,width,height`.
    pub fn encode(&self) -> String {
        let [t, r, b, l] = self.insets;
        format!("{t},{r},{b},{l},{},{}", self.base.0, self.base.1)
    }

    pub fn decode(s: &str) -> Option<Self> {
        let v: Vec<f64> = s.split(',').map(|p| p.trim().parse::<f64>()).collect::<Result<_, _>>().ok()?;
        let [t, r, b, l, w, h] = v.as_slice() else { return None };
        let s = Self { insets: [*t, *r, *b, *l], base: (*w, *h) }.clamped();
        s.valid().then_some(s)
    }

    /// Finite, a base of a real size, cuts that fit in it.
    pub fn valid(&self) -> bool {
        let (w, h) = self.base;
        self.insets.iter().chain([&w, &h]).all(|v| v.is_finite()) && w > 0.0 && h > 0.0
    }

    /// The cuts kept inside the base: none negative, left + right within the width, top + bottom
    /// within the height.
    pub fn clamped(self) -> Self {
        let (w, h) = self.base;
        let c = |v: f64, max: f64| if v.is_finite() { v.clamp(0.0, max.max(0.0)) } else { 0.0 };
        let [t, r, b, l] = self.insets;
        let (l, t) = (c(l, w), c(t, h));
        let (r, b) = (c(r, w - l), c(b, h - t));
        Self { insets: [t, r, b, l], base: self.base }
    }

    /// The 9-slice stored on `n`, if any.
    pub fn of(n: &Node) -> Option<Self> {
        n.plugin_data.get(KEY).and_then(|s| Self::decode(s))
    }
}

/// The 9-slice that applies to layer `id` and the layer whose content it slices: its own, or for
/// an instance without one, its main component's (sliced from the main's own layers).
pub fn effective(doc: &Document, id: NodeId) -> Option<(NineSlice, NodeId)> {
    let n = doc.node(id)?;
    if let Some(s) = NineSlice::of(n) {
        return Some((s, id));
    }
    match n.kind {
        NodeKind::Instance { component, .. } => doc.node(component).and_then(NineSlice::of).map(|s| (s, component)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_nine_slice_round_trips_and_keeps_its_cuts_inside() {
        let s = NineSlice { insets: [10.0, 12.0, 8.0, 6.0], base: (100.0, 40.0) };
        assert_eq!(NineSlice::decode(&s.encode()), Some(s));
        let wide = NineSlice { insets: [30.0, 80.0, 30.0, 60.0], base: (100.0, 40.0) }.clamped();
        assert_eq!(wide.insets, [30.0, 40.0, 10.0, 60.0], "left + right and top + bottom fit");
        assert_eq!(NineSlice::decode("1,2,3"), None);
        assert_eq!(NineSlice::decode("1,1,1,1,0,10"), None, "no width");
        assert_eq!(NineSlice::new(80.0, 40.0).map(|s| s.insets), Some([10.0; 4]));
    }
}
