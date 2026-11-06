//! Graybox: a frame whose fill a layer inside it hides completely (a screenshot or a background
//! covering the whole frame) doesn't draw that fill. Drawn, it showed as a thin light line around
//! the frame wherever the edge fell between pixels: the fill's soft edge and the layer's soft edge
//! each let some of the other through.

use figcraft_color::{BlendMode, PaintKind};
use figcraft_doc::{Document, Node, NodeKind};

/// Whether some layer of frame `n` paints the frame's whole box opaque, over its fill.
pub(crate) fn fill_hidden(doc: &Document, n: &Node) -> bool {
    if !n.kind.is_frame_like() || n.children.is_empty() {
        return false;
    }
    let kids: Vec<&Node> = n.children.iter().filter_map(|c| doc.node(*c)).collect();
    // A mask could cut the covering layer: then the fill may show.
    if kids.iter().any(|k| k.is_mask) {
        return false;
    }
    kids.iter().any(|k| covers(doc, n, k))
}

/// Whether `k`, a layer of `frame`, paints all of the frame's box opaque.
fn covers(doc: &Document, frame: &Node, k: &Node) -> bool {
    if !k.visible || k.opacity < 1.0 || k.blend != BlendMode::Normal || k.effects.iter().any(|e| e.visible) {
        return false;
    }
    if !matches!(k.kind, NodeKind::Rectangle | NodeKind::Frame { .. }) || k.corners.uniform() != Some(0.0) {
        return false;
    }
    let [a, b, c, d, tx, ty] = k.transform.as_coeffs();
    if b.abs() > 1e-9 || c.abs() > 1e-9 || a <= 0.0 || d <= 0.0 {
        return false;
    }
    let eps = 1e-6;
    let boxed = tx <= eps && ty <= eps && tx + a * k.width >= frame.width - eps && ty + d * k.height >= frame.height - eps;
    boxed
        && k.fills.iter().any(|p| {
            p.visible
                && p.blend == BlendMode::Normal
                && p.opacity >= 1.0
                && match &p.kind {
                    PaintKind::Solid { color } => color.a >= 1.0,
                    PaintKind::Image(img) => crate::paint::fills_box_opaque(doc, img, k.width, k.height),
                    _ => false,
                }
        })
}

#[cfg(test)]
mod tests {
    use crate::paint_behaviour_tests::{page, rect};
    use crate::{View, render_nodes_into};
    use figcraft_color::{Color, Paint};
    use figcraft_doc::NodeKind;

    /// A white frame covered by a dark layer, its bottom edge between pixels, on a black view: the
    /// edge row is no lighter than half the dark layer (no white comes through), and a layer that
    /// leaves part of the frame bare still lets the fill show there.
    #[test]
    fn a_covered_fill_leaves_no_light_seam() {
        let (mut d, p) = page();
        let f = d.create(p, NodeKind::Frame, "F", 10.0, 10.0, 40.0, 30.5).unwrap();
        {
            let n = d.get_mut(f).unwrap();
            n.fills = vec![Paint::solid(Color::WHITE)];
            n.clip = true;
        }
        let k = rect(&mut d, f, (0.0, 0.0, 40.0, 30.5), vec![Paint::solid(Color::from_hex("#202830").unwrap())]);
        let mut pix = tiny_skia::Pixmap::new(100, 100).unwrap();
        pix.fill(tiny_skia::Color::BLACK);
        render_nodes_into(&d, &[f], &View { origin: (0.0, 0.0), scale: 1.0, width: 100, height: 100 }, &mut pix);
        let edge = pix.pixel(25, 40).unwrap();
        assert!(edge.red() <= 0x20 / 2 + 8, "a light seam: {edge:?}");
        assert!(super::fill_hidden(&d, d.node(f).unwrap()));
        d.get_mut(k).unwrap().width = 20.0;
        assert!(!super::fill_hidden(&d, d.node(f).unwrap()), "half covered: the fill shows");
    }
}
