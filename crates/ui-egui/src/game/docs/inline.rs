//! Formatting words inside a block (G13.2), as in Notion: a bar over selected words (bold, italic,
//! underline, strike, code, link, colour), the keys (Ctrl+B, I, U, E, K, Ctrl+Shift+X), and the
//! words drawn with their looks. Ctrl+click on linked words opens the address.

use egui::text::{CCursor, LayoutJob, TextFormat};
use egui::{Align2, Color32, CornerRadius, FontId, Id, Key, Modifiers, Pos2, Rect, Sense, Stroke, Ui, pos2, vec2};
use figcraft_doc::docs::{Block, BlockId, BlockKind, DocId, Mark, Run, has_mark};
use figcraft_engine::keymap::Platform;
use serde_json::json;

use super::blocks::{parse_hex, swatches};
use super::editor::commit;
use super::state::{State, Target};
use crate::FigApp;
use crate::i18n::tr;
use crate::icons::{self, Icon};
use crate::theme::{Tokens, regular, semibold};

/// The bar of block `block` while it is up: the words it formats (characters `from..to`), where it
/// was drawn, the address being typed when the link field is open, and whether the colours show.
#[derive(Clone, Debug)]
pub(super) struct Bar {
    pub doc: DocId,
    pub block: BlockId,
    pub from: usize,
    pub to: usize,
    pub rect: Option<Rect>,
    pub link: Option<String>,
    /// The link field takes the keyboard next frame.
    pub link_start: bool,
    pub colours: bool,
}

/// What a key asked for.
pub(super) enum Do {
    Mark(Mark),
    Link,
}

