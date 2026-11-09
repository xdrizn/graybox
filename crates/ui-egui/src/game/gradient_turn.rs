//! Graybox: "Rotate 90°" in the colour picker turns a linear gradient a quarter clockwise about
//! the middle of the layer, so it stays centred in it: top → bottom becomes right → left, then
//! bottom → top. (Upstream turned it about its start, which took it out of the layer.) Radial,
//! angular and diamond gradients turn about their own centre, their start.

use figcraft_color::{Gradient, GradientKind};

/// Turn `g` a quarter clockwise (on screen, y down), as above.
pub(crate) fn quarter(g: &mut Gradient) {
    let (cx, cy) = if g.kind == GradientKind::Linear { (0.5, 0.5) } else { g.from };
    let rot = |(x, y): (f64, f64)| (cx - (y - cy), cy + (x - cx));
    g.from = rot(g.from);
    g.to = rot(g.to);
    g.width = rot(g.width);
}

#[cfg(test)]
mod tests {
    use super::*;
    use figcraft_color::Color;

    fn near(a: (f64, f64), b: (f64, f64)) -> bool {
        (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9
    }

    #[test]
    fn a_linear_gradient_turns_about_the_layers_middle() {
        let mut g = Gradient::default_for(GradientKind::Linear, Color::BLACK);
        assert!(near(g.from, (0.5, 0.0)) && near(g.to, (0.5, 1.0)), "top to bottom");
        quarter(&mut g);
        assert!(near(g.from, (1.0, 0.5)) && near(g.to, (0.0, 0.5)), "right to left: {:?} → {:?}", g.from, g.to);
        quarter(&mut g);
        assert!(near(g.from, (0.5, 1.0)) && near(g.to, (0.5, 0.0)), "bottom to top");
        let mut r = Gradient::default_for(GradientKind::Radial, Color::BLACK);
        let centre = r.from;
        quarter(&mut r);
        assert!(near(r.from, centre), "a radial gradient keeps its centre");
    }
}
