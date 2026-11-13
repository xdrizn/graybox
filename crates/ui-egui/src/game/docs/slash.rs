//! The `/` menu (G13.2): typing `/` at the start of a block or after a space opens a list of blocks
//! under the cursor. The letters typed after the `/` filter it (in English or in the app's language,
//! accents ignored), the arrows choose, Enter or a click picks, Escape closes and leaves the words.
//! An empty block turns into the kind picked; a block with words gets the new block below it.

use egui::{Id, Key, Modifiers, Pos2, Rect, Ui};
use figcraft_doc::docs::{BlockId, DocId};
use serde_json::json;

use super::blocks::{add_block, place_after};
use super::editor::commit;
use super::state::{State, Target};
use crate::FigApp;
use crate::i18n::tr;
use crate::theme::{Tokens, regular};

/// The longest filter the menu reads: past it, the `/` was just a slash.
const MAX_QUERY: usize = 24;
/// How wide the menu is.
const MENU_W: f32 = 240.0;

/// The `/` menu, open in one block.
#[derive(Clone, Debug)]
pub(super) struct Slash {
    pub block: BlockId,
    /// Where the `/` is, in characters.
    pub at: usize,
    /// The highlighted match.
    pub sel: usize,
    /// Where the menu was drawn last frame: a press on it keeps the menu open while the field lets
    /// the keyboard go.
    pub rect: Option<Rect>,
}

/// A key the open menu took from the block.
#[derive(Clone, Copy, Debug)]
pub(super) enum Move {
    Up,
    Down,
    Pick,
    Close,
}

/// What the menu lists: (label, kind, other words that find it).
fn items() -> [(&'static str, &'static str, &'static str); 14] {
    [
        (tr("Text"), "paragraph", "text paragraph plain"),
        (tr("Heading 1"), "heading1", "h1 heading title"),
        (tr("Heading 2"), "heading2", "h2 heading subtitle"),
        (tr("Heading 3"), "heading3", "h3 heading"),
        (tr("Bulleted list"), "bulleted", "bullet list ul"),
        (tr("Numbered list"), "numbered", "number list ol"),
        (tr("To-do list"), "checklist", "todo to-do task checkbox"),
        (tr("Toggle list"), "toggle", "toggle collapse fold"),
        (tr("Callout"), "callout", "callout note tip"),
        (tr("Quote"), "quote", "quote citation"),
        (tr("Divider"), "divider", "divider line rule hr separator"),
        (tr("Decision"), "decision", "decision decided"),
        (tr("Link"), "link", "link url web"),
        (tr("Signature"), "signature", "signature sign"),
    ]
}

/// `s` in lower case without the accents of Portuguese and other Latin languages.
fn fold(s: &str) -> String {
    s.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            c => c,
        })
        .collect()
}

/// The items `query` finds, as indexes into [`items`], in the menu's order.
fn matches(query: &str) -> Vec<usize> {
    let q = fold(query.trim());
    items()
        .iter()
        .enumerate()
        .filter(|(_, (label, kind, words))| q.is_empty() || fold(label).contains(&q) || words.contains(q.as_str()) || kind.contains(q.as_str()))
        .map(|(i, _)| i)
        .collect()
}

/// Where a `/` that typing just added sits, when it opens the menu: at the start of the block or
/// after a space, with the cursor right after it.
pub(super) fn opened(before: &str, text: &str, caret: Option<usize>) -> Option<usize> {
    if text.chars().count() != before.chars().count() + 1 {
        return None;
    }
    let at = caret?.checked_sub(1)?;
    let mut back = text.chars().skip(at.saturating_sub(1));
    let starts = at == 0 || back.next().is_some_and(char::is_whitespace);
    let slash = text.chars().nth(at) == Some('/');
    (slash && starts).then_some(at)
}

/// The letters between the `/` at `at` and the cursor, or `None` when the menu should close (the
/// `/` is gone, the cursor went before it, a line break or too many letters).
fn query(text: &str, at: usize, caret: usize) -> Option<String> {
    if caret <= at || text.chars().nth(at) != Some('/') {
        return None;
    }
    let q: String = text.chars().skip(at + 1).take(caret - at - 1).collect();
    (!q.contains('\n') && q.chars().count() <= MAX_QUERY).then_some(q)
}

/// Whether the menu of a block showing `text`, its `/` at `at` and the cursor at `caret`, lists
/// something.
pub(super) fn lists(text: &str, at: usize, caret: Option<usize>) -> bool {
    caret.and_then(|c| query(text, at, c)).is_some_and(|q| !matches(&q).is_empty())
}

/// The keys the open menu takes before the block's field sees them: Escape, and the arrows and
/// Enter while it `lists` something (else Enter is an ordinary Enter).
pub(super) fn keys(ui: &mut Ui, lists: bool) -> Option<Move> {
    ui.input_mut(|i| {
        if i.modifiers.shift {
            return None;
        }
        if i.consume_key(Modifiers::NONE, Key::Escape) {
            return Some(Move::Close);
        }
        if !lists {
            return None;
        }
        if i.consume_key(Modifiers::NONE, Key::ArrowDown) {
            Some(Move::Down)
        } else if i.consume_key(Modifiers::NONE, Key::ArrowUp) {
            Some(Move::Up)
        } else if i.consume_key(Modifiers::NONE, Key::Enter) {
            Some(Move::Pick)
        } else {
            None
        }
    })
}

