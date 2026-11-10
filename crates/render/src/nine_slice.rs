//! Graybox: a 9-sliced layer ([`figcraft_doc::nine_slice`]) drawn at another size than its base: its
//! content is drawn at the base size, then cut in nine; the corners go on at their own size, the
//! edges stretch along their side and the middle both ways. At its base size it draws as usual.

use figcraft_doc::nine_slice::{NineSlice, effective};
use figcraft_doc::{Node, NodeId};
use figcraft_geom::Affine;
use tiny_skia::{FilterQuality, Paint, Pattern, Pixmap, SpreadMode, Transform};

use crate::draw::{Bounds, Ctx};

/// The largest side of the base drawing (device pixels).
const MAX_SIDE: f64 = 4096.0;

/// Draw `n` (at `xf`, its own transform included) 9-sliced, if it is and its size differs from its
/// base. False when it isn't: the caller draws it as usual.
pub(crate) fn draw(ctx: &Ctx, pix: &mut Pixmap, n: &Node, xf: Affine, depth: usize) -> bool {
    let Some((slice, source)) = effective(ctx.doc, n.id) else { return false };
    let (bw, bh) = slice.base;
    if (n.width - bw).abs() < 1e-6 && (n.height - bh).abs() < 1e-6 {
        return false;
    }
    let Some(base) = base_layer(ctx, n, source, slice) else { return false };
    let [a, b, c, d, ..] = xf.as_coeffs();
    // Device pixels per layer unit along each axis.
    let (sx, sy) = ((a * a + b * b).sqrt().clamp(0.01, 64.0), (c * c + d * d).sqrt().clamp(0.01, 64.0));
    let k = (MAX_SIDE / (bw * sx).max(bh * sy)).min(1.0);
    let (sx, sy) = (sx * k, sy * k);
    let (pw, ph) = ((bw * sx).ceil().max(1.0) as u32, (bh * sy).ceil().max(1.0) as u32);
    let Some(_held) = crate::layer_budget::Held::take(pw, ph, 1) else { return false };
    let Some(mut off) = Pixmap::new(pw, ph) else { return false };
    {
        // The content at its base size, from a copy of the document where the layer has it.
        let mut doc = ctx.doc.clone();
        if let Some(m) = doc.node_mut(base.id) {
            *m = base.clone();
        }
        let bounds = Bounds::default();
        let sub = Ctx { doc: &doc, bounds: &bounds, under: None, skip: ctx.skip, outlined: ctx.outlined, ..*ctx };
        if let Some(node) = doc.node(base.id) {
            crate::draw::draw_layer(&sub, &mut off, node, Affine::scale_non_uniform(sx, sy), depth + 1);
        }
    }
    let [t, r, btm, l] = slice.insets;
    let (w, h) = (n.width.max(0.0), n.height.max(0.0));
    // Cuts that fit the drawn size too (a layer smaller than its corners shows them shrunk).
    let fit = |lo: f64, hi: f64, size: f64| {
        let sum = lo + hi;
        if sum > size && sum > 0.0 { (lo * size / sum, hi * size / sum) } else { (lo, hi) }
    };
    let ((dl, dr), (dt, db)) = (fit(l, r, w), fit(t, btm, h));
    let cols = [(0.0, l, 0.0, dl), (l, bw - r, dl, w - dr), (bw - r, bw, w - dr, w)];
    let rows = [(0.0, t, 0.0, dt), (t, bh - btm, dt, h - db), (bh - btm, bh, h - db, h)];
    let to_device = crate::draw::ts_transform(xf);
    for (sx0, sx1, dx0, dx1) in cols {
        for (sy0, sy1, dy0, dy1) in rows {
            let (sw, sh, dw, dh) = (sx1 - sx0, sy1 - sy0, dx1 - dx0, dy1 - dy0);
            if sw <= 1e-6 || sh <= 1e-6 || dw <= 1e-6 || dh <= 1e-6 {
                continue;
            }
            let Some(rect) = tiny_skia::Rect::from_xywh(dx0 as f32, dy0 as f32, dw as f32, dh as f32) else { continue };
            // Base drawing pixels → this cell in the layer's units.
            let (kx, ky) = (dw / sw, dh / sh);
            let cell = Transform::from_row((kx / sx) as f32, 0.0, 0.0, (ky / sy) as f32, (dx0 - sx0 * kx) as f32, (dy0 - sy0 * ky) as f32);
            let paint = Paint {
                shader: Pattern::new(off.as_ref(), SpreadMode::Pad, FilterQuality::Bilinear, 1.0, cell),
                // Cells meet edge to edge: smoothed edges would let a seam through between them.
                anti_alias: false,
                ..Paint::default()
            };
            pix.fill_rect(rect, &paint, to_device, None);
        }
    }
    true
}

/// The layer drawn at its base size: `n` itself (its layers kept where they were drawn), or for an
/// instance the main component's content at the instance's place.
fn base_layer(ctx: &Ctx, n: &Node, source: NodeId, slice: NineSlice) -> Option<Node> {
    let from = if source == n.id { n.clone() } else { ctx.doc.node(source)?.clone() };
    let mut base = from;
    base.width = slice.base.0;
    base.height = slice.base.1;
    base.transform = Affine::IDENTITY;
    // Drawn plainly: no 9-slice again, as the layer it stands in for.
    base.plugin_data.remove(figcraft_doc::nine_slice::KEY);
    Some(base)
}

#[cfg(test)]
mod tests {
    use crate::paint_behaviour_tests::{page, rect};
    use crate::{View, render_nodes_into};
    use figcraft_color::{Color, Paint};
    use figcraft_doc::nine_slice::{KEY, NineSlice};

    /// A white panel with a 10-unit red border, 9-sliced at 10: drawn three times as wide, its
    /// border stays 10 wide (corners and edges unscaled) and its middle stays white.
    #[test]
    fn a_nine_sliced_panel_keeps_its_border() {
        let (mut d, p) = page();
        let panel = d.create(p, figcraft_doc::NodeKind::Frame, "Panel", 0.0, 0.0, 40.0, 40.0).unwrap();
        rect(&mut d, panel, (10.0, 10.0, 20.0, 20.0), vec![Paint::solid(Color::WHITE)]);
        {
            let n = d.get_mut(panel).unwrap();
            n.fills = vec![Paint::solid(Color::from_hex("#ff0000").unwrap())];
            n.plugin_data.insert(KEY.into(), NineSlice { insets: [10.0; 4], base: (40.0, 40.0) }.encode());
            n.width = 120.0;
        }
        let mut pix = tiny_skia::Pixmap::new(140, 60).unwrap();
        pix.fill(tiny_skia::Color::BLACK);
        render_nodes_into(&d, &[panel], &View { origin: (0.0, 0.0), scale: 1.0, width: 140, height: 60 }, &mut pix);
        let px = |x: u32, y: u32| {
            let c = pix.pixel(x, y).unwrap();
            (c.red(), c.green(), c.blue())
        };
        assert_eq!(px(5, 20), (255, 0, 0), "left border");
        assert_eq!(px(114, 20), (255, 0, 0), "right border stays 10 wide at the new right edge");
        assert_eq!(px(60, 20), (255, 255, 255), "the middle stretches white");
        assert_eq!(px(60, 5), (255, 0, 0), "top edge");
        assert_eq!(px(125, 20), (0, 0, 0), "nothing past the layer");
    }
}
