//! Graybox (G9): working inside a top-level frame, as artboards do in Figma.
//!
//! A frame with layers in it that is being worked in (it is selected, a layer in it is selected,
//! or the view is zoomed onto it) is like the canvas for the layers in it: a press on one of them
//! selects it, a drag on the frame's empty background draws a marquee, and the frame shows no hover
//! outline over its whole area. The frame itself moves by its name label.

use figcraft_doc::{Document, NodeId, NodeKind};
use figcraft_geom::Point;

use crate::Session;

/// The share of the view a frame must cover, along either axis, for the view to count as zoomed
/// onto it (zooming to fit fills one axis only, a wide frame in a tall view fills the width).
const FILLS_VIEW: f64 = 0.7;

/// Whether `id` is a top-level frame with layers in it, one that is worked inside like an artboard.
/// An empty frame, a widget, a board object such as a sticky, and a component or instance (small
/// things that are dragged by their body) are not.
fn artboard(doc: &Document, page: NodeId, id: NodeId) -> bool {
    doc.node(id).is_some_and(|n| {
        n.parent == Some(page) && matches!(n.kind, NodeKind::Frame) && !n.children.is_empty() && !crate::widgets::is_widget(n) && !n.is_board_atom()
    })
}

/// Whether the selection is just such a frame: a press anywhere on it that reaches no layer of its
/// own marquees instead of dragging it.
pub(super) fn only_artboard_selected(doc: &Document, page: NodeId, sel: &[NodeId]) -> bool {
    matches!(sel, [f] if artboard(doc, page, *f))
}

impl Session {
    /// Whether the view is zoomed onto `id`: it covers most of the viewport along an axis.
    fn fills_view(&self, doc: &Document, id: NodeId) -> bool {
        let Ok(ds) = self.ds() else { return false };
        let v = &ds.view;
        let Some(b) = doc.world_bounds(id) else { return false };
        let (w, h) = ((b.x1 - b.x0) * v.zoom, (b.y1 - b.y0) * v.zoom);
        v.vw > 0.0 && v.vh > 0.0 && w.is_finite() && h.is_finite() && (w >= v.vw * FILLS_VIEW || h >= v.vh * FILLS_VIEW)
    }

    /// `hover` (what a hover at `p` found, `leaf` being the layer painted there) without the
    /// outline of a frame that is being worked in, over its background. Its name label and the
    /// layers in it still light up.
    pub(super) fn quiet_frame_hover(
        &self,
        doc: &Document,
        page: NodeId,
        sel: &[NodeId],
        hover: Option<NodeId>,
        leaf: Option<NodeId>,
        p: Point,
    ) -> Option<NodeId> {
        let h = hover?;
        let worked_in = || sel.iter().any(|s| doc.is_ancestor(h, *s)) || self.fills_view(doc, h);
        let on_background = leaf == Some(h) && super::handles::label_at(doc, page, &self.tool.labels, p).is_none();
        (!(on_background && artboard(doc, page, h) && worked_in())).then_some(h)
    }
}
