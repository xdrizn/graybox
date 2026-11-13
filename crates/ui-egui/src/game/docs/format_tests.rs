//! Formatting words inside a block (G13.2): the keys, the bar over selected words, links, and how the
//! looks are drawn.

use egui::{Key, Modifiers};
use figcraft_doc::docs::Run;

use super::blocks_tests::{blocks, caret_in, click, key_with, open, settle, type_text, words};
use crate::layers_drag_tests::Ui;

/// Select the last `n` characters of the block showing `s`.
fn select_end(u: &mut Ui, s: &str, n: usize) {
    caret_in(u, s, n);
    key_with(u, Key::End, Modifiers::SHIFT);
}

fn runs(u: &Ui, doc: u64) -> Vec<Run> {
    blocks(u, doc).first().map(|b| b.text.clone()).unwrap_or_default()
}

fn looks(u: &Ui, doc: u64) -> Vec<(String, bool, bool, bool)> {
    runs(u, doc).into_iter().map(|r| (r.text, r.bold, r.italic, r.underline)).collect()
}

fn l(t: &str, b: bool, i: bool, u: bool) -> (String, bool, bool, bool) {
    (t.to_string(), b, i, u)
}

#[test]
fn ctrl_b_makes_the_selected_words_bold_and_again_takes_it_off() {
    let (mut u, doc) = open(&[("paragraph", "Hello world")]);
    select_end(&mut u, "Hello world", 5);
    key_with(&mut u, Key::B, Modifiers::COMMAND);
    assert_eq!(looks(&u, doc), [l("Hello ", false, false, false), l("world", true, false, false)]);
    // The words stay selected: the same key takes it off.
    key_with(&mut u, Key::B, Modifiers::COMMAND);
    assert_eq!(looks(&u, doc), [l("Hello world", false, false, false)]);
}

#[test]
fn ctrl_i_and_ctrl_u_stack_and_ctrl_u_does_not_delete_the_line() {
    let (mut u, doc) = open(&[("paragraph", "Hello world")]);
    select_end(&mut u, "Hello world", 5);
    key_with(&mut u, Key::I, Modifiers::COMMAND);
    key_with(&mut u, Key::U, Modifiers::COMMAND);
    assert_eq!(looks(&u, doc), [l("Hello ", false, false, false), l("world", false, true, true)]);
    // With nothing selected the keys do nothing at all (egui would delete the line on Ctrl+U).
    key_with(&mut u, Key::End, Modifiers::NONE);
    key_with(&mut u, Key::U, Modifiers::COMMAND);
    settle(&mut u);
    assert_eq!(words(&u, doc), ["Hello world"]);
}

#[test]
fn ctrl_e_makes_code_and_ctrl_shift_x_strikes_without_cutting() {
    let (mut u, doc) = open(&[("paragraph", "run fast")]);
    select_end(&mut u, "run fast", 4);
    key_with(&mut u, Key::E, Modifiers::COMMAND);
    assert!(runs(&u, doc).iter().any(|r| r.text == "fast" && r.code));
    // The window sends Cut along with Ctrl+Shift+X, as it does for Ctrl+X.
    let m = Modifiers::COMMAND | Modifiers::SHIFT;
    u.frame_with(vec![egui::Event::Key { key: Key::X, physical_key: None, pressed: true, repeat: false, modifiers: m }, egui::Event::Cut], m);
    u.frame_with(vec![egui::Event::Key { key: Key::X, physical_key: None, pressed: false, repeat: false, modifiers: m }], m);
    settle(&mut u);
    assert_eq!(words(&u, doc), ["run fast"]);
    assert!(runs(&u, doc).iter().any(|r| r.text == "fast" && r.code && r.strike));
}

#[test]
fn the_bar_shows_over_selected_words_and_its_buttons_format_them() {
    let (mut u, doc) = open(&[("paragraph", "Hello world")]);
    assert!(super::inline::last_button("bold").is_none());
    select_end(&mut u, "Hello world", 5);
    u.frame(vec![]);
    let bold = super::inline::last_button("bold").expect("the bar shows over a selection");
    click(&mut u, bold.center());
    assert_eq!(looks(&u, doc), [l("Hello ", false, false, false), l("world", true, false, false)]);
    // The selection is kept, so a second button acts on the same words.
    u.frame(vec![]);
    let italic = super::inline::last_button("italic").expect("the bar is still there");
    click(&mut u, italic.center());
    assert_eq!(looks(&u, doc), [l("Hello ", false, false, false), l("world", true, true, false)]);
}

