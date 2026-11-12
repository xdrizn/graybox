//! A cel's pixels: straight (not premultiplied) RGBA, row by row from the top left.

/// One art pixel: red, green, blue, alpha (straight alpha).
pub type Rgba = [u8; 4];

/// The transparent pixel.
pub const CLEAR: Rgba = [0, 0, 0, 0];

/// A picture of `width` × `height` art pixels.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Canvas {
    width: u32,
    height: u32,
    px: Vec<Rgba>,
}

impl Canvas {
    /// The longest side a sprite may have (art pixels).
    pub const MAX_SIDE: u32 = 1024;

    /// A transparent canvas, or None for a size of zero or over [`Self::MAX_SIDE`].
    pub fn new(width: u32, height: u32) -> Option<Self> {
        Self::filled(width, height, CLEAR)
    }

    /// A canvas of one colour, or None for a size of zero or over [`Self::MAX_SIDE`].
    pub fn filled(width: u32, height: u32, c: Rgba) -> Option<Self> {
        if width == 0 || height == 0 || width > Self::MAX_SIDE || height > Self::MAX_SIDE {
            return None;
        }
        Some(Self { width, height, px: vec![c; width as usize * height as usize] })
    }

    /// A canvas from its pixels (`width * height` of them), or None when they don't match.
    pub fn from_pixels(width: u32, height: u32, px: Vec<Rgba>) -> Option<Self> {
        let ok = width > 0 && height > 0 && width <= Self::MAX_SIDE && height <= Self::MAX_SIDE;
        (ok && px.len() == width as usize * height as usize).then_some(Self { width, height, px })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    /// Every pixel, row by row.
    pub fn pixels(&self) -> &[Rgba] {
        &self.px
    }

    fn index(&self, x: i64, y: i64) -> Option<usize> {
        (x >= 0 && y >= 0 && x < i64::from(self.width) && y < i64::from(self.height)).then(|| y as usize * self.width as usize + x as usize)
    }

    /// The pixel at `x`, `y`, or None outside.
    pub fn get(&self, x: i64, y: i64) -> Option<Rgba> {
        self.index(x, y).and_then(|i| self.px.get(i).copied())
    }

    /// Sets the pixel at `x`, `y`; false outside.
    pub fn set(&mut self, x: i64, y: i64, c: Rgba) -> bool {
        match self.index(x, y).and_then(|i| self.px.get_mut(i)) {
            Some(p) => {
                *p = c;
                true
            }
            None => false,
        }
    }

    /// Whether every pixel is transparent.
    pub fn is_clear(&self) -> bool {
        self.px.iter().all(|p| p[3] == 0)
    }

    /// `top` drawn over this canvas at `opacity` (0–255), pixel for pixel (same size; a smaller
    /// or larger `top` is drawn from the top left).
    pub fn draw_over(&mut self, top: &Canvas, opacity: u8) {
        if opacity == 0 {
            return;
        }
        for y in 0..self.height.min(top.height) {
            for x in 0..self.width.min(top.width) {
                let (x, y) = (i64::from(x), i64::from(y));
                if let (Some(under), Some(over)) = (self.get(x, y), top.get(x, y)) {
                    self.set(x, y, over_pixel(under, over, opacity));
                }
            }
        }
    }

    /// The RGBA bytes, row by row.
    pub fn to_bytes(&self) -> Vec<u8> {
        self.px.iter().flatten().copied().collect()
    }

    /// A canvas from RGBA bytes, or None when they don't match the size.
    pub fn from_bytes(width: u32, height: u32, bytes: &[u8]) -> Option<Self> {
        if bytes.len() != width as usize * height as usize * 4 {
            return None;
        }
        let px = bytes.chunks_exact(4).map(|c| [c[0], c[1], c[2], c[3]]).collect();
        Self::from_pixels(width, height, px)
    }

    /// This canvas as a PNG (RGBA, 8 bits).
    pub fn to_png(&self) -> Option<Vec<u8>> {
        encode_png(self.width, self.height, &self.to_bytes())
    }

    /// A canvas from a PNG (any colour type at 8 bits, palettes and greys expanded), or None for a
    /// broken or too large one.
    pub fn from_png(bytes: &[u8]) -> Option<Self> {
        let (w, h, px) = decode_png(bytes, Self::MAX_SIDE)?;
        Self::from_bytes(w, h, &px)
    }

    /// A copy `width` × `height` with this picture at `dx`, `dy` (what falls outside is cut).
    pub fn resized(&self, width: u32, height: u32, dx: i64, dy: i64) -> Option<Self> {
        let mut out = Self::new(width, height)?;
        for y in 0..i64::from(self.height) {
            for x in 0..i64::from(self.width) {
                if let Some(c) = self.get(x, y) {
                    out.set(x + dx, y + dy, c);
                }
            }
        }
        Some(out)
    }
}

/// RGBA `px` (`width` × `height`, 8 bits) as a PNG, or None when they don't match.
pub fn encode_png(width: u32, height: u32, px: &[u8]) -> Option<Vec<u8>> {
    if width == 0 || height == 0 || px.len() as u64 != u64::from(width) * u64::from(height) * 4 {
        return None;
    }
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, width, height);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.set_compression(png::Compression::Fast);
        let mut w = enc.write_header().ok()?;
        w.write_image_data(px).ok()?;
        w.finish().ok()?;
    }
    Some(out)
}