/// The menu of block `block`, under its `/` (`galley` drawn at `galley_pos`), when it is open; picks
/// what Enter or a click chose.
#[allow(clippy::too_many_arguments)]
pub(super) fn show(
    app: &mut FigApp,
    ui: &Ui,
    st: &mut State,
    t: &Tokens,
    doc: DocId,
    block: BlockId,
    text: &str,
    caret: Option<usize>,
    focused: bool,
    galley: &egui::Galley,
    galley_pos: Pos2,
    key: Option<Move>,
) {
    let Some(s) = st.slash.clone().filter(|s| s.block == block) else { return };
    let Some((end, q)) = caret.and_then(|c| query(text, s.at, c).map(|q| (c, q))) else {
        st.slash = None;
        return;
    };
    let found = matches(&q);
    let on_menu = s.rect.is_some_and(|r| ui.input(|i| i.pointer.interact_pos()).is_some_and(|p| r.contains(p)));
    // The keyboard went elsewhere, or a space after letters that find nothing: it was just a slash.
    if (!focused && !on_menu) || (found.is_empty() && q.ends_with(' ')) {
        st.slash = None;
        return;
    }
    let n = found.len();
    let mut sel = s.sel.min(n.saturating_sub(1));
    let mut picked = None;
    match key {
        Some(Move::Up) => sel = if sel == 0 { n.saturating_sub(1) } else { sel - 1 },
        Some(Move::Down) => sel = if n == 0 { 0 } else { (sel + 1) % n },
        Some(Move::Pick) => picked = found.get(sel).copied(),
        Some(Move::Close) => {
            st.slash = None;
            return;
        }
        None => {}
    }
    let moved = matches!(key, Some(Move::Up | Move::Down));
    let under = galley.pos_from_cursor(egui::text::CCursor::new(s.at));
    let anchor = galley_pos + under.left_bottom().to_vec2() + egui::vec2(-8.0, 6.0);
    let all = items();
    let area = egui::Area::new(Id::unique(("gbx_doc_slash", doc.0, block.0)))
        .order(egui::Order::Foreground)
        .fixed_pos(anchor)
        .constrain(true)
        .default_width(MENU_W)
        .show(ui.ctx(), |ui| {
            crate::right::menu_style(ui.style_mut());
            egui::Frame::menu(ui.style()).show(ui, |ui| {
                ui.set_min_width(MENU_W);
                ui.set_max_width(MENU_W);
                ui.label(egui::RichText::new(tr("Blocks")).font(regular(11.0)).color(t.text_secondary));
                if found.is_empty() {
                    ui.label(egui::RichText::new(tr("No results")).font(regular(12.0)).color(t.text_tertiary));
                }
                egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                    for (k, &i) in found.iter().enumerate() {
                        let Some((label, _, _)) = all.get(i) else { continue };
                        let resp = crate::right::menu_row_lit(ui, label, k == sel);
                        if k == sel && moved {
                            resp.scroll_to_me(None);
                        }
                        if resp.clicked() {
                            picked = Some(i);
                        }
                    }
                });
            });
        });
    st.slash = Some(Slash { sel, rect: Some(area.response.rect), ..s });
    if let Some((_, kind, _)) = picked.and_then(|i| all.get(i)) {
        pick(app, st, doc, block, text, s.at, end, kind);
    }
}

/// Take the `/` and its letters out of the block and make the kind picked: the block itself when
/// nothing else is written in it, else a new block below it.
#[allow(clippy::too_many_arguments)]
fn pick(app: &mut FigApp, st: &mut State, doc: DocId, block: BlockId, text: &str, at: usize, end: usize, kind: &str) {
    st.slash = None;
    // The words typed so far go out first, so the edit works on them.
    if let Some(p) = st.pending.take() {
        commit(app, &p);
    }
    let rest: String = text.chars().take(at).chain(text.chars().skip(end)).collect();
    if rest.trim().is_empty() {
        app.run("doc.block.turn_into", json!({"doc": doc.0, "id": block.0, "kind": kind, "text": ""}));
        if kind == "divider" {
            // A line, and writing goes on below it.
            let place = place_after(app, doc, block);
            add_block(app, st, doc, place, "paragraph");
        } else {
            st.focus = Some(Target::Block(block));
            st.caret = Some(0);
        }
        return;
    }
    app.run("doc.block.set", json!({"doc": doc.0, "id": block.0, "text": rest}));
    let place = place_after(app, doc, block);
    add_block(app, st, doc, place, kind);
    if kind == "divider" {
        st.focus = Some(Target::Block(block));
        st.caret = Some(at);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slash_opens_at_the_start_or_after_a_space_only() {
        assert_eq!(opened("", "/", Some(1)), Some(0));
        assert_eq!(opened("ab ", "ab /", Some(4)), Some(3));
        assert_eq!(opened("and", "and/", Some(4)), None);
        // Pasted, not typed.
        assert_eq!(opened("", "/x", Some(2)), None);
    }

    #[test]
    fn the_query_is_what_follows_the_slash_up_to_the_cursor() {
        assert_eq!(query("ab /hea", 3, 7).as_deref(), Some("hea"));
        assert_eq!(query("ab /hea", 3, 3), None);
        assert_eq!(query("ab xhea", 3, 7), None);
        assert_eq!(query("/a\nb", 0, 4), None);
    }

    #[test]
    fn filtering_ignores_case_and_accents() {
        assert_eq!(fold("Título Ação"), "titulo acao");
        let all = items();
        assert_eq!(matches("TODO").first().and_then(|i| all.get(*i)).map(|x| x.1), Some("checklist"));
        assert_eq!(matches("").len(), items().len());
        assert!(matches("zzz").is_empty());
    }
}
