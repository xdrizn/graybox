//! Formatting some of a block's words (G13.2): bold, italic, underline, strike, code, a colour, a
//! highlight or a link on a range of characters, as the bar over selected text and Ctrl+B do it.

use super::edit::join_runs;
use super::{BlockId, DocError, DocId, Documents, Run, is_hex_color, split_runs};

/// One change to the look of some characters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mark {
    Bold,
    Italic,
    Underline,
    Strike,
    Code,
    /// Text colour (`#RGB`, `#RRGGBB` or with alpha), or none.
    Color(Option<String>),
    /// Highlight colour (`#RGB`, `#RRGGBB` or with alpha), or none.
    Highlight(Option<String>),
    /// A web address, or none.
    Link(Option<String>),
}

impl Mark {
    /// The mark called `name` (`bold`, `italic`, `underline`, `strike`, `code`, `color`, `highlight`,
    /// `link`), with `value` for the last three (`None` takes it off).
    pub fn from_name(name: &str, value: Option<&str>) -> Option<Self> {
        let value = value.map(str::trim).filter(|v| !v.is_empty()).map(str::to_string);
        Some(match name {
            "bold" => Self::Bold,
            "italic" => Self::Italic,
            "underline" => Self::Underline,
            "strike" => Self::Strike,
            "code" => Self::Code,
            "color" => Self::Color(value),
            "highlight" => Self::Highlight(value),
            "link" => Self::Link(value),
            _ => return None,
        })
    }

    /// The on/off look this mark toggles, if it is one.
    fn flag(&self, r: &Run) -> Option<bool> {
        Some(match self {
            Self::Bold => r.bold,
            Self::Italic => r.italic,
            Self::Underline => r.underline,
            Self::Strike => r.strike,
            Self::Code => r.code,
            _ => return None,
        })
    }

    fn apply(&self, r: &mut Run, on: bool) {
        match self {
            Self::Bold => r.bold = on,
            Self::Italic => r.italic = on,
            Self::Underline => r.underline = on,
            Self::Strike => r.strike = on,
            Self::Code => r.code = on,
            Self::Color(c) => r.color = c.clone(),
            Self::Highlight(c) => r.highlight = c.clone(),
            Self::Link(l) => r.link = l.clone(),
        }
    }

    fn valid(&self) -> bool {
        match self {
            Self::Color(Some(c)) | Self::Highlight(Some(c)) => is_hex_color(c),
            Self::Link(Some(l)) => l.chars().count() <= 2_000,
            _ => true,
        }
    }
}

/// The runs of characters `from..to` of `runs`, and those before and after them.
fn three(runs: &[Run], from: usize, to: usize) -> (Vec<Run>, Vec<Run>, Vec<Run>) {
    let (head, rest) = split_runs(runs, from);
    let (mid, tail) = split_runs(&rest, to.saturating_sub(from));
    (head, mid, tail)
}

/// Whether every character of `from..to` has the on/off look `mark` (false for an empty range or a
/// mark with a value).
pub fn has_mark(runs: &[Run], from: usize, to: usize, mark: &Mark) -> bool {
    let (_, mid, _) = three(runs, from, to);
    let mut any = false;
    for r in mid.iter().filter(|r| !r.text.is_empty()) {
        any = true;
        if mark.flag(r) != Some(true) {
            return false;
        }
    }
    any
}

/// `runs` with `mark` on characters `from..to`. An on/off look is set to `on`, or, when `on` is
/// `None`, turned off if every character already has it and on otherwise; a colour or a link is set.
/// Neighbours that end up looking the same are joined.
pub fn format_runs(runs: &[Run], from: usize, to: usize, mark: &Mark, on: Option<bool>) -> Vec<Run> {
    let on = on.unwrap_or_else(|| !has_mark(runs, from, to, mark));
    let (head, mut mid, tail) = three(runs, from, to);
    for r in &mut mid {
        mark.apply(r, on);
    }
    join_runs(join_runs(head, mid), tail).into_iter().filter(|r| !r.text.is_empty()).collect()
}