#[cfg(test)]
thread_local! {
    static BUTTONS: std::cell::RefCell<Vec<(&'static str, Rect)>> = const { std::cell::RefCell::new(Vec::new()) };
    static FONTS: std::cell::RefCell<Vec<FontId>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Where the bar's button `name` was last drawn (tests).
#[cfg(test)]
pub(super) fn last_button(name: &str) -> Option<Rect> {
    BUTTONS.with(|b| b.borrow().iter().find(|(n, _)| *n == name).map(|(_, r)| *r))
}

/// Whether the bar was drawn in the last frame (tests).
#[cfg(test)]
pub(super) fn shown() -> bool {
    BUTTONS.with(|b| !b.borrow().is_empty())
}

/// The fonts of the words laid out last (tests).
#[cfg(test)]
pub(super) fn last_fonts() -> Vec<FontId> {
    FONTS.with(|f| f.borrow().clone())
}

/// Forget what the last frame drew (tests): called before the blocks are drawn.
#[cfg(test)]
pub(super) fn forget() {
    BUTTONS.with(|b| b.borrow_mut().clear());
}

/// `runs` (what the file has) with the words typed since (`text`) put in, each new character taking
/// the look of the run it lands in.
fn runs_for(runs: &[Run], text: &str) -> Vec<Run> {
    if runs.iter().map(|r| r.text.as_str()).collect::<String>() == text {
        return runs.to_vec();
    }
    let mut b = Block::new(BlockId(0), BlockKind::Paragraph);
    b.text = runs.to_vec();
    b.set_plain_text(text);
    b.text
}

/// How a block's words are drawn: its font and colour, struck through when it is a task done, and
/// the colours of links and of code.
#[derive(Clone)]
pub(super) struct Ink {
    pub font: FontId,
    pub color: Color32,
    pub strike: bool,
    pub link: Color32,
    pub code: Color32,
}

fn format_of(r: &Run, ink: &Ink) -> TextFormat {
    let size = ink.font.size;
    let font_id = if r.code {
        FontId::monospace(size * 0.9)
    } else if r.bold {
        semibold(size)
    } else {
        ink.font.clone()
    };
    // A task done is dim all through, whatever colour its words have.
    let own = r.color.as_deref().and_then(parse_hex).unwrap_or(if r.link.is_some() { ink.link } else { ink.color });
    let color = if ink.strike { ink.color } else { own };
    let line = Stroke::new(1.0, color);
    let background = r.highlight.as_deref().and_then(parse_hex).unwrap_or(if r.code { ink.code } else { Color32::TRANSPARENT });
    TextFormat {
        font_id,
        color,
        italics: r.italic,
        underline: if r.underline || r.link.is_some() { line } else { Stroke::NONE },
        strikethrough: if r.strike || ink.strike { line } else { Stroke::NONE },
        background,
        ..TextFormat::default()
    }
}

/// The layout of `text` with the looks of `runs`, wrapped at `wrap`.
pub(super) fn job(runs: &[Run], text: &str, ink: &Ink, wrap: f32) -> LayoutJob {
    let runs = runs_for(runs, text);
    let plain = TextFormat {
        font_id: ink.font.clone(),
        color: ink.color,
        strikethrough: if ink.strike { Stroke::new(1.0, ink.color) } else { Stroke::NONE },
        ..TextFormat::default()
    };
    let mut job = if text.is_empty() || runs.iter().map(|r| r.text.as_str()).collect::<String>() != text {
        LayoutJob::single_section(text.to_owned(), plain)
    } else {
        let mut job = LayoutJob::default();
        for r in runs.iter().filter(|r| !r.text.is_empty()) {
            job.append(&r.text, 0.0, format_of(r, ink));
        }
        job
    };
    #[cfg(test)]
    FONTS.with(|f| *f.borrow_mut() = job.sections.iter().map(|s| s.format.font_id.clone()).collect());
    job.wrap.max_width = wrap;
    job
}

/// Read the formatting keys of a focused block before its field sees them. Ctrl+U and Ctrl+K are
/// always taken (egui would delete to the line's start or end); they act only on selected words.
pub(super) fn keys(ui: &mut Ui) -> Option<Do> {
    ui.input_mut(|i| {
        if i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::X) {
            // Shift does not stop egui from reading Ctrl+X as Cut: the words stay.
            i.events.retain(|e| !matches!(e, egui::Event::Cut));
            return Some(Do::Mark(Mark::Strike));
        }
        let picks = [(Key::B, Mark::Bold), (Key::I, Mark::Italic), (Key::U, Mark::Underline), (Key::E, Mark::Code)];
        for (key, mark) in picks {
            // With Shift too: egui reads Ctrl+Shift+U and K as deleting words.
            if i.consume_key(Modifiers::COMMAND, key) {
                return Some(Do::Mark(mark));
            }
        }
        i.consume_key(Modifiers::COMMAND, Key::K).then_some(Do::Link)
    })
}

/// The selected characters of a field, when some are.
fn selected(range: Option<egui::text::CCursorRange>) -> Option<(usize, usize)> {
    let r = range?;
    let (a, b) = (r.primary.index.0.min(r.secondary.index.0), r.primary.index.0.max(r.secondary.index.0));
    (b > a).then_some((a, b))
}

/// Do what a key asked for on characters `from..to` of `block` (whose words are `runs`).
fn act(app: &mut FigApp, st: &mut State, doc: DocId, block: BlockId, runs: &[Run], (from, to): (usize, usize), what: Do) {
    match what {
        Do::Mark(m) => format(app, st, doc, block, (from, to), &m),
        Do::Link => {
            let current = link_at(runs, from).unwrap_or_default();
            st.bar = Some(Bar { doc, block, from, to, rect: None, link: Some(current), link_start: true, colours: false });
        }
    }
}

/// The address of the linked word at character `at`, if it is one.
fn link_at(runs: &[Run], at: usize) -> Option<String> {
    let mut pos = 0;
    for r in runs {
        let len = r.text.chars().count();
        if at >= pos && at < pos + len {
            return r.link.clone();
        }
        pos += len;
    }
    None
}

/// Put `mark` on characters `from..to` of `block`, then give the block the keyboard back with the
/// same words selected.
fn format(app: &mut FigApp, st: &mut State, doc: DocId, block: BlockId, (from, to): (usize, usize), mark: &Mark) {
    // The words typed so far go out first, so the range counts in them.
    if let Some(p) = st.pending.take() {
        commit(app, &p);
    }
    let (name, value) = match mark {
        Mark::Bold => ("bold", None),
        Mark::Italic => ("italic", None),
        Mark::Underline => ("underline", None),
        Mark::Strike => ("strike", None),
        Mark::Code => ("code", None),
        Mark::Color(c) => ("color", c.clone()),
        Mark::Highlight(c) => ("highlight", c.clone()),
        Mark::Link(l) => ("link", l.clone()),
    };
    app.run("doc.block.format", json!({"doc": doc.0, "id": block.0, "from": from, "to": to, "mark": name, "value": value}));
    st.focus = Some(Target::Block(block));
    st.select = Some((from, to));
}

/// `Ctrl+B` or `⌘B`, as this platform writes it.
fn chord(keys: &str) -> String {
    match crate::shortcuts::platform() {
        Platform::Mac => format!("⌘{}", keys.replace("Shift+", "⇧")),
        Platform::Other => format!("Ctrl+{keys}"),
    }
}

/// A button showing a letter in its look (bold B, slanted I, underlined U, struck S).
fn letter(ui: &mut Ui, t: &Tokens, name: &'static str, glyph: &str, look: TextFormat, active: bool, tip: &str) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::click());
    #[cfg(test)]
    BUTTONS.with(|b| b.borrow_mut().push((name, r)));
    let _ = name;
    let bg = if active {
        t.selected_row
    } else if resp.hovered() {
        t.hover
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(r, CornerRadius::same(5), bg);
    let color = if active { t.accent_text } else { t.icon };
    let mut job = LayoutJob::single_section(glyph.to_owned(), TextFormat { color, ..look });
    job.halign = egui::Align::Center;
    let galley = ui.fonts_mut(|f| f.layout_job(job));
    let at = pos2(r.center().x, r.center().y - galley.size().y / 2.0);
    ui.painter().galley(at, galley, color);
    let resp = crate::a11y::Tip::tip(resp, tip);
    crate::a11y::toggle(&resp, tip, active);
    resp
}

/// An icon button of the bar.
fn icon(ui: &mut Ui, name: &'static str, glyph: Icon, active: bool, tip: &str) -> bool {
    let resp = icons::button(ui, glyph, 28.0, active, tip);
    #[cfg(test)]
    BUTTONS.with(|b| b.borrow_mut().push((name, resp.rect)));
    let _ = name;
    resp.clicked()
}

/// A block's field as it was drawn this frame.
pub(super) struct Field<'a> {
    /// The words in the field, and their looks in the file.
    pub text: &'a str,
    pub runs: &'a [Run],
    /// The characters selected before this frame's keys, and a formatting key pressed.
    pub before: Option<(usize, usize)>,
    pub key: Option<Do>,
    /// The selection now, whether the field has the keyboard, and where its words were drawn.
    pub sel: Option<egui::text::CCursorRange>,
    pub focused: bool,
    pub galley: &'a egui::Galley,
    pub galley_pos: Pos2,
}

/// A formatting key's work, then the bar over the selected words of `block`, while there are some or
/// while the pointer or the keyboard is on the bar; and Ctrl+click on a link.
pub(super) fn show(app: &mut FigApp, ui: &Ui, st: &mut State, t: &Tokens, doc: DocId, block: BlockId, f: Field) {
    let runs = runs_for(f.runs, f.text);
    if let (Some(what), Some(sel)) = (f.key, f.before.filter(|(a, b)| b > a)) {
        act(app, st, doc, block, &runs, sel, what);
    }
    let (sel, focused, galley, galley_pos) = (selected(f.sel), f.focused, f.galley, f.galley_pos);
    open_link(ui, &runs, galley, galley_pos);
    let link_id = Id::unique(("gbx_doc_link_edit", doc.0, block.0));
    let link_focused = ui.memory(|m| m.has_focus(link_id));
    let pointer_down = ui.input(|i| i.pointer.primary_down());
    let mine = st.bar.as_ref().filter(|b| b.doc == doc && b.block == block).cloned();
    let on_bar = mine.as_ref().and_then(|b| b.rect).is_some_and(|r| ui.input(|i| i.pointer.interact_pos()).is_some_and(|p| r.contains(p)));
    let mut bar = match (focused, sel, mine) {
        // Selected words: the bar comes up once the button is let go, and follows the selection.
        (true, Some((from, to)), Some(b)) => Bar { from, to, ..b },
        (true, Some((from, to)), None) if !pointer_down => Bar { doc, block, from, to, rect: None, link: None, link_start: false, colours: false },
        (_, _, Some(b)) if b.link_start || link_focused || (on_bar && !focused) => b,
        (_, _, Some(_)) => {
            st.bar = None;
            return;
        }
        _ => return,
    };
    let first = galley.pos_from_cursor(CCursor::new(bar.from));
    let anchor = galley_pos + first.left_top().to_vec2() + vec2(-4.0, -6.0);
    let mut picked: Option<Mark> = None;
    // The keyboard goes back to the words, still selected.
    let mut back = false;
    let has = |m: &Mark| has_mark(&runs, bar.from, bar.to, m);
    let area = egui::Area::new(Id::unique(("gbx_doc_bar", doc.0, block.0)))
        .order(egui::Order::Foreground)
        .fixed_pos(anchor)
        .pivot(Align2::LEFT_BOTTOM)
        .constrain(true)
        .show(ui.ctx(), |ui| {
            crate::right::menu_style(ui.style_mut());
            egui::Frame::menu(ui.style()).inner_margin(egui::Margin::same(3)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = vec2(1.0, 0.0);
                    let size = 15.0;
                    let base = TextFormat { font_id: regular(size), ..TextFormat::default() };
                    let bold = (format!("{}  {}", tr("Bold"), chord("B")), Mark::Bold);
                    if letter(ui, t, "bold", "B", TextFormat { font_id: semibold(size), ..base.clone() }, has(&bold.1), &bold.0).clicked() {
                        picked = Some(bold.1);
                    }
                    let italic = format!("{}  {}", tr("Italic"), chord("I"));
                    if letter(ui, t, "italic", "I", TextFormat { italics: true, ..base.clone() }, has(&Mark::Italic), &italic).clicked() {
                        picked = Some(Mark::Italic);
                    }
                    let under = format!("{}  {}", tr("Underline"), chord("U"));
                    let line = Stroke::new(1.0, t.icon);
                    if letter(ui, t, "underline", "U", TextFormat { underline: line, ..base.clone() }, has(&Mark::Underline), &under).clicked() {
                        picked = Some(Mark::Underline);
                    }
                    let strike = format!("{}  {}", tr("Strikethrough"), chord("Shift+X"));
                    if letter(ui, t, "strike", "S", TextFormat { strikethrough: line, ..base.clone() }, has(&Mark::Strike), &strike).clicked() {
                        picked = Some(Mark::Strike);
                    }
                    let code = format!("{}  {}", tr("Code"), chord("E"));
                    if icon(ui, "code", Icon::Code, has(&Mark::Code), &code) {
                        picked = Some(Mark::Code);
                    }
                    let link = format!("{}  {}", tr("Link"), chord("K"));
                    if icon(ui, "link", Icon::Link, bar.link.is_some() || link_at(&runs, bar.from).is_some(), &link) {
                        bar.link = if bar.link.is_some() { None } else { Some(link_at(&runs, bar.from).unwrap_or_default()) };
                        bar.link_start = bar.link.is_some();
                        bar.colours = false;
                        back = bar.link.is_none();
                    }
                    let colour = tr("Colour");
                    let shown = runs_colour(&runs, bar.from).unwrap_or(t.icon);
                    let r = letter(ui, t, "colour", "A", base.clone(), bar.colours, colour);
                    let bottom = r.rect.center_bottom() - vec2(0.0, 6.0);
                    ui.painter().line_segment([bottom - vec2(6.0, 0.0), bottom + vec2(6.0, 0.0)], Stroke::new(2.0, shown));
                    if r.clicked() {
                        bar.colours = !bar.colours;
                        bar.link = None;
                        back = true;
                    }
                });
                if let Some(address) = &mut bar.link {
                    let edit = egui::TextEdit::singleline(address)
                        .id(link_id)
                        .desired_width(220.0)
                        .hint_text(egui::RichText::new(tr("Paste a link")).color(t.text_tertiary));
                    let resp = ui.add(edit);
                    crate::a11y::text(&resp, tr("Link"), address);
                    if bar.link_start {
                        resp.request_focus();
                        bar.link_start = false;
                    }
                    if resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                        let a = address.trim();
                        picked = Some(Mark::Link((!a.is_empty()).then(|| a.to_string())));
                    } else if resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Escape)) {
                        bar.link = None;
                        back = true;
                    }
                }
                if bar.colours {
                    #[cfg(test)]
                    BUTTONS.with(|b| b.borrow_mut().push(("swatches", ui.min_rect())));
                    crate::widgets::label(ui, tr("Colour"), t.text_secondary);
                    if let Some(c) = swatches(ui, t, false) {
                        picked = Some(Mark::Color(c.as_str().map(str::to_string)));
                    }
                    crate::widgets::label(ui, tr("Highlight"), t.text_secondary);
                    if let Some(c) = swatches(ui, t, true) {
                        picked = Some(Mark::Highlight(c.as_str().map(str::to_string)));
                    }
                }
            });
        });
    bar.rect = Some(area.response.rect);
    let range = (bar.from, bar.to);
    // A pick closes the link field and the colours.
    let open = picked.is_none();
    st.bar = Some(Bar { link: bar.link.clone().filter(|_| open), colours: bar.colours && open, ..bar });
    if let Some(m) = picked {
        format(app, st, doc, block, range, &m);
    } else if back {
        st.focus = Some(Target::Block(block));
        st.select = Some(range);
    }
}

