//! Pixel fonts (G16.6): a glyph sheet, a grid of cells drawn one character each, turned into a
//! TrueType font whose glyphs are the cells' painted pixels as squares. Text layers then use it
//! like any font (it lays out, wraps and hugs as usual), and Pixel text draws it sharp. Glyphs are
//! proportional: each is as wide as its painted columns plus the spacing; an empty cell (the
//! space) is half a cell wide.

use serde::{Deserialize, Serialize};

use crate::canvas::Canvas;

/// The plugin data key a glyph sheet's font settings are stored under.
pub const KEY: &str = "graybox.pixel.font";
/// The characters a new font's cells hold, in order: printable ASCII from the space.
pub const ASCII: &str = " !\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~";
/// The largest cell side.
pub const MAX_CELL: u32 = 128;
/// The most characters in one font.
pub const MAX_CHARS: usize = 1024;
/// Font units per art pixel.
const UNIT: i32 = 64;

/// How a glyph sheet is read as a font.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Font {
    /// The family name text layers use.
    pub name: String,
    /// The characters of the cells, left to right then down.
    pub chars: String,
    pub cell_width: u32,
    pub cell_height: u32,
    /// Rows from a cell's top to the baseline (the rows below are descenders).
    pub baseline: u32,
    /// Empty art pixels after each glyph.
    pub spacing: u32,
}

impl Font {
    /// A font named `name` for `cell_width` × `cell_height` cells: printable ASCII, a quarter of
    /// the cell below the baseline, one pixel between glyphs.
    pub fn new(name: &str, cell_width: u32, cell_height: u32) -> Self {
        let (w, h) = (cell_width.clamp(1, MAX_CELL), cell_height.clamp(1, MAX_CELL));
        Self { name: name.into(), chars: ASCII.into(), cell_width: w, cell_height: h, baseline: h - h / 4, spacing: 1 }
    }

    /// The same settings kept in range (None without a name).
    pub fn sanitized(mut self) -> Option<Self> {
        self.name = self.name.trim().chars().take(64).collect();
        if self.name.is_empty() {
            return None;
        }
        self.chars = self.chars.chars().take(MAX_CHARS).collect();
        self.cell_width = self.cell_width.clamp(1, MAX_CELL);
        self.cell_height = self.cell_height.clamp(1, MAX_CELL);
        self.baseline = self.baseline.clamp(1, self.cell_height);
        self.spacing = self.spacing.min(MAX_CELL);
        Some(self)
    }

    /// The em in art pixels: the cell height rounded up to a power of two, so text lays every
    /// glyph pixel out exactly (text layout puts advances on a grid of 1/1024 em). Text at
    /// `em() × scale` points draws one glyph pixel as `scale` page units.
    pub fn em(&self) -> u32 {
        self.cell_height.clamp(1, MAX_CELL).next_power_of_two()
    }

    pub fn load(text: &str) -> Option<Self> {
        serde_json::from_str::<Self>(text).ok()?.sanitized()
    }