impl Documents {
    /// Put `mark` on characters `from..to` of block `id` (see [`format_runs`]). Refused for an empty
    /// range, a block without words and a colour that is not a hex colour.
    #[allow(clippy::too_many_arguments)]
    pub fn format_text(
        &mut self,
        doc: DocId,
        version: Option<u32>,
        id: BlockId,
        from: usize,
        to: usize,
        mark: &Mark,
        on: Option<bool>,
    ) -> Result<(), DocError> {
        if !mark.valid() {
            return Err(DocError::Invalid("a colour is written #RRGGBB (or #RGB, with alpha #RRGGBBAA); a link is at most 2000 characters"));
        }
        self.update_block(doc, version, id, |b| {
            if !b.kind.has_text() {
                return Err(DocError::Invalid("only a block with words can be formatted"));
            }
            let len = b.text_len();
            let (from, to) = (from.min(len), to.min(len));
            if from >= to {
                return Err(DocError::Invalid("select some characters to format"));
            }
            b.text = format_runs(&b.text, from, to, mark, on);
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::docs::{BlockKind, Place};

    fn looks(runs: &[Run]) -> Vec<(String, bool, bool)> {
        runs.iter().map(|r| (r.text.clone(), r.bold, r.italic)).collect()
    }

    fn s(t: &str, bold: bool, italic: bool) -> (String, bool, bool) {
        (t.to_string(), bold, italic)
    }

    #[test]
    fn bold_on_a_range_cuts_the_run_in_three() {
        let out = format_runs(&[Run::plain("Hello world")], 2, 5, &Mark::Bold, None);
        assert_eq!(looks(&out), [s("He", false, false), s("llo", true, false), s(" world", false, false)]);
    }

    #[test]
    fn bold_again_on_the_same_range_joins_the_runs_back() {
        let once = format_runs(&[Run::plain("Hello world")], 2, 5, &Mark::Bold, None);
        let twice = format_runs(&once, 2, 5, &Mark::Bold, None);
        assert_eq!(twice, [Run::plain("Hello world")]);
    }

    #[test]
    fn a_range_partly_bold_becomes_all_bold() {
        let runs = [Run::plain("ab"), Run { bold: true, ..Run::plain("cd") }, Run::plain("ef")];
        let out = format_runs(&runs, 1, 5, &Mark::Bold, None);
        assert_eq!(looks(&out), [s("a", false, false), s("bcde", true, false), s("f", false, false)]);
        assert!(has_mark(&out, 1, 5, &Mark::Bold));
        assert!(!has_mark(&out, 0, 5, &Mark::Bold));
    }

    #[test]
    fn looks_stack_and_colour_and_link_are_set() {
        let bold = format_runs(&[Run::plain("abc")], 0, 3, &Mark::Bold, None);
        let both = format_runs(&bold, 1, 2, &Mark::Italic, None);
        assert_eq!(looks(&both), [s("a", true, false), s("b", true, true), s("c", true, false)]);
        let red = format_runs(&[Run::plain("abc")], 0, 1, &Mark::Color(Some("#FF0000".into())), None);
        assert_eq!(red[0].color.as_deref(), Some("#FF0000"));
        let linked = format_runs(&red, 0, 3, &Mark::Link(Some("https://example.com".into())), None);
        assert!(linked.iter().all(|r| r.link.as_deref() == Some("https://example.com")));
        let unlinked = format_runs(&linked, 0, 3, &Mark::Link(None), None);
        assert!(unlinked.iter().all(|r| r.link.is_none()));
    }

    #[test]
    fn an_explicit_on_or_off_wins_over_toggling() {
        let bold = format_runs(&[Run::plain("abc")], 0, 3, &Mark::Bold, None);
        assert_eq!(format_runs(&bold, 0, 3, &Mark::Bold, Some(true)), bold);
        assert_eq!(format_runs(&bold, 0, 3, &Mark::Bold, Some(false)), [Run::plain("abc")]);
    }

    #[test]
    fn ranges_count_characters_not_bytes() {
        let out = format_runs(&[Run::plain("çã🙂x")], 1, 3, &Mark::Code, None);
        let texts: Vec<_> = out.iter().map(|r| (r.text.as_str(), r.code)).collect();
        assert_eq!(texts, [("ç", false), ("ã🙂", true), ("x", false)]);
    }

    #[test]
    fn the_store_formats_a_block_and_refuses_nonsense() {
        let mut d = Documents::default();
        let doc = d.create("Spec", None, None).unwrap();
        let b = d.insert_block(doc, None, Place::default(), BlockKind::Paragraph, vec![Run::plain("Hello")]).unwrap();
        d.format_text(doc, None, b, 0, 99, &Mark::Underline, None).unwrap();
        let block = d.get(doc).unwrap().current().unwrap().block(b).unwrap().clone();
        assert!(block.text.iter().all(|r| r.underline) && block.plain_text() == "Hello");
        assert!(d.format_text(doc, None, b, 3, 3, &Mark::Bold, None).is_err());
        assert!(d.format_text(doc, None, b, 0, 2, &Mark::Color(Some("red".into())), None).is_err());
        let line = d.insert_block(doc, None, Place::default(), BlockKind::Divider, vec![]).unwrap();
        assert!(d.format_text(doc, None, line, 0, 1, &Mark::Bold, None).is_err());
        assert_eq!(Mark::from_name("color", Some(" ")), Some(Mark::Color(None)));
        assert_eq!(Mark::from_name("blink", None), None);
    }
}
