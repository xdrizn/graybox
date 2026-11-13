//! Palette files: GIMP palettes (`.gpl`, which most pixel editors read and write) and plain hex
//! lists (`.hex`, one `rrggbb` per line), read and written.

use crate::canvas::{Rgba, parse_hex, to_hex};
use crate::sprite::MAX_PALETTE;

/// The colours of a palette file, `.gpl` or `.hex` (told apart by the `GIMP Palette` header), in
/// order, at most [`MAX_PALETTE`]. Lines that aren't colours are skipped.
pub fn parse(text: &str) -> Vec<Rgba> {
    parse_named(text).into_iter().map(|(c, _)| c).collect()
}

/// [`parse`], with each colour's name when the file gives one (a `.gpl` line's words after the
/// three numbers, unless they are just its hex).
pub fn parse_named(text: &str) -> Vec<(Rgba, Option<String>)> {
    let gpl = text.trim_start_matches('\u{feff}').trim_start().starts_with("GIMP Palette");
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') && gpl {
            continue;
        }
        let c = if gpl {
            let mut words = line.split_whitespace();
            let mut num = || words.next().and_then(|w| w.parse::<u16>().ok()).filter(|v| *v <= 255);
            match (num(), num(), num()) {
                (Some(r), Some(g), Some(b)) => {
                    let c = [r as u8, g as u8, b as u8, 255];
                    let name: String = words.collect::<Vec<_>>().join(" ").chars().filter(|ch| !ch.is_control()).take(64).collect();
                    let plain = name.is_empty() || to_hex(c).trim_start_matches('#').eq_ignore_ascii_case(name.trim_start_matches('#'));
                    Some((c, (!plain).then_some(name)))
                }
                _ => None,
            }
        } else {
            parse_hex(line).map(|c| (c, None))
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

/// A `.gpl` palette's `Name:`, when it has one.
pub fn name(text: &str) -> Option<String> {
    let line = text.lines().take(8).find_map(|l| l.trim().strip_prefix("Name:"))?;
    let name: String = line.trim().chars().filter(|c| !c.is_control()).take(64).collect();
    (!name.is_empty()).then_some(name)
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
        let colors = [[27, 29, 41, 255], [243, 241, 234, 255], [200, 52, 71, 255]];
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
