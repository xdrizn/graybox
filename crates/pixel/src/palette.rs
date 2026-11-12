//! Palette files: GIMP palettes (`.gpl`, which most pixel editors read and write) and plain hex
//! lists (`.hex`, one `rrggbb` per line), read and written.

use crate::canvas::{Rgba, parse_hex, to_hex};
use crate::sprite::MAX_PALETTE;

/// The colours of a palette file, `.gpl` or `.hex` (told apart by the `GIMP Palette` header), in
/// order, at most [`MAX_PALETTE`]. Lines that aren't colours are skipped.
pub fn parse(text: &str) -> Vec<Rgba> {
    let gpl = text.trim_start_matches('\u{feff}').trim_start().starts_with("GIMP Palette");
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') && gpl {
            continue;
        }
        let c = if gpl {
            let mut parts = line.split_whitespace().map(str::parse::<u16>);
            match (parts.next(), parts.next(), parts.next()) {
                (Some(Ok(r)), Some(Ok(g)), Some(Ok(b))) if r <= 255 && g <= 255 && b <= 255 => Some([r as u8, g as u8, b as u8, 255]),
                _ => None,
            }
        } else {
            parse_hex(line)
        };
        if let Some(c) = c {
            out.push(c);
            if out.len() >= MAX_PALETTE {
                break;
            }
        }
    }
    out
}

/// `colors` as a GIMP palette called `name`.
pub fn to_gpl(name: &str, colors: &[Rgba]) -> String {
    let name: String = name.chars().filter(|c| !c.is_control()).take(100).collect();
    let mut s = format!("GIMP Palette\nName: {name}\nColumns: 8\n#\n");
    for c in colors {
        s.push_str(&format!("{:>3} {:>3} {:>3}\t{}\n", c[0], c[1], c[2], to_hex(*c).trim_start_matches('#')));
    }
    s
}

/// `colors` as a hex list (`rrggbb` per line; `rrggbbaa` when not opaque).
pub fn to_hex_list(colors: &[Rgba]) -> String {
    colors.iter().map(|c| format!("{}\n", to_hex(*c).trim_start_matches('#'))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gimp_palettes_and_hex_lists_round_trip() {
        let colors = [[26, 28, 44, 255], [244, 244, 244, 255], [255, 0, 77, 255]];
        assert_eq!(parse(&to_gpl("Mine", &colors)), colors);
        assert_eq!(parse(&to_hex_list(&colors)), colors);
    }

    #[test]
    fn odd_lines_are_skipped() {
        let gpl = "GIMP Palette\nName: x\n#\n  0 0 0 Black\n300 0 0 too red\nnot a colour\n255 255 255\n";
        assert_eq!(parse(gpl), [[0, 0, 0, 255], [255, 255, 255, 255]]);
        assert_eq!(parse("ff0000\n\nzz\n#00ff00\n"), [[255, 0, 0, 255], [0, 255, 0, 255]]);
    }
}