/// The colour of the word at character `at`, if it has one.
fn runs_colour(runs: &[Run], at: usize) -> Option<Color32> {
    let mut pos = 0;
    for r in runs {
        let len = r.text.chars().count();
        if at >= pos && at < pos + len {
            return r.color.as_deref().and_then(parse_hex);
        }
        pos += len;
    }
    None
}

/// Ctrl+click (⌘ on a Mac) on linked words opens their address; the pointer says so while Ctrl is held.
fn open_link(ui: &Ui, runs: &[Run], galley: &egui::Galley, galley_pos: Pos2) {
    let (pos, command, clicked) = ui.input(|i| (i.pointer.hover_pos(), i.modifiers.command, i.pointer.primary_clicked()));
    let Some(p) = pos.filter(|_| command) else { return };
    let local = p - galley_pos;
    if !galley.rect.expand(2.0).contains(local.to_pos2()) {
        return;
    }
    let at = galley.cursor_from_pos(local).index.0;
    // The character under the pointer: the one before the cursor when the pointer is past its middle.
    let Some(url) = link_at(runs, at).or_else(|| at.checked_sub(1).and_then(|a| link_at(runs, a))).filter(|u| opens(u)) else { return };
    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    if clicked {
        ui.ctx().open_url(egui::OpenUrl::new_tab(url));
    }
}

/// Whether an address may be opened: a web page or an e-mail, never a file or a program (a document
/// can come from someone else).
pub(super) fn opens(url: &str) -> bool {
    let lower = url.trim_start().to_ascii_lowercase();
    ["https://", "http://", "mailto:"].iter().any(|s| lower.starts_with(s))
}
