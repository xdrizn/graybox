//! How a game screen fills the monitor in Play. The frame as designed is the reference size: it is
//! scaled until its width or height meets the monitor's (whichever comes first, so nothing is cut)
//! and then stretched along the other side to the monitor's shape. Its layers' constraints decide
//! where the extra room goes (Right + Bottom keeps a layer in that corner, Center in the middle,
//! Left & Right stretches it), the way game engines anchor UI to a reference resolution.

/// The frame's size in Play and the scale it is drawn at, for a frame designed at `design` on a
/// monitor area of `area` (both width × height). None when either size is empty or not finite.
pub fn fit(design: (f64, f64), area: (f64, f64)) -> Option<((f64, f64), f64)> {
    let ok = |v: f64| v.is_finite() && v >= 1.0;
    if !(ok(design.0) && ok(design.1) && ok(area.0) && ok(area.1)) {
        return None;
    }
    let scale = (area.0 / design.0).min(area.1 / design.1).clamp(0.01, 16.0);
    let size = ((area.0 / scale).clamp(1.0, 100_000.0), (area.1 / scale).clamp(1.0, 100_000.0));
    Some((size, scale))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: (f64, f64), b: (f64, f64)) -> bool {
        (a.0 - b.0).abs() < 1e-6 && (a.1 - b.1).abs() < 1e-6
    }

    #[test]
    fn same_shape_scales_only() {
        let (size, scale) = fit((1920.0, 1080.0), (1920.0, 1080.0)).unwrap();
        assert!(near(size, (1920.0, 1080.0)) && scale == 1.0, "a 16:9 monitor at the design size: as designed");
        let (size, scale) = fit((1920.0, 1080.0), (3840.0, 2160.0)).unwrap();
        assert!(near(size, (1920.0, 1080.0)) && scale == 2.0, "4K: twice as big, same layout");
        let (size, scale) = fit((1280.0, 720.0), (1920.0, 1080.0)).unwrap();
        assert!(near(size, (1280.0, 720.0)) && scale == 1.5);
    }

    #[test]
    fn other_shapes_stretch_the_frame() {
        let (size, scale) = fit((1920.0, 1080.0), (3440.0, 1440.0)).unwrap();
        assert!(near(size, (2580.0, 1080.0)) && (scale - 1440.0 / 1080.0).abs() < 1e-9, "ultrawide: wider, same height");
        let (size, _) = fit((1920.0, 1080.0), (1920.0, 1200.0)).unwrap();
        assert!(near(size, (1920.0, 1200.0)), "16:10: taller, same width");
        let (size, scale) = fit((1920.0, 1080.0), (1024.0, 768.0)).unwrap();
        assert!(near((size.0 * scale, size.1 * scale), (1024.0, 768.0)), "4:3: fills the monitor exactly");
        assert!(size.0 >= 1920.0 - 1e-6 && size.1 >= 1080.0 - 1e-6, "never smaller than designed: nothing is cut");
    }

    #[test]
    fn junk_sizes() {
        assert!(fit((0.0, 1080.0), (1920.0, 1080.0)).is_none());
        assert!(fit((1920.0, 1080.0), (f64::NAN, 1080.0)).is_none());
        assert!(fit((1920.0, 1080.0), (f64::INFINITY, 1080.0)).is_none());
        let (size, scale) = fit((1.0, 1.0), (100_000.0, 10.0)).unwrap();
        assert!(size.0.is_finite() && size.1.is_finite() && scale.is_finite());
    }
}
