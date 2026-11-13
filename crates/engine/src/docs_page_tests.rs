//! Page layout of a document version: what lands on which page.

use figcraft_doc::docs::{Block, BlockId, BlockKind, Run, Table};

use crate::docs_page::{Info, Item, Layout, PAGE_H, PAGE_W, layout};

fn info(closed: bool, code: Option<&'static str>) -> Info<'static> {
    Info { title: "Spec", icon: "", number: 1, closed, code, ..Info::default() }
}

fn block(id: u64, kind: BlockKind, text: &str) -> Block {
    let mut b = Block::new(BlockId(id), kind);
    if !text.is_empty() {
        b.text = vec![Run::plain(text)];
    }
    b
}

fn words(n: usize) -> String {
    (0..n).map(|i| format!("word{i}")).collect::<Vec<_>>().join(" ")
}

fn texts(l: &Layout, page: usize) -> Vec<String> {
    l.pages.get(page).map(|p| p.iter().filter(|i| matches!(i, Item::Text { .. })).map(|i| i.text().to_string()).collect()).unwrap_or_default()
}

fn has_fonts() -> bool {
    figcraft_text::find_face("Inter", "Regular").is_some()
}

#[test]
fn an_empty_version_is_one_page_with_title_version_and_footer() {
    let l = layout(&info(false, None), &[]);
    assert_eq!(l.pages.len(), 1);
    let t = texts(&l, 0);
    assert!(t.iter().any(|s| s == "Spec"), "{t:?}");
    assert!(t.iter().any(|s| s == "Version 1 · Draft"), "{t:?}");
    assert!(t.iter().any(|s| s == "Page 1 of 1"), "{t:?}");
    assert!(t.iter().any(|s| s.starts_with("Spec · version 1")), "{t:?}");
    assert!(!t.iter().any(|s| s == "CLOSED"));
}

#[test]
fn a_closed_version_has_a_seal_and_the_code_in_every_footer() {
    if !has_fonts() {
        return;
    }
    let blocks: Vec<Block> = (0..40).map(|i| block(i, BlockKind::Paragraph, &words(60))).collect();
    let l = layout(&info(true, Some("GBX-7Q4M-2KD9")), &blocks);
    assert!(l.pages.len() >= 3, "{} pages", l.pages.len());
    assert!(texts(&l, 0).iter().any(|s| s == "CLOSED"));
    assert!(!texts(&l, 1).iter().any(|s| s == "CLOSED"), "the seal is on the first page only");
    for page in 0..l.pages.len() {
        let t = texts(&l, page);
        assert!(t.iter().any(|s| s == "GBX-7Q4M-2KD9"), "page {page}: {t:?}");
        assert!(t.iter().any(|s| *s == format!("Page {} of {}", page + 1, l.pages.len())), "page {page}: {t:?}");
    }
}

#[test]
fn long_text_flows_onto_the_next_page_without_losing_a_word() {
    if !has_fonts() {
        return;
    }
    let long = words(1500);
    let l = layout(&info(false, None), &[block(1, BlockKind::Paragraph, &long)]);
    assert!(l.pages.len() >= 2, "{} pages", l.pages.len());
    let mut got = Vec::new();
    for page in &l.pages {
        for item in page {
            if let Item::Text { data, .. } = item
                && data.characters.starts_with("word")
            {
                got.push(data.characters.trim().to_string());
            }
        }
    }
    assert_eq!(got.join(" ").split_whitespace().collect::<Vec<_>>(), long.split_whitespace().collect::<Vec<_>>());
}

#[test]
fn nothing_is_placed_below_the_text_area_or_outside_the_page() {
    if !has_fonts() {
        return;
    }
    let blocks: Vec<Block> = (0..30)
        .flat_map(|i| [block(i * 2, BlockKind::Heading { level: 2 }, "A heading"), block(i * 2 + 1, BlockKind::Paragraph, &words(80))])
        .collect();
    let l = layout(&info(false, None), &blocks);
    for (n, page) in l.pages.iter().enumerate() {
        for item in page {
            if let Item::Text { x, y, w, h, .. } = item {
                assert!(*x >= 0.0 && x + w <= PAGE_W + 0.5, "page {n}: {x} {w}");
                // Footer text starts at PAGE_H - 54; everything else ends above the footer rule.
                assert!(y + h <= PAGE_H - 40.0, "page {n}: text at {y} + {h}");
            }
        }
    }
}

#[test]
fn a_heading_never_ends_a_page() {
    if !has_fonts() {
        return;
    }
    // Fill most of page 1, so the heading would sit at the very bottom.
    let mut blocks = vec![block(1, BlockKind::Paragraph, &words(330))];
    blocks.push(block(2, BlockKind::Heading { level: 1 }, "Last on the page?"));
    blocks.push(block(3, BlockKind::Paragraph, &words(10)));
    let l = layout(&info(false, None), &blocks);
    for page in 0..l.pages.len() {
        let t = texts(&l, page);
        let body: Vec<&String> = t.iter().filter(|s| !s.starts_with("Page ") && !s.starts_with("Spec ·")).collect();
        if let Some(last) = body.last() {
            assert_ne!(last.as_str(), "Last on the page?", "page {page}");
        }
    }
}

#[test]
fn lists_get_a_marker_each_and_numbers_count_up() {
    let blocks = vec![
        block(1, BlockKind::Numbered, "one"),
        block(2, BlockKind::Numbered, "two"),
        block(3, BlockKind::Paragraph, "between"),
        block(4, BlockKind::Numbered, "again"),
        block(5, BlockKind::Bulleted, "dot"),
        block(6, BlockKind::Checklist { checked: true }, "done"),
    ];
    let l = layout(&info(false, None), &blocks);
    let t = texts(&l, 0);
    for want in ["1.", "2.", "1."] {
        assert!(t.iter().any(|s| s == want), "{want} in {t:?}");
    }
    let shapes = l.pages[0].iter().filter(|i| matches!(i, Item::Shape { ellipse: true, .. })).count();
    assert_eq!(shapes, 1, "one bullet");
    let boxes = l.pages[0].iter().filter(|i| matches!(i, Item::Shape { ellipse: false, radius, fill: Some(_), .. } if *radius == 2.0)).count();
    assert_eq!(boxes, 1, "one ticked box");
}

#[test]
fn a_table_puts_each_cell_in_its_own_column() {
    let cell = |s: &str| vec![Run::plain(s)];
    let table = Table { rows: vec![vec![cell("A"), cell("B")], vec![cell("1"), cell("2")]], header: true };
    let l = layout(&info(false, None), &[block(1, BlockKind::Table { table }, "")]);
    let xs: Vec<f64> = l.pages[0]
        .iter()
        .filter_map(|i| match i {
            Item::Text { x, data, .. } if matches!(data.characters.as_str(), "A" | "B" | "1" | "2") => Some(*x),
            _ => None,
        })
        .collect();
    assert_eq!(xs.len(), 4);
    assert_eq!(xs[0], xs[2]);
    assert_eq!(xs[1], xs[3]);
    assert!(xs[1] > xs[0] + 100.0);
}

#[test]
fn quotes_callouts_and_decisions_say_their_words() {
    let decision = block(3, BlockKind::Decision { who: "Ana".into(), when: "2026-11-12".into(), why: "cheaper".into() }, "Use the 9-slice");
    let blocks = vec![block(1, BlockKind::Quote, "wise"), block(2, BlockKind::Callout { icon: String::new() }, "careful"), decision];
    let l = layout(&info(false, None), &blocks);
    let t = texts(&l, 0).join("\n");
    for want in ["wise", "careful", "Decision: Use the 9-slice", "Who: Ana", "When: 2026-11-12", "Why: cheaper"] {
        assert!(t.contains(want), "{want} in {t}");
    }
}

#[test]
fn text_looks_become_style_runs() {
    let mut b = Block::new(BlockId(1), BlockKind::Paragraph);
    b.text = vec![Run::plain("a "), Run { text: "bold".into(), bold: true, ..Run::default() }, Run::plain(" b")];
    let l = layout(&info(false, None), &[b]);
    let data = l.pages[0].iter().find_map(|i| match i {
        Item::Text { data, .. } if data.characters == "a bold b" => Some(data.clone()),
        _ => None,
    });
    let data = data.expect("the paragraph");
    assert_eq!(data.runs.len(), 1);
    assert_eq!((data.runs[0].start, data.runs[0].end), (2, 6));
    assert_eq!(data.runs[0].style.font_style, "Bold");
}

#[test]
fn deep_nesting_and_huge_documents_stay_bounded() {
    let mut inner = block(0, BlockKind::Bulleted, "deep");
    for i in 1..60 {
        let mut outer = block(i, BlockKind::Bulleted, "level");
        outer.children.push(inner);
        inner = outer;
    }
    let l = layout(&info(false, None), &[inner]);
    assert!(!l.pages.is_empty());
    let many: Vec<Block> = (0..3000).map(|i| block(i, BlockKind::Divider, "")).collect();
    let l = layout(&info(false, None), &many);
    assert!(l.pages.len() <= 300);
}

#[test]
fn a_page_break_inside_a_text_never_starts_the_next_page_with_a_blank() {
    if !has_fonts() {
        return;
    }
    let lines: Vec<String> = (0..120).map(|i| format!("line {i}")).collect();
    let l = layout(&info(false, None), &[block(1, BlockKind::Paragraph, &lines.join("\n"))]);
    assert!(l.pages.len() >= 2);
    let mut got = Vec::new();
    for page in &l.pages {
        for item in page {
            if let Item::Text { data, .. } = item
                && data.characters.starts_with("line")
            {
                assert!(!data.characters.starts_with(['\n', ' ']), "{:?}", data.characters.get(..10));
                got.extend(data.characters.lines().map(str::to_string));
            }
        }
    }
    assert_eq!(got.iter().filter(|s| !s.is_empty()).cloned().collect::<Vec<_>>(), lines);
}

#[test]
fn a_short_paragraph_at_the_bottom_moves_whole_instead_of_leaving_one_line() {
    if !has_fonts() {
        return;
    }
    // Fill page 1 with one-line paragraphs, then a three-line one: wherever the page ends, no page
    // holds exactly one of its lines.
    for filler in 30..42 {
        let mut blocks: Vec<Block> = (0..filler).map(|i| block(i, BlockKind::Paragraph, "One line.")).collect();
        blocks.push(block(999, BlockKind::Paragraph, "first\nsecond\nthird"));
        let l = layout(&info(false, None), &blocks);
        for page in &l.pages {
            for item in page {
                if let Item::Text { data, .. } = item {
                    let c = &data.characters;
                    assert!(!(["first", "second", "third"].contains(&c.as_str())), "filler {filler}: a lone line {c:?}");
                }
            }
        }
    }
}

#[test]
fn long_list_numbers_stay_on_one_line() {
    if !has_fonts() {
        return;
    }
    let blocks: Vec<Block> = (0..12).map(|i| block(i, BlockKind::Numbered, "item")).collect();
    let l = layout(&info(false, None), &blocks);
    for item in &l.pages[0] {
        if let Item::Text { data, w, .. } = item
            && data.characters == "12."
        {
            let lay = figcraft_text::layout_cached(data, Some(*w), None);
            assert_eq!(lay.lines.len(), 1, "12. fits on one line");
            return;
        }
    }
    panic!("no marker 12.");
}