/// A PNG's size and RGBA pixels (any colour type at 8 bits, palettes and greys expanded), or None
/// for a broken one or one more than `max_side` on a side.
pub fn decode_png(bytes: &[u8], max_side: u32) -> Option<(u32, u32, Vec<u8>)> {
    let mut dec = png::Decoder::new(std::io::Cursor::new(bytes));
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    dec.set_limits(png::Limits { bytes: (max_side as usize * max_side as usize * 4) * 2 });
    let mut reader = dec.read_info().ok()?;
    let (w, h) = (reader.info().width, reader.info().height);
    if w == 0 || h == 0 || w > max_side || h > max_side {
        return None;
    }
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    let data = buf.get(..info.buffer_size())?;
    let px: Vec<u8> = match info.color_type {
        png::ColorType::Rgba => data.to_vec(),
        png::ColorType::Rgb => data.chunks_exact(3).flat_map(|c| [c[0], c[1], c[2], 255]).collect(),
        png::ColorType::GrayscaleAlpha => data.chunks_exact(2).flat_map(|c| [c[0], c[0], c[0], c[1]]).collect(),
        png::ColorType::Grayscale => data.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => return None,
    };
    (px.len() as u64 == u64::from(w) * u64::from(h) * 4).then_some((w, h, px))
}

/// `over` drawn on `under` at `opacity` (straight alpha "source over").
pub fn over_pixel(under: Rgba, over: Rgba, opacity: u8) -> Rgba {
    let sa = u32::from(over[3]) * u32::from(opacity) / 255;
    if sa == 0 {
        return under;
    }
    if sa == 255 {
        return [over[0], over[1], over[2], 255];
    }
    let da = u32::from(under[3]);
    // out_a = sa + da * (1 - sa), in 0..=255 (times 255 to keep precision).
    let out_a255 = sa * 255 + da * (255 - sa);
    if out_a255 == 0 {
        return CLEAR;
    }
    let ch = |s: u8, d: u8| -> u8 {
        let v = (u32::from(s) * sa * 255 + u32::from(d) * da * (255 - sa) + out_a255 / 2) / out_a255;
        v.min(255) as u8
    };
    [ch(over[0], under[0]), ch(over[1], under[1]), ch(over[2], under[2]), ((out_a255 + 127) / 255).min(255) as u8]
}

/// `#rrggbb` or `#rrggbbaa` (the `#` optional) → a pixel.
pub fn parse_hex(s: &str) -> Option<Rgba> {
    let h = s.trim().trim_start_matches('#');
    if !h.is_ascii() || !(h.len() == 6 || h.len() == 8) {
        return None;
    }
    let byte = |i: usize| h.get(i..i + 2).and_then(|b| u8::from_str_radix(b, 16).ok());
    Some([byte(0)?, byte(2)?, byte(4)?, if h.len() == 8 { byte(6)? } else { 255 }])
}

/// A pixel as `#rrggbb`, or `#rrggbbaa` when not opaque.
pub fn to_hex(c: Rgba) -> String {
    if c[3] == 255 { format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]) } else { format!("#{:02x}{:02x}{:02x}{:02x}", c[0], c[1], c[2], c[3]) }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED: Rgba = [255, 0, 0, 255];

    #[test]
    fn a_canvas_refuses_sizes_it_cannot_hold() {
        assert!(Canvas::new(0, 4).is_none());
        assert!(Canvas::new(4, 1025).is_none());
        let c = Canvas::new(3, 2).unwrap();
        assert_eq!(c.pixels().len(), 6);
        assert!(c.is_clear());
    }

    #[test]
    fn pixels_outside_are_not_there() {
        let mut c = Canvas::new(2, 2).unwrap();
        assert!(c.set(1, 1, RED));
        assert!(!c.set(2, 0, RED));
        assert!(!c.set(-1, 0, RED));
        assert_eq!(c.get(1, 1), Some(RED));
        assert_eq!(c.get(0, 2), None);
    }

    #[test]
    fn a_canvas_round_trips_through_png() {
        let mut c = Canvas::new(5, 3).unwrap();
        c.set(0, 0, RED);
        c.set(4, 2, [10, 20, 30, 128]);
        let png = c.to_png().unwrap();
        assert_eq!(Canvas::from_png(&png), Some(c));
        assert_eq!(Canvas::from_png(b"not a png"), None);
        assert_eq!(Canvas::from_png(&png[..png.len() / 2]), None);
    }

    #[test]
    fn half_transparent_over_white_is_pink() {
        assert_eq!(over_pixel([255, 255, 255, 255], [255, 0, 0, 128], 255), [255, 127, 127, 255]);
        assert_eq!(over_pixel(CLEAR, RED, 255), RED);
        assert_eq!(over_pixel(RED, [0, 0, 255, 255], 0), RED, "nothing at opacity 0");
        assert_eq!(over_pixel(CLEAR, [0, 0, 255, 255], 51), [0, 0, 255, 51]);
    }

    #[test]
    fn hex_colours_read_and_write() {
        assert_eq!(parse_hex("#ff0000"), Some(RED));
        assert_eq!(parse_hex("00ff0080"), Some([0, 255, 0, 128]));
        assert_eq!(parse_hex("#ff00"), None);
        assert_eq!(parse_hex("#ééé"), None);
        assert_eq!(to_hex(RED), "#ff0000");
        assert_eq!(to_hex([1, 2, 3, 4]), "#01020304");
    }

    #[test]
    fn resizing_moves_and_cuts() {
        let mut c = Canvas::new(2, 2).unwrap();
        c.set(0, 0, RED);
        let r = c.resized(3, 3, 1, 1).unwrap();
        assert_eq!(r.get(1, 1), Some(RED));
        assert_eq!(r.get(0, 0), Some(CLEAR));
        let cut = c.resized(1, 1, -1, 0).unwrap();
        assert!(cut.is_clear());
    }
}