    pub fn save(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

/// One character's glyph: its painted pixels as rectangles (x, y from the cell's top left, width,
/// height, in art pixels, already moved left to the first painted column) and its advance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Glyph {
    pub ch: char,
    pub rects: Vec<(u32, u32, u32, u32)>,
    pub advance: u32,
}

/// The glyphs of `sheet` read as `font` (a character whose cell falls outside the sheet is left
/// out; the first of a repeated character wins).
pub fn glyphs(sheet: &Canvas, font: &Font) -> Vec<Glyph> {
    let (cw, ch) = (font.cell_width.max(1), font.cell_height.max(1));
    let cols = sheet.width() / cw;
    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for (i, c) in font.chars.chars().enumerate().take(MAX_CHARS) {
        let Ok(i) = u32::try_from(i) else { break };
        if cols == 0 || !seen.insert(c) {
            continue;
        }
        let (x0, y0) = ((i % cols) * cw, (i / cols) * ch);
        if y0 + ch > sheet.height() {
            continue;
        }
        let painted = |x: u32, y: u32| sheet.get(i64::from(x0 + x), i64::from(y0 + y)).is_some_and(|p| p[3] >= 128);
        let cols_painted: Vec<u32> = (0..cw).filter(|x| (0..ch).any(|y| painted(*x, y))).collect();
        let (Some(&left), Some(&right)) = (cols_painted.first(), cols_painted.last()) else {
            out.push(Glyph { ch: c, rects: Vec::new(), advance: (cw / 2).max(1) });
            continue;
        };
        // Runs of painted pixels in each row, grown downwards while the row below has the same run.
        let mut rects: Vec<(u32, u32, u32, u32)> = Vec::new();
        let mut open: Vec<usize> = Vec::new();
        for y in 0..ch {
            let mut runs = Vec::new();
            let mut x = left;
            while x <= right {
                if painted(x, y) {
                    let start = x;
                    while x <= right && painted(x, y) {
                        x += 1;
                    }
                    runs.push((start - left, x - start));
                } else {
                    x += 1;
                }
            }
            let mut next_open = Vec::new();
            for (rx, rw) in runs {
                match open.iter().copied().find(|&r| rects.get(r).is_some_and(|q| q.0 == rx && q.2 == rw)) {
                    Some(r) => {
                        if let Some(q) = rects.get_mut(r) {
                            q.3 += 1;
                        }
                        next_open.push(r);
                    }
                    None => {
                        rects.push((rx, y, rw, 1));
                        next_open.push(rects.len() - 1);
                    }
                }
            }
            open = next_open;
        }
        out.push(Glyph { ch: c, rects, advance: right - left + 1 + font.spacing });
    }
    out
}

/// A TrueType font file of `glyphs` as `font` (None when there are no glyphs).
pub fn ttf(font: &Font, glyphs: &[Glyph]) -> Option<Vec<u8>> {
    if glyphs.is_empty() {
        return None;
    }
    let mut sorted: Vec<&Glyph> = glyphs.iter().collect();
    sorted.sort_by_key(|g| g.ch);
    let n = u16::try_from(sorted.len() + 1).ok()?;
    let u = UNIT;
    let base = i32::try_from(font.baseline).ok()?;
    let height = i32::try_from(font.cell_height).ok()?;
    let ascent = base * u;
    let descent = (base - height) * u;
    let mut glyf = Vec::new();
    let mut loca: Vec<u32> = vec![0];
    let mut hmtx = Vec::new();
    // .notdef: empty, half a cell wide.
    let notdef_adv = i32::try_from((font.cell_width / 2).max(1)).ok()? * u;
    push_u16(&mut hmtx, notdef_adv as u16);
    push_i16(&mut hmtx, 0);
    loca.push(0);
    let (mut max_points, mut max_contours, mut max_adv) = (0u16, 0u16, notdef_adv);
    let (mut x_max, mut y_min, mut y_max) = (0i32, 0i32, 0i32);
    for g in &sorted {
        let adv = i32::try_from(g.advance).ok()? * u;
        max_adv = max_adv.max(adv);
        push_u16(&mut hmtx, u16::try_from(adv).ok()?);
        push_i16(&mut hmtx, 0);
        if !g.rects.is_empty() {
            let mut xs = Vec::new();
            let mut ys = Vec::new();
            for &(x, y, w, h) in &g.rects {
                let (x0, x1) = (i32::try_from(x).ok()? * u, i32::try_from(x + w).ok()? * u);
                let top = (base - i32::try_from(y).ok()?) * u;
                let bottom = top - i32::try_from(h).ok()? * u;
                // Clockwise (y up): bottom left, top left, top right, bottom right.
                xs.extend([x0, x0, x1, x1]);
                ys.extend([bottom, top, top, bottom]);
            }
            let (gx0, gx1) = (xs.iter().copied().min()?, xs.iter().copied().max()?);
            let (gy0, gy1) = (ys.iter().copied().min()?, ys.iter().copied().max()?);
            (x_max, y_min, y_max) = (x_max.max(gx1), y_min.min(gy0), y_max.max(gy1));
            let contours = u16::try_from(g.rects.len()).ok()?;
            let points = contours.checked_mul(4)?;
            (max_points, max_contours) = (max_points.max(points), max_contours.max(contours));
            push_i16(&mut glyf, contours as i16);
            for v in [gx0, gy0, gx1, gy1] {
                push_i16(&mut glyf, i16::try_from(v).ok()?);
            }
            for c in 0..contours {
                push_u16(&mut glyf, c * 4 + 3);
            }
            push_u16(&mut glyf, 0);
            glyf.extend(std::iter::repeat_n(1u8, usize::from(points)));
            let mut last = 0;
            for x in &xs {
                push_i16(&mut glyf, i16::try_from(x - last).ok()?);
                last = *x;
            }
            last = 0;
            for y in &ys {
                push_i16(&mut glyf, i16::try_from(y - last).ok()?);
                last = *y;
            }
            while glyf.len() % 4 != 0 {
                glyf.push(0);
            }
        }
        loca.push(u32::try_from(glyf.len()).ok()?);
    }
    // The em is a power of two of art pixels, so each pixel is a whole step of the advance grid.
    let upm = u16::try_from(i32::try_from(font.em()).ok()? * u).ok()?;
    let tables: Vec<(&[u8; 4], Vec<u8>)> = vec![
        (b"OS/2", os2(font, &sorted, ascent, descent, max_adv)?),
        (b"cmap", cmap(&sorted)?),
        (b"glyf", glyf),
        (b"head", head(upm, [0, y_min, x_max, y_max])?),
        (b"hhea", hhea(ascent, descent, max_adv, x_max, n)?),
        (b"hmtx", hmtx),
        (b"loca", loca.iter().flat_map(|o| o.to_be_bytes()).collect()),
        (b"maxp", maxp(n, max_points, max_contours)),
        (b"name", name(&font.name)?),
        (b"post", post()),
    ];
    Some(assemble(&tables))
}

fn push_u16(v: &mut Vec<u8>, x: u16) {
    v.extend(x.to_be_bytes());
}

fn push_i16(v: &mut Vec<u8>, x: i16) {
    v.extend(x.to_be_bytes());
}

fn push_u32(v: &mut Vec<u8>, x: u32) {
    v.extend(x.to_be_bytes());
}

fn i16_of(v: i32) -> Option<i16> {
    i16::try_from(v).ok()
}

fn head(upm: u16, [x0, y0, x1, y1]: [i32; 4]) -> Option<Vec<u8>> {
    let mut t = Vec::with_capacity(54);
    push_u32(&mut t, 0x0001_0000);
    push_u32(&mut t, 0x0001_0000);
    push_u32(&mut t, 0); // checksum adjustment, set once the file is put together
    push_u32(&mut t, 0x5F0F_3CF5);
    push_u16(&mut t, 0x000B);
    push_u16(&mut t, upm);
    t.extend([0u8; 16]); // created, modified
    for v in [x0, y0, x1, y1] {
        push_i16(&mut t, i16_of(v)?);
    }
    push_u16(&mut t, 0); // mac style
    push_u16(&mut t, 8); // lowest readable size
    push_i16(&mut t, 2);
    push_i16(&mut t, 1); // long loca
    push_i16(&mut t, 0);
    Some(t)
}

fn hhea(ascent: i32, descent: i32, max_adv: i32, x_max: i32, n: u16) -> Option<Vec<u8>> {
    let mut t = Vec::with_capacity(36);
    push_u32(&mut t, 0x0001_0000);
    push_i16(&mut t, i16_of(ascent)?);
    push_i16(&mut t, i16_of(descent)?);
    push_i16(&mut t, 0);
    push_u16(&mut t, u16::try_from(max_adv).ok()?);
    push_i16(&mut t, 0);
    push_i16(&mut t, 0);
    push_i16(&mut t, i16_of(x_max)?);
    push_i16(&mut t, 1);
    push_i16(&mut t, 0);
    push_i16(&mut t, 0);
    t.extend([0u8; 8]);
    push_i16(&mut t, 0);
    push_u16(&mut t, n);
    Some(t)
}

fn maxp(n: u16, points: u16, contours: u16) -> Vec<u8> {
    let mut t = Vec::with_capacity(32);
    push_u32(&mut t, 0x0001_0000);
    for v in [n, points, contours, 0, 0, 2] {
        push_u16(&mut t, v);
    }
    t.extend([0u8; 16]);
    t
}

fn os2(font: &Font, glyphs: &[&Glyph], ascent: i32, descent: i32, max_adv: i32) -> Option<Vec<u8>> {
    let avg = glyphs.iter().map(|g| i64::from(g.advance)).sum::<i64>() / glyphs.len().max(1) as i64 * i64::from(UNIT);
    let first = glyphs.first().map_or(0, |g| u32::from(g.ch).min(0xFFFF)) as u16;
    let last = glyphs.last().map_or(0, |g| u32::from(g.ch).min(0xFFFF)) as u16;
    let cell = i32::try_from(font.cell_height).ok()? * UNIT;
    let mut t = Vec::with_capacity(96);
    push_u16(&mut t, 4);
    push_i16(&mut t, i16::try_from(avg).ok()?);
    push_u16(&mut t, 400);
    push_u16(&mut t, 5);
    push_u16(&mut t, 0);
    let sub = i16_of(cell / 2)?;
    for v in [sub, sub, 0, sub / 2, sub, sub, 0, sub, i16_of(UNIT)?, i16_of(ascent / 2)?] {
        push_i16(&mut t, v);
    }
    push_i16(&mut t, 0);
    t.extend([0u8; 10]); // panose
    t.extend([0u8; 16]); // unicode ranges
    t.extend(*b"GBOX");
    push_u16(&mut t, 0x0040); // regular
    push_u16(&mut t, first);
    push_u16(&mut t, last);
    push_i16(&mut t, i16_of(ascent)?);
    push_i16(&mut t, i16_of(descent)?);
    push_i16(&mut t, 0);
    push_u16(&mut t, u16::try_from(ascent.max(0)).ok()?);
    push_u16(&mut t, u16::try_from((-descent).max(0)).ok()?);
    push_u32(&mut t, 1); // Latin 1
    push_u32(&mut t, 0);
    push_i16(&mut t, i16_of(ascent / 2)?);
    push_i16(&mut t, i16_of(ascent)?);
    push_u16(&mut t, 0);
    push_u16(&mut t, 32);
    push_u16(&mut t, 1);
    let _ = max_adv;
    Some(t)
}

fn cmap(glyphs: &[&Glyph]) -> Option<Vec<u8>> {
    let groups = u32::try_from(glyphs.len()).ok()?;
    let sub_len = 16 + 12 * groups;
    let mut t = Vec::new();
    push_u16(&mut t, 0);
    push_u16(&mut t, 2);
    // Unicode full repertoire and Windows full repertoire, both to the one subtable after them.
    for (platform, encoding) in [(0u16, 4u16), (3, 10)] {
        push_u16(&mut t, platform);
        push_u16(&mut t, encoding);
        push_u32(&mut t, 20);
    }
    push_u16(&mut t, 12);
    push_u16(&mut t, 0);
    push_u32(&mut t, sub_len);
    push_u32(&mut t, 0);
    push_u32(&mut t, groups);
    for (i, g) in glyphs.iter().enumerate() {
        let c = u32::from(g.ch);
        push_u32(&mut t, c);
        push_u32(&mut t, c);
        push_u32(&mut t, u32::try_from(i + 1).ok()?);
    }
    Some(t)
}

fn name(family: &str) -> Option<Vec<u8>> {
    let ps: String = family.chars().filter(|c| c.is_ascii_alphanumeric()).take(60).collect();
    let ps = if ps.is_empty() { "GrayboxPixelFont".to_string() } else { ps };
    let strings =
        [(1u16, family.to_string()), (2, "Regular".into()), (3, format!("Graybox: {family}")), (4, family.to_string()), (6, format!("{ps}-Regular"))];
    let encoded: Vec<(u16, Vec<u8>)> = strings.iter().map(|(id, s)| (*id, s.encode_utf16().flat_map(u16::to_be_bytes).collect())).collect();
    let count = u16::try_from(encoded.len()).ok()?;
    let mut t = Vec::new();
    push_u16(&mut t, 0);
    push_u16(&mut t, count);
    push_u16(&mut t, 6 + 12 * count);
    let mut offset = 0u16;
    for (id, bytes) in &encoded {
        let len = u16::try_from(bytes.len()).ok()?;
        for v in [3, 1, 0x0409, *id, len, offset] {
            push_u16(&mut t, v);
        }
        offset = offset.checked_add(len)?;
    }
    for (_, bytes) in &encoded {
        t.extend(bytes);
    }
    Some(t)
}

fn post() -> Vec<u8> {
    let mut t = Vec::with_capacity(32);
    push_u32(&mut t, 0x0003_0000);
    push_u32(&mut t, 0);
    push_i16(&mut t, -UNIT as i16);
    push_i16(&mut t, UNIT as i16);
    t.extend([0u8; 20]);
    t
}

fn checksum(data: &[u8]) -> u32 {
    data.chunks(4).fold(0u32, |sum, c| {
        let mut w = [0u8; 4];
        for (d, s) in w.iter_mut().zip(c) {
            *d = *s;
        }
        sum.wrapping_add(u32::from_be_bytes(w))
    })
}

/// The tables (sorted by tag) as one font file.
fn assemble(tables: &[(&[u8; 4], Vec<u8>)]) -> Vec<u8> {
    let n = tables.len() as u16;
    let mut pow = 1u16;
    let mut log = 0u16;
    while pow * 2 <= n {
        pow *= 2;
        log += 1;
    }
    let mut out = Vec::new();
    push_u32(&mut out, 0x0001_0000);
    push_u16(&mut out, n);
    push_u16(&mut out, pow * 16);
    push_u16(&mut out, log);
    push_u16(&mut out, n * 16 - pow * 16);
    let mut offset = 12 + 16 * u32::from(n);
    let mut head_at = None;
    for (tag, data) in tables {
        out.extend(*tag);
        push_u32(&mut out, checksum(data));
        push_u32(&mut out, offset);
        push_u32(&mut out, data.len() as u32);
        offset += (data.len() as u32).div_ceil(4) * 4;
    }
    for (tag, data) in tables {
        if *tag == b"head" {
            head_at = Some(out.len());
        }
        out.extend(data);
        while out.len() % 4 != 0 {
            out.push(0);
        }
    }
    if let Some(at) = head_at {
        let adjust = 0xB1B0_AFBAu32.wrapping_sub(checksum(&out));
        if let Some(slot) = out.get_mut(at + 8..at + 12) {
            slot.copy_from_slice(&adjust.to_be_bytes());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const INK: [u8; 4] = [20, 20, 30, 255];

    /// A sheet of 4 × 4 cells, two across: `A` a 2 × 3 block with a hole, `.` one pixel, space empty.
    fn sheet() -> (Canvas, Font) {
        let mut c = Canvas::new(8, 8).unwrap();
        for (x, y) in [(1, 0), (2, 0), (1, 1), (1, 2), (2, 2)] {
            c.set(x, y, INK);
        }
        c.set(4 + 3, 2, INK);
        let mut f = Font::new("Tiny", 4, 4);
        f.chars = "A. ".into();
        (c, f)
    }

    #[test]
    fn cells_become_proportional_glyphs_made_of_merged_rectangles() {
        let (c, f) = sheet();
        let g = glyphs(&c, &f);
        assert_eq!(g.iter().map(|g| g.ch).collect::<String>(), "A. ");
        // A: the left column runs down 3 rows; the right pixels stay apart (rows 0 and 2).
        assert_eq!(g[0].rects, vec![(0, 0, 2, 1), (0, 1, 1, 1), (0, 2, 2, 1)]);
        assert_eq!(g[0].advance, 3, "2 wide plus 1 of spacing");
        assert_eq!((g[1].rects.clone(), g[1].advance), (vec![(0, 2, 1, 1)], 2), "moved to the left edge");
        assert_eq!((g[2].rects.len(), g[2].advance), (0, 2), "an empty cell is half a cell wide");
    }

    #[test]
    fn the_font_file_has_its_tables_and_a_valid_checksum() {
        let (c, f) = sheet();
        let bytes = ttf(&f, &glyphs(&c, &f)).unwrap();
        assert_eq!(checksum(&bytes), 0xB1B0_AFBA, "the whole file sums to the magic number");
        let n = u16::from_be_bytes([bytes[4], bytes[5]]) as usize;
        let tags: Vec<&[u8]> = (0..n).map(|i| &bytes[12 + 16 * i..16 + 16 * i]).collect();
        assert_eq!(tags, [&b"OS/2"[..], b"cmap", b"glyf", b"head", b"hhea", b"hmtx", b"loca", b"maxp", b"name", b"post"]);
        assert!(ttf(&f, &[]).is_none());
        assert_eq!(Font::load(&f.save()), Some(f.clone()));
        assert!(Font::load(r#"{"name":" ","chars":"","cellWidth":0,"cellHeight":0,"baseline":0,"spacing":0}"#).is_none());
    }
}
