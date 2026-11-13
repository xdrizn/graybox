//! Markdown export: what each block becomes.

use crate::docs::{BlockKind, DocId, Documents, Place, Run, Table};
use crate::docs_md::to_markdown;

fn md(d: &Documents, id: DocId) -> String {
    let doc = d.get(id).unwrap();
    to_markdown(&doc.title, &doc.icon, doc.current().unwrap(), None)
}

fn add(d: &mut Documents, id: DocId, kind: BlockKind, text: &str) -> crate::docs::BlockId {
    d.insert_block(id, None, Place::default(), kind, if text.is_empty() { vec![] } else { vec![Run::plain(text)] }).unwrap()
}

#[test]
fn a_document_is_a_title_a_version_line_and_its_blocks() {
    let mut d = Documents::default();
    let id = d.create("Spec", None, None).unwrap();
    d.set_icon(id, "📄").unwrap();
    add(&mut d, id, BlockKind::Heading { level: 1 }, "Goals");
    add(&mut d, id, BlockKind::Paragraph, "Players read the design.");
    assert_eq!(md(&d, id), "# 📄 Spec\n\n*Version 1 · draft*\n\n## Goals\n\nPlayers read the design.\n");
}

#[test]
fn an_empty_title_reads_untitled_and_the_code_is_shown() {
    let mut d = Documents::default();
    let id = d.create("", None, None).unwrap();
    let doc = d.get(id).unwrap();
    let out = to_markdown(&doc.title, &doc.icon, doc.current().unwrap(), Some("GBX-7Q4M-2KD9"));
    assert_eq!(out, "# Untitled\n\n*Version 1 · draft* · `GBX-7Q4M-2KD9`\n");
}

#[test]
fn lists_keep_together_count_and_nest() {
    let mut d = Documents::default();
    let id = d.create("L", None, None).unwrap();
    add(&mut d, id, BlockKind::Numbered, "one");
    let two = add(&mut d, id, BlockKind::Numbered, "two");
    d.insert_block(id, None, Place { parent: Some(two), index: None }, BlockKind::Bulleted, vec![Run::plain("inner")]).unwrap();
    add(&mut d, id, BlockKind::Paragraph, "after");
    add(&mut d, id, BlockKind::Checklist { checked: true }, "done");
    add(&mut d, id, BlockKind::Checklist { checked: false }, "todo");
    let out = md(&d, id);
    assert!(out.contains("1. one\n2. two\n   - inner\n\nafter\n"), "{out}");
    assert!(out.contains("- [x] done\n- [ ] todo\n"), "{out}");
}

#[test]
fn text_looks_become_markdown_and_syntax_characters_are_escaped() {
    let mut d = Documents::default();
    let id = d.create("T", None, None).unwrap();
    let runs = vec![
        Run::plain("a * b and "),
        Run { text: "bold ".into(), bold: true, ..Run::default() },
        Run { text: "it".into(), italic: true, ..Run::default() },
        Run::plain(" "),
        Run { text: "gone".into(), strike: true, ..Run::default() },
        Run::plain(" "),
        Run { text: "x_1".into(), code: true, ..Run::default() },
        Run::plain(" "),
        Run { text: "site".into(), link: Some("https://example.test".into()), ..Run::default() },
    ];
    d.insert_block(id, None, Place::default(), BlockKind::Paragraph, runs).unwrap();
    let out = md(&d, id);
    assert!(out.contains("a \\* b and **bold** *it* ~~gone~~ `x_1` [site](https://example.test)\n"), "{out}");
}

#[test]
fn quotes_callouts_dividers_links_and_decisions() {
    let mut d = Documents::default();
    let id = d.create("Q", None, None).unwrap();
    add(&mut d, id, BlockKind::Quote, "wise");
    add(&mut d, id, BlockKind::Callout { icon: "!".into() }, "careful");
    add(&mut d, id, BlockKind::Divider, "");
    add(&mut d, id, BlockKind::Link { url: "https://example.test".into() }, "docs");
    add(&mut d, id, BlockKind::Decision { who: "Ana".into(), when: "2026-11-12".into(), why: "cheaper".into() }, "Use the 9-slice");
    let out = md(&d, id);
    assert!(out.contains("> wise\n\n> ! careful\n\n---\n\n[docs](https://example.test)\n\n"), "{out}");
    // Each line its own paragraph inside the quote, or Markdown would run them together.
    assert!(out.contains("> **Decision:** Use the 9-slice\n>\n> **Who:** Ana\n>\n> **When:** 2026-11-12\n>\n> **Why:** cheaper\n"), "{out}");
}

#[test]
fn a_table_has_a_header_row_even_when_the_first_row_is_data() {
    let mut d = Documents::default();
    let id = d.create("Tb", None, None).unwrap();
    let cell = |s: &str| vec![Run::plain(s)];
    let table = Table { rows: vec![vec![cell("A"), cell("B")], vec![cell("1"), cell("x|y")]], header: true };
    d.insert_block(id, None, Place::default(), BlockKind::Table { table: table.clone() }, vec![]).unwrap();
    assert!(md(&d, id).contains("| A | B |\n| --- | --- |\n| 1 | x\\|y |\n"));
    let id2 = d.create("Tb2", None, None).unwrap();
    d.insert_block(id2, None, Place::default(), BlockKind::Table { table: Table { header: false, ..table } }, vec![]).unwrap();
    assert!(md(&d, id2).contains("|  |  |\n| --- | --- |\n| A | B |\n| 1 | x\\|y |\n"));
}

#[test]
fn toggles_wrap_their_content_and_unknown_blocks_vanish() {
    let mut d = Documents::default();
    let id = d.create("Tg", None, None).unwrap();
    let t = add(&mut d, id, BlockKind::Toggle { open: false }, "More");
    d.insert_block(id, None, Place { parent: Some(t), index: None }, BlockKind::Paragraph, vec![Run::plain("hidden")]).unwrap();
    let out = md(&d, id);
    assert!(out.contains("<details>\n<summary>More</summary>\n\nhidden\n</details>\n"), "{out}");
    let mut version = d.get(id).unwrap().current().unwrap().clone();
    version.blocks.push(crate::docs::Block::new(crate::docs::BlockId(99), BlockKind::Unknown));
    assert_eq!(to_markdown("Tg", "", &version, None), to_markdown("Tg", "", d.get(id).unwrap().current().unwrap(), None));
}
