//! A very small Markdown reader for Claude's replies: paragraphs, headings, bullets, code fences,
//! **bold** and `code`. Everything else is shown as the plain text it is.

/// A run of text with its inline style.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub bold: bool,
    pub code: bool,
}

/// A block of the reply.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Block {
    Para(Vec<Span>),
    Heading(Vec<Span>),
    Bullet(Vec<Span>),
    Code(String),
}

/// Split `text` into blocks (a blank line ends a paragraph; code fences are kept verbatim).
pub fn blocks(text: &str) -> Vec<Block> {
    let mut out = Vec::new();
    let mut para: Vec<&str> = Vec::new();
    let mut code: Option<Vec<&str>> = None;
    let flush = |para: &mut Vec<&str>, out: &mut Vec<Block>| {
        if !para.is_empty() {
            out.push(Block::Para(spans(&para.join(" "))));
            para.clear();
        }
    };
    for line in text.lines() {
        if let Some(lines) = code.as_mut() {
            if line.trim_start().starts_with("```") {
                out.push(Block::Code(lines.join("\n")));
                code = None;
            } else {
                lines.push(line);
            }
            continue;
        }
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            flush(&mut para, &mut out);
            code = Some(Vec::new());
        } else if trimmed.is_empty() {
            flush(&mut para, &mut out);
        } else if let Some(rest) = trimmed.strip_prefix("- ").or_else(|| trimmed.strip_prefix("* ")) {
            flush(&mut para, &mut out);
            out.push(Block::Bullet(spans(rest)));
        } else if trimmed.starts_with('#') {
            flush(&mut para, &mut out);
            out.push(Block::Heading(spans(trimmed.trim_start_matches('#').trim())));
        } else {
            para.push(trimmed);
        }
    }
    if let Some(lines) = code {
        out.push(Block::Code(lines.join("\n")));
    }
    flush(&mut para, &mut out);
    out
}

/// Inline styles: `**bold**` and `` `code` ``. An unclosed marker stays as typed.
pub fn spans(text: &str) -> Vec<Span> {
    let mut out: Vec<Span> = Vec::new();
    let mut rest = text;
    let mut plain = String::new();
    let push = |out: &mut Vec<Span>, plain: &mut String| {
        if !plain.is_empty() {
            out.push(Span { text: std::mem::take(plain), bold: false, code: false });
        }
    };
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("**")
            && let Some(end) = after.find("**")
            && end > 0
            && let (Some(inner), Some(tail)) = (after.get(..end), after.get(end + 2..))
        {
            push(&mut out, &mut plain);
            out.push(Span { text: inner.to_string(), bold: true, code: false });
            rest = tail;
        } else if let Some(after) = rest.strip_prefix('`')
            && let Some(end) = after.find('`')
            && end > 0
            && let (Some(inner), Some(tail)) = (after.get(..end), after.get(end + 1..))
        {
            push(&mut out, &mut plain);
            out.push(Span { text: inner.to_string(), bold: false, code: true });
            rest = tail;
        } else if let Some(c) = rest.chars().next() {
            plain.push(c);
            rest = rest.get(c.len_utf8()..).unwrap_or("");
        } else {
            break;
        }
    }
    push(&mut out, &mut plain);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(s: &str) -> Span {
        Span { text: s.into(), bold: false, code: false }
    }

    #[test]
    fn bold_and_code_inline() {
        let s = spans("make **Menu** a `frame` ok");
        assert_eq!(s.len(), 5);
        assert_eq!(s[0], plain("make "));
        assert!(s[1].bold && s[1].text == "Menu");
        assert!(s[3].code && s[3].text == "frame");
    }

    #[test]
    fn unclosed_markers_stay_as_typed() {
        assert_eq!(spans("a ** b"), vec![plain("a ** b")]);
        assert_eq!(spans("a ` b"), vec![plain("a ` b")]);
        assert_eq!(spans("****"), vec![plain("****")]);
        assert_eq!(spans("ação **é** ótima ✓"), vec![plain("ação "), Span { text: "é".into(), bold: true, code: false }, plain(" ótima ✓")]);
    }

    #[test]
    fn blocks_lists_headings_code() {
        let b = blocks("# Title\nOne\ntwo\n\n- a\n- b **x**\n```\nlet x = 1;\n```\nend");
        assert_eq!(b.len(), 6);
        assert!(matches!(&b[0], Block::Heading(_)));
        assert_eq!(b[1], Block::Para(vec![plain("One two")]));
        assert!(matches!(&b[2], Block::Bullet(_)) && matches!(&b[3], Block::Bullet(_)));
        assert_eq!(b[4], Block::Code("let x = 1;".into()));
        assert_eq!(b[5], Block::Para(vec![plain("end")]));
    }

    #[test]
    fn an_open_fence_keeps_what_follows_as_code() {
        assert_eq!(blocks("```\nabc"), vec![Block::Code("abc".into())]);
        assert!(blocks("").is_empty());
    }
}