#[test]
fn the_bar_goes_when_the_selection_does() {
    let (mut u, _doc) = open(&[("paragraph", "Hello world")]);
    select_end(&mut u, "Hello world", 5);
    u.frame(vec![]);
    assert!(super::inline::shown());
    key_with(&mut u, Key::ArrowLeft, Modifiers::NONE);
    u.frame(vec![]);
    assert!(!super::inline::shown());
}

#[test]
fn ctrl_k_asks_for_an_address_and_enter_links_the_words() {
    let (mut u, doc) = open(&[("paragraph", "see the site")]);
    select_end(&mut u, "see the site", 4);
    key_with(&mut u, Key::K, Modifiers::COMMAND);
    type_text(&mut u, "https://example.com");
    key_with(&mut u, Key::Enter, Modifiers::NONE);
    let r = runs(&u, doc);
    assert!(r.iter().any(|r| r.text == "site" && r.link.as_deref() == Some("https://example.com")), "{r:?}");
    assert!(r.iter().any(|r| r.text == "see the " && r.link.is_none()));
    // The words did not change, and the keyboard is back in the block.
    assert_eq!(words(&u, doc), ["see the site"]);
}

#[test]
fn typing_right_after_bold_words_keeps_writing_bold() {
    let (mut u, doc) = open(&[("paragraph", "Hello world")]);
    select_end(&mut u, "Hello world", 5);
    key_with(&mut u, Key::B, Modifiers::COMMAND);
    key_with(&mut u, Key::End, Modifiers::NONE);
    type_text(&mut u, "s");
    settle(&mut u);
    assert_eq!(looks(&u, doc), [l("Hello ", false, false, false), l("worlds", true, false, false)]);
}

#[test]
fn bold_words_are_drawn_in_the_bold_font() {
    let (mut u, _doc) = open(&[("paragraph", "Hello world")]);
    select_end(&mut u, "Hello world", 5);
    key_with(&mut u, Key::B, Modifiers::COMMAND);
    let fonts = super::inline::last_fonts();
    assert!(fonts.iter().any(|f| matches!(&f.family, egui::FontFamily::Name(n) if &**n == "semibold")), "{fonts:?}");
    assert!(fonts.iter().any(|f| f.family == egui::FontFamily::Proportional));
}

#[test]
fn opening_the_colours_keeps_the_words_selected_and_the_bar_up() {
    let (mut u, doc) = open(&[("paragraph", "Hello world")]);
    select_end(&mut u, "Hello world", 5);
    u.frame(vec![]);
    let colour = super::inline::last_button("colour").expect("the bar shows over a selection");
    click(&mut u, colour.center());
    // The pointer leaves the bar: it stays, with the words still selected.
    u.frame(vec![egui::Event::PointerMoved(egui::pos2(5.0, 5.0))]);
    u.frame(vec![]);
    assert!(super::inline::last_button("swatches").is_some(), "the colours stay open");
    key_with(&mut u, Key::B, Modifiers::COMMAND);
    assert_eq!(looks(&u, doc), [l("Hello ", false, false, false), l("world", true, false, false)]);
}

#[test]
fn ctrl_shift_u_and_k_never_delete_the_words() {
    let (mut u, doc) = open(&[("paragraph", "Hello world")]);
    caret_in(&mut u, "Hello world", 5);
    key_with(&mut u, Key::U, Modifiers::COMMAND | Modifiers::SHIFT);
    key_with(&mut u, Key::K, Modifiers::COMMAND | Modifiers::SHIFT);
    settle(&mut u);
    assert_eq!(words(&u, doc), ["Hello world"]);
}

#[test]
fn only_web_and_mail_addresses_open() {
    for ok in ["https://example.com", "HTTP://example.com/a", "mailto:team@example.com"] {
        assert!(super::inline::opens(ok), "{ok}");
    }
    for no in ["file:///C:/x.exe", "javascript:alert(1)", r"C:\x.exe", "ms-settings:", "example.com"] {
        assert!(!super::inline::opens(no), "{no}");
    }
}
