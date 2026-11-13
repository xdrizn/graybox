//! Markdown export of a document version (G13.3). Pure text from the blocks: the same words the page
//! layout puts on paper, in the plain format any editor, wiki or chat reads.
//!
//! The document title is the `#` heading, so a block heading of level 1 is `##`, level 2 `###`
//! and so on (level 4 is `#####`). Lists, checklists, quotes, callouts, tables and links use the
//! usual syntax; what Markdown has no word for (underline, text colour) is left out, and the
//! blocks that are pictures of the file (embeds, images, shapes) become a short note.

use crate::docs::{AutoSource, Block, BlockKind, DocVersion, Run, Table};

/// The version as Markdown: the title, a line saying which version it is, then every block.
pub fn to_markdown(title: &str, icon: &str, version: &DocVersion, code: Option<&str>) -> String {
    let title = if title.trim().is_empty() { "Untitled" } else { title.trim() };
    let mut out = String::new();
    if icon.is_empty() {
        out.push_str(&format!("# {}\n\n", escape(title)));
    } else {
        out.push_str(&format!("# {icon} {}\n\n", escape(title)));
    }
    let state = if version.is_closed() { "closed" } else { "draft" };
    out.push_str(&format!("*Version {} · {state}*", version.number));
    if let Some(code) = code {
        out.push_str(&format!(" · `{code}`"));
    }
    out.push_str("\n\n");
    out.push_str(&blocks(&version.blocks, ""));
    while out.ends_with("\n\n") {
        out.pop();
    }
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// Some blocks (and what is inside them) as Markdown: what a copy of blocks gives other apps.
pub fn blocks_markdown(list: &[Block]) -> String {
    blocks(list, "")
}

/// Blocks one after the other, each line starting with `indent`.
fn blocks(list: &[Block], indent: &str) -> String {
    let mut out = String::new();
    let mut number = 0usize;
    let mut previous_item = false;
    for b in list {
        let is_item = matches!(b.kind, BlockKind::Bulleted | BlockKind::Numbered | BlockKind::Checklist { .. });
        number = if matches!(b.kind, BlockKind::Numbered) { number + 1 } else { 0 };
        // Items of one list sit together; everything else is a paragraph apart.
        if !out.is_empty() && !(is_item && previous_item) {
            out.push('\n');
        }
        previous_item = is_item;
        out.push_str(&block(b, indent, number));
    }
    out
}

/// One block (and what is inside it) as lines, each ending in a newline.
fn block(b: &Block, indent: &str, number: usize) -> String {
    let text = runs(&b.text);
    let pad = |s: &str| prefix_lines(s, indent);
    let child_indent = |extra: &str| format!("{indent}{extra}");
    match &b.kind {
        BlockKind::Paragraph => format!("{}{}", pad(&text), kids(b, indent)),
        BlockKind::Heading { level } => {
            let hashes = "#".repeat(usize::from((*level).clamp(1, 4)) + 1);
            format!("{indent}{hashes} {}\n{}", one_line(&text), kids(b, indent))
        }
        BlockKind::Bulleted => format!("{}{}", item("- ", &text, indent), children_in(b, &child_indent("  "))),
        BlockKind::Numbered => {
            let marker = format!("{number}. ");
            let extra = " ".repeat(marker.len());
            format!("{}{}", item(&marker, &text, indent), children_in(b, &child_indent(&extra)))
        }
        BlockKind::Checklist { checked } => {
            let marker = if *checked { "- [x] " } else { "- [ ] " };
            format!("{}{}", item(marker, &text, indent), children_in(b, &child_indent("  ")))
        }
        BlockKind::Toggle { .. } => {
            let mut s = format!("{indent}<details>\n{indent}<summary>{}</summary>\n", one_line(&text));
            let inner = blocks(&b.children, indent);
            if !inner.is_empty() {
                s.push_str(&format!("{indent}\n{inner}"));
            }
            s.push_str(&format!("{indent}</details>\n"));
            s
        }
        BlockKind::Callout { icon } => {
            let lead = if icon.is_empty() { String::new() } else { format!("{icon} ") };
            format!("{}{}", quote(&format!("{lead}{text}"), indent), kids(b, indent))
        }
        BlockKind::Quote => format!("{}{}", quote(&text, indent), kids(b, indent)),
        BlockKind::Divider => format!("{indent}---\n"),
        BlockKind::Table { table } => table_md(table, indent),
        BlockKind::Link { url } => {
            let label = if text.is_empty() { escape(url) } else { text };
            format!("{indent}[{label}]({url})\n{}", kids(b, indent))
        }
        BlockKind::Image { image, alt } => {
            let alt = if alt.is_empty() { "image".to_string() } else { escape(alt) };
            format!("{indent}![{alt}]({image})\n")
        }
        BlockKind::Shape { shape, fill } => {
            let fill = fill.as_deref().map(|f| format!(" `{f}`")).unwrap_or_default();
            format!("{indent}*[{shape}{fill}]*\n")
        }
        BlockKind::Swatch { color } => format!("{indent}`{color}` {text}\n"),
        BlockKind::Embed { node, live, .. } => {
            let how = if *live { "live" } else { "snapshot" };
            format!("{indent}*[Layer {node}, {how}]*\n")
        }
        BlockKind::Decision { who, when, why } => {
            let mut lines = vec![format!("**Decision:** {text}")];
            for (label, value) in [("Who", who), ("When", when), ("Why", why)] {
                if !value.is_empty() {
                    lines.push(format!("**{label}:** {}", escape(value)));
                }
            }
            // A paragraph each inside the quote, or Markdown would run the lines together.
            quote(&lines.join("\n\n"), indent)
        }
        BlockKind::AutoTable { source } => {
            let what = match source {
                AutoSource::Screens => "screens",
                AutoSource::Components => "components",
                AutoSource::Variables => "variables",
            };
            format!("{indent}*[automatic table of the {what}]*\n")
        }
        BlockKind::Signature { role } => {
            let who = if role.is_empty() { text } else { escape(role) };
            format!("{indent}Signature ({who}): ______________________\n")
        }
        BlockKind::Unknown => String::new(),
    }
}

/// A list item: `marker` then the text, the lines after the first lined up under it.
fn item(marker: &str, text: &str, indent: &str) -> String {
    let rest = format!("{indent}{}", " ".repeat(marker.len()));
    let mut lines = text.split('\n');
    let first = lines.next().unwrap_or("");
    let mut s = format!("{indent}{marker}{first}\n");
    for l in lines {
        s.push_str(&format!("{rest}{l}\n"));
    }
    s
}

/// The blocks inside a block that is not a list item, one level out.
fn kids(b: &Block, indent: &str) -> String {
    if b.children.is_empty() {
        return String::new();
    }
    format!("\n{}", blocks(&b.children, indent))
}

/// The blocks inside a list item, indented under it.
fn children_in(b: &Block, indent: &str) -> String {
    blocks(&b.children, indent)
}

fn quote(text: &str, indent: &str) -> String {
    let mut s = String::new();
    for l in text.split('\n') {
        if l.is_empty() {
            s.push_str(&format!("{indent}>\n"));
        } else {
            s.push_str(&format!("{indent}> {l}\n"));
        }
    }
    s
}

fn prefix_lines(text: &str, indent: &str) -> String {
    let mut s = String::new();
    for l in text.split('\n') {
        s.push_str(&format!("{indent}{l}\n"));
    }
    s
}

fn one_line(text: &str) -> String {
    text.replace('\n', " ")
}

fn table_md(t: &Table, indent: &str) -> String {
    let cols = t.rows.iter().map(Vec::len).max().unwrap_or(0);
    if cols == 0 {
        return String::new();
    }
    let cell = |row: &[Vec<Run>], c: usize| -> String { row.get(c).map(|runs_| one_line(&runs(runs_)).replace('|', "\\|")).unwrap_or_default() };
    let line = |cells: Vec<String>| format!("{indent}| {} |\n", cells.join(" | "));
    let mut s = String::new();
    let (head, body): (Vec<String>, &[Vec<Vec<Run>>]) = match t.rows.split_first() {
        Some((first, rest)) if t.header => ((0..cols).map(|c| cell(first, c)).collect(), rest),
        _ => (vec![String::new(); cols], t.rows.as_slice()),
    };
    s.push_str(&line(head));
    s.push_str(&line(vec!["---".to_string(); cols]));
    for row in body {
        s.push_str(&line((0..cols).map(|c| cell(row, c)).collect()));
    }
    s
}

/// Runs as Markdown: bold, italic, strikethrough, code and links.
fn runs(list: &[Run]) -> String {
    list.iter().map(run).collect()
}

fn run(r: &Run) -> String {
    let core = r.text.trim_matches([' ', '\t']);
    if core.is_empty() {
        return r.text.clone();
    }
    // The spaces stay outside the markers, or `** bold **` would not be bold.
    let lead_len = r.text.len() - r.text.trim_start_matches([' ', '\t']).len();
    let lead = r.text.get(..lead_len).unwrap_or("");
    let trail = r.text.get(r.text.trim_end_matches([' ', '\t']).len()..).unwrap_or("");
    let mut s = if r.code { code_span(core) } else { escape(core) };
    if r.bold {
        s = format!("**{s}**");
    }
    if r.italic {
        s = format!("*{s}*");
    }
    if r.strike {
        s = format!("~~{s}~~");
    }
    if let Some(url) = r.link.as_deref().filter(|u| !u.is_empty()) {
        s = format!("[{s}]({url})");
    }
    format!("{lead}{s}{trail}")
}

fn code_span(text: &str) -> String {
    // A longer fence than any run of backticks inside.
    let longest = text.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    let fence = "`".repeat(longest + 1);
    let pad = if text.starts_with('`') || text.ends_with('`') { " " } else { "" };
    format!("{fence}{pad}{text}{pad}{fence}")
}

/// Characters Markdown would take for syntax get a backslash.
fn escape(text: &str) -> String {
    let mut s = String::with_capacity(text.len());
    for c in text.chars() {
        if matches!(c, '\\' | '*' | '_' | '`' | '[' | ']' | '<' | '>' | '~') {
            s.push('\\');
        }
        s.push(c);
    }
    s
}
