//! The `/` menu (G13.2): typing `/` opens a list of blocks under the cursor, the letters after it
//! filter the list, the arrows choose, Enter or a click picks, Escape closes.

use egui::Key;
use figcraft_doc::docs::BlockKind;

use super::blocks_tests::{blocks, caret_in, click, key, open, settle, shapes, type_text, words};
use crate::live_tests::{has_text, text_center};

/// Put the keyboard in the only (empty) paragraph of a new document.
fn empty_paragraph() -> (crate::layers_drag_tests::Ui, u64) {
    let (mut u, doc) = open(&[("paragraph", "")]);
    let at = text_center(&shapes(&mut u), "Type “/” for blocks");
    click(&mut u, at);
    (u, doc)
}

#[test]
fn the_only_empty_paragraph_says_to_type_a_slash_and_others_only_with_the_keyboard() {
    let (mut u, _) = open(&[("paragraph", "")]);
    assert!(has_text(&shapes(&mut u), "Type “/” for blocks"));
    let (mut u, _) = open(&[("paragraph", "a"), ("paragraph", "")]);
    assert!(!has_text(&shapes(&mut u), "Type “/” for blocks"));
    caret_in(&mut u, "a", 0);
    key(&mut u, Key::Enter);
    assert!(has_text(&shapes(&mut u), "Type “/” for blocks"));
}

#[test]
fn a_slash_opens_the_menu_and_letters_filter_it() {
    let (mut u, _) = empty_paragraph();
    type_text(&mut u, "/");
    let sh = shapes(&mut u);
    assert!(has_text(&sh, "Heading 1") && has_text(&sh, "Bulleted list") && has_text(&sh, "Divider"));
    type_text(&mut u, "head");
    let sh = shapes(&mut u);
    assert!(has_text(&sh, "Heading 2"));
    assert!(!has_text(&sh, "Bulleted list"));
}

#[test]
fn enter_picks_the_first_match_and_turns_the_empty_block() {
    let (mut u, doc) = empty_paragraph();
    type_text(&mut u, "/todo");
    key(&mut u, Key::Enter);
    let b = blocks(&u, doc);
    assert_eq!(b.len(), 1);
    assert_eq!(b[0].kind, BlockKind::Checklist { checked: false });
    assert_eq!(b[0].plain_text(), "");
    // The menu is gone and the keyboard stays in the block.
    assert!(!has_text(&shapes(&mut u), "Heading 1"));
    type_text(&mut u, "Ship it");
    settle(&mut u);
    assert_eq!(words(&u, doc), ["Ship it"]);
}

#[test]
fn the_arrows_choose_another_match() {
    let (mut u, doc) = empty_paragraph();
    type_text(&mut u, "/heading");
    key(&mut u, Key::ArrowDown);
    key(&mut u, Key::Enter);
    assert_eq!(blocks(&u, doc)[0].kind, BlockKind::Heading { level: 2 });
}

#[test]
fn a_block_with_words_gets_the_new_block_below_and_keeps_its_words() {
    let (mut u, doc) = open(&[("paragraph", "Plan")]);
    caret_in(&mut u, "Plan", 0);
    type_text(&mut u, " /quote");
    key(&mut u, Key::Enter);
    let b = blocks(&u, doc);
    assert_eq!(words(&u, doc), ["Plan ", ""]);
    assert_eq!(b[0].kind, BlockKind::Paragraph);
    assert_eq!(b[1].kind, BlockKind::Quote);
}

#[test]
fn clicking_an_item_picks_it() {
    let (mut u, doc) = empty_paragraph();
    type_text(&mut u, "/");
    let at = text_center(&shapes(&mut u), "Callout");
    click(&mut u, at);
    assert_eq!(blocks(&u, doc)[0].kind.name(), "callout");
}

#[test]
fn escape_closes_the_menu_and_leaves_the_words() {
    let (mut u, doc) = empty_paragraph();
    type_text(&mut u, "/he");
    key(&mut u, Key::Escape);
    assert!(!has_text(&shapes(&mut u), "Heading 1"));
    settle(&mut u);
    assert_eq!(words(&u, doc), ["/he"]);
    assert_eq!(blocks(&u, doc)[0].kind, BlockKind::Paragraph);
}

#[test]
fn a_slash_inside_a_word_is_just_a_slash() {
    let (mut u, _) = open(&[("paragraph", "and")]);
    caret_in(&mut u, "and", 0);
    type_text(&mut u, "/or");
    assert!(!has_text(&shapes(&mut u), "Heading 1"));
}

#[test]
fn a_divider_picked_in_an_empty_block_lets_writing_go_on_below() {
    let (mut u, doc) = empty_paragraph();
    type_text(&mut u, "/divider");
    key(&mut u, Key::Enter);
    type_text(&mut u, "next");
    settle(&mut u);
    let b = blocks(&u, doc);
    assert_eq!(b[0].kind, BlockKind::Divider);
    assert_eq!(b.last().map(|b| b.plain_text()).as_deref(), Some("next"));
}

#[test]
fn enter_when_nothing_matches_is_an_ordinary_enter() {
    let (mut u, doc) = open(&[("paragraph", "Plan")]);
    caret_in(&mut u, "Plan", 0);
    type_text(&mut u, " /zzz");
    key(&mut u, Key::Enter);
    assert_eq!(words(&u, doc), ["Plan /zzz", ""]);
    assert!(!has_text(&shapes(&mut u), "No results"));
}
