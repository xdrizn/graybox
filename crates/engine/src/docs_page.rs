//! Page layout of a document version (G13.3): the one layout every consumer shares.
//!
//! A version's blocks are laid out on A4 pages as a flat list of items (text boxes and plain
//! shapes) with page positions. The PDF export, the canvas card and the web view all build from this
//! list, so what a client reads is exactly what Adriel sees. Text is measured with the real text
//! engine, so a long paragraph splits between pages by whole lines.

use figcraft_color::{Color, Paint};
use figcraft_doc::docs::{AutoSource, Block, BlockKind, Run, Table};
use figcraft_doc::{LineHeight, StyleRun, TextAlignH, TextAutoResize, TextData, TextDecoration, TextStyle};

/// A4 in points.
pub const PAGE_W: f64 = 595.0;
pub const PAGE_H: f64 = 842.0;
const MARGIN_X: f64 = 56.0;
const MARGIN_TOP: f64 = 60.0;
const BOTTOM: f64 = PAGE_H - 84.0;
const CONTENT_W: f64 = PAGE_W - 2.0 * MARGIN_X;
const BODY: f64 = 11.0;
/// Line height of body text, in percent.
const LINE: f64 = 150.0;
/// A document longer than this is cut off (a guard against runaway files).
const MAX_PAGES: usize = 300;
/// Nesting deeper than this is not laid out further.
const MAX_DEPTH: usize = 12;

const INK: Color = Color::from_rgb8_const(0x1F, 0x24, 0x30);
const MUTED: Color = Color::from_rgb8_const(0x6B, 0x72, 0x80);
const RULE: Color = Color::from_rgb8_const(0xD9, 0xDC, 0xE3);
const ACCENT: Color = Color::from_rgb8_const(0x2F, 0x6F, 0xDE);
const SOFT: Color = Color::from_rgb8_const(0xF3, 0xF4, 0xF7);
const SOFT_ACCENT: Color = Color::from_rgb8_const(0xEE, 0xF3, 0xFD);
const SEAL: Color = Color::from_rgb8_const(0x1E, 0x7B, 0x4F);

/// What the page furniture (title block, footer, seal) says about the version.
pub struct Info<'a> {
    pub title: &'a str,
    pub icon: &'a str,
    pub number: u32,
    pub closed: bool,
    /// The short fingerprint code, shown in every footer.
    pub code: Option<&'a str>,
}

/// One thing on a page, in page points from the page's top left.
#[derive(Clone, Debug)]
pub enum Item {
    Text { x: f64, y: f64, w: f64, h: f64, data: Box<TextData> },
    Shape { x: f64, y: f64, w: f64, h: f64, ellipse: bool, fill: Option<Color>, stroke: Option<(Color, f64)>, radius: f64 },
}

impl Item {
    fn rect(x: f64, y: f64, w: f64, h: f64, fill: Color) -> Self {
        Self::Shape { x, y, w, h, ellipse: false, fill: Some(fill), stroke: None, radius: 0.0 }
    }

    /// The text of a text item (empty for a shape).
    #[cfg(test)]
    pub fn text(&self) -> &str {
        match self {
            Self::Text { data, .. } => &data.characters,
            Self::Shape { .. } => "",
        }
    }
}

/// The laid-out version: one list of items per page, bottom to top.
#[derive(Clone, Debug, Default)]
pub struct Layout {
    pub pages: Vec<Vec<Item>>,
}

/// How a piece of text looks before its runs change it.
#[derive(Clone)]
struct Look {
    size: f64,
    bold: bool,
    italic: bool,
    color: Color,
    family: Option<String>,
    align: TextAlignH,
}

impl Look {
    fn body() -> Self {
        Self { size: BODY, bold: false, italic: false, color: INK, family: None, align: TextAlignH::Left }
    }

    fn sized(size: f64, bold: bool) -> Self {
        Self { size, bold, ..Self::body() }
    }

    fn muted(size: f64) -> Self {
        Self { color: MUTED, ..Self::sized(size, false) }
    }

    /// The block's own font, size and colour over the default look of its kind.
    fn of(b: &Block, base: Self) -> Self {
        let mut l = base;
        if let Some(s) = b.style.size {
            l.size = f64::from(s);
        }
        if let Some(c) = b.style.color.as_deref().and_then(Color::from_hex) {
            l.color = c;
        }
        if let Some(f) = b.style.font.as_ref().filter(|f| !f.is_empty()) {
            l.family = Some(f.clone());
        }
        l
    }

    fn line(&self) -> f64 {
        self.size * LINE / 100.0
    }
}

fn run_style(look: &Look, r: Option<&Run>) -> TextStyle {
    let bold = look.bold || r.is_some_and(|r| r.bold);
    let italic = look.italic || r.is_some_and(|r| r.italic);
    let font_style = match (bold, italic) {
        (false, false) => "Regular",
        (true, false) => "Bold",
        (false, true) => "Italic",
        (true, true) => "Bold Italic",
    };
    let linked = r.is_some_and(|r| r.link.is_some() || r.mention.is_some());
    let color = r.and_then(|r| r.color.as_deref()).and_then(Color::from_hex).unwrap_or(if linked { ACCENT } else { look.color });
    let code = r.is_some_and(|r| r.code);
    let decoration = match r {
        Some(r) if r.strike => TextDecoration::Strikethrough,
        Some(r) if r.underline || r.link.is_some() => TextDecoration::Underline,
        _ => TextDecoration::None,
    };
    TextStyle {
        font_family: if code { "Courier New".to_string() } else { look.family.clone().unwrap_or_else(|| "Inter".to_string()) },
        font_style: font_style.to_string(),
        font_size: if code { look.size * 0.92 } else { look.size },
        line_height: LineHeight::Percent(LINE),
        decoration,
        fills: Some(vec![Paint::solid(color)]),
        link: r.and_then(|r| r.link.clone()),
        ..TextStyle::default()
    }
}

/// Runs as a text layer. Only the runs that look different from the base become style runs.
fn text_data(runs: &[Run], look: &Look) -> TextData {
    let base = run_style(look, None);
    let mut characters = String::new();
    let mut style_runs = Vec::new();
    let mut at = 0usize;
    for r in runs {
        let t = r.text.replace('\t', "    ");
        let n = t.chars().count();
        if n == 0 {
            continue;
        }
        let style = run_style(look, Some(r));
        if style != base {
            style_runs.push(StyleRun { start: at, end: at + n, style });
        }
        characters.push_str(&t);
        at += n;
    }
    TextData { characters, style: base, runs: style_runs, auto_resize: TextAutoResize::Height, align_h: look.align, ..TextData::default() }
}

fn plain(text: &str) -> Vec<Run> {
    vec![Run::plain(text)]
}

/// One wrapped line of a measured text: where its characters start and end, and where it ends down the box.
#[derive(Clone, Copy)]
struct LineBox {
    start: usize,
    end: usize,
    bottom: f64,
}

/// A measured text: how tall it is and where each wrapped line ends.
struct Measured {
    height: f64,
    lines: Vec<LineBox>,
}

fn measure(data: &TextData, w: f64) -> Measured {
    let line = data.style.font_size * LINE / 100.0;
    if data.characters.is_empty() {
        return Measured { height: line, lines: Vec::new() };
    }
    let lay = figcraft_text::layout_cached(data, Some(w.max(1.0)), None);
    let lines: Vec<LineBox> = lay.lines.iter().map(|l| LineBox { start: l.char_start, end: l.char_end, bottom: l.top + l.height }).collect();
    let height = if lines.is_empty() {
        // No face to measure with: a rough estimate keeps the layout usable.
        let per_line = (w / (data.style.font_size * 0.5)).max(1.0);
        let n = data.characters.split('\n').map(|p| (p.chars().count() as f64 / per_line).ceil().max(1.0)).sum::<f64>();
        n * line
    } else {
        lay.height.max(lines.last().map_or(0.0, |l| l.bottom))
    };
    Measured { height, lines }
}

/// The characters `from..to` of a text, with their style runs.
fn slice(data: &TextData, from: usize, to: usize) -> TextData {
    let mut d = data.clone();
    d.characters = data.characters.chars().skip(from).take(to.saturating_sub(from)).collect();
    d.runs = data
        .runs
        .iter()
        .filter_map(|r| {
            let (s, e) = (r.start.max(from), r.end.min(to));
            (s < e).then(|| StyleRun { start: s - from, end: e - from, style: r.style.clone() })
        })
        .collect();
    d
}

/// How much of a text goes on the current page.
enum Fit {
    All,
    NextPage,
    Lines(usize),
}

/// Where a piece of text landed: its page and vertical extent.
#[derive(Clone, Copy)]
struct Frag {
    page: usize,
    y: f64,
    h: f64,
}

/// The pages being filled, with the cursor.
struct Pager {
    pages: Vec<Vec<Item>>,
    y: f64,
    full: bool,
}

impl Pager {
    fn new() -> Self {
        Self { pages: vec![Vec::new()], y: MARGIN_TOP, full: false }
    }

    fn cur(&self) -> usize {
        self.pages.len().saturating_sub(1)
    }

    fn room(&self) -> f64 {
        BOTTOM - self.y
    }

    fn fresh(&self) -> bool {
        self.y <= MARGIN_TOP + 0.5
    }

    fn page_break(&mut self) {
        if self.pages.len() >= MAX_PAGES {
            self.full = true;
        } else {
            self.pages.push(Vec::new());
        }
        self.y = MARGIN_TOP;
    }

    /// Start a new page unless `h` still fits (or the page is empty and would not fit anyway).
    fn need(&mut self, h: f64) {
        if h > self.room() && !self.fresh() {
            self.page_break();
        }
    }

    /// Space between blocks; none at the top of a page.
    fn gap(&mut self, g: f64) {
        if !self.fresh() {
            self.y += g;
        }
    }

    fn push(&mut self, item: Item) {
        if self.full {
            return;
        }
        let cur = self.cur();
        if let Some(p) = self.pages.get_mut(cur) {
            p.push(item);
        }
    }

    fn push_at(&mut self, page: usize, item: Item) {
        if let Some(p) = self.pages.get_mut(page) {
            p.push(item);
        }
    }

    /// Under everything else on `page` (a background).
    fn behind(&mut self, page: usize, item: Item) {
        if let Some(p) = self.pages.get_mut(page) {
            p.insert(0, item);
        }
    }

    /// Put a text at column `x`, `w` wide, splitting it between pages by whole lines (see
    /// [`Pager::lines_that_fit`]).
    fn text(&mut self, data: TextData, x: f64, w: f64) -> Vec<Frag> {
        let mut data = data;
        let mut frags = Vec::new();
        for _ in 0..MAX_PAGES + 2 {
            if self.full {
                return frags;
            }
            let m = measure(&data, w);
            let n = match self.lines_that_fit(&m) {
                Fit::All => break,
                Fit::NextPage => {
                    self.page_break();
                    continue;
                }
                Fit::Lines(n) => n,
            };
            let total = data.characters.chars().count();
            // The page ends after line `n`; the next page starts where line `n + 1` starts, so the
            // space or line break between them is on neither.
            let (Some(last), Some(next)) = (m.lines.get(n - 1), m.lines.get(n)) else { break };
            if last.end == 0 || next.start >= total {
                break;
            }
            frags.push(self.place(slice(&data, 0, last.end), x, w, last.bottom));
            data = slice(&data, next.start, total);
            self.page_break();
        }
        // What is left fits here (or cannot be split): it goes in whole, never dropped.
        let m = measure(&data, w);
        if m.lines.is_empty() && m.height > self.room() && !self.fresh() {
            self.page_break();
        }
        frags.push(self.place(data, x, w, m.height));
        frags
    }

    /// How many lines of a measured text go on this page. A single line is never left alone at the
    /// bottom (orphan) or the top (widow) of a page.
    fn lines_that_fit(&self, m: &Measured) -> Fit {
        let fit = m.lines.iter().take_while(|l| l.bottom <= self.room() + 0.01).count();
        if m.lines.is_empty() || fit >= m.lines.len() {
            return Fit::All;
        }
        if fit == 0 || (fit == 1 && !self.fresh()) {
            return if self.fresh() { Fit::Lines(1) } else { Fit::NextPage };
        }
        let n = if m.lines.len() - fit == 1 { fit - 1 } else { fit };
        // Keeping the last line company may leave one line alone at the bottom: then all of it moves.
        if n <= 1 && !self.fresh() { Fit::NextPage } else { Fit::Lines(n.max(1)) }
    }

    fn place(&mut self, data: TextData, x: f64, w: f64, h: f64) -> Frag {
        let frag = Frag { page: self.cur(), y: self.y, h };
        self.push(Item::Text { x, y: self.y, w, h, data: Box::new(data) });
        self.y += h;
        frag
    }
}

/// Lay out a version: the title block, every block, and the footer of each page.
pub fn layout(info: &Info, blocks: &[Block]) -> Layout {
    let mut p = Pager::new();
    title_block(&mut p, info);
    lay_blocks(&mut p, blocks, MARGIN_X, CONTENT_W, 0);
    let total = p.pages.len();
    for i in 0..total {
        footer(&mut p, info, i, total);
    }
    Layout { pages: p.pages }
}

fn title_block(p: &mut Pager, info: &Info) {
    let title = if info.title.trim().is_empty() { "Untitled" } else { info.title.trim() };
    let heading = if info.icon.is_empty() { title.to_string() } else { format!("{} {title}", info.icon) };
    let seal_room = if info.closed { 70.0 } else { 0.0 };
    p.text(text_data(&plain(&heading), &Look::sized(26.0, true)), MARGIN_X, CONTENT_W - seal_room);
    let state = if info.closed { "Closed" } else { "Draft" };
    p.y += 2.0;
    p.text(text_data(&plain(&format!("Version {} · {state}", info.number)), &Look::muted(10.0)), MARGIN_X, CONTENT_W);
    p.y += 10.0;
    p.push(Item::rect(MARGIN_X, p.y, CONTENT_W, 0.75, RULE));
    p.y += 18.0;
    if info.closed {
        seal(p, info);
    }
}

/// A round stamp at the top right of the first page.
fn seal(p: &mut Pager, info: &Info) {
    let (x, y, d) = (PAGE_W - MARGIN_X - 58.0, 38.0, 58.0);
    p.push(Item::Shape { x, y, w: d, h: d, ellipse: true, fill: None, stroke: Some((SEAL, 1.5)), radius: 0.0 });
    p.push(Item::Shape { x: x + 4.0, y: y + 4.0, w: d - 8.0, h: d - 8.0, ellipse: true, fill: None, stroke: Some((SEAL, 0.5)), radius: 0.0 });
    let look = Look { color: SEAL, align: TextAlignH::Center, ..Look::sized(8.0, true) };
    for (text, dy) in [("CLOSED".to_string(), 19.0), (format!("v{}", info.number), 31.0)] {
        let data = text_data(&plain(&text), &look);
        let h = measure(&data, d).height;
        p.push(Item::Text { x, y: y + dy, w: d, h, data: Box::new(data) });
    }
}

fn short(text: &str, max: usize) -> String {
    if text.chars().count() <= max { text.to_string() } else { format!("{}…", text.chars().take(max.saturating_sub(1)).collect::<String>()) }
}

fn footer(p: &mut Pager, info: &Info, index: usize, total: usize) {
    let y = PAGE_H - 62.0;
    let title = if info.title.trim().is_empty() { "Untitled" } else { info.title.trim() };
    let state = if info.closed { "closed" } else { "draft" };
    let left = format!("{} · version {} ({state})", short(title, 34), info.number);
    let right = format!("Page {} of {total}", index + 1);
    let mut items = vec![Item::rect(MARGIN_X, y, CONTENT_W, 0.75, RULE)];
    let mut note = |text: &str, x: f64, w: f64, look: &Look| {
        let data = text_data(&plain(text), look);
        let h = measure(&data, w).height;
        items.push(Item::Text { x, y: y + 8.0, w, h, data: Box::new(data) });
    };
    note(&left, MARGIN_X, 250.0, &Look::muted(8.0));
    if let Some(code) = info.code {
        note(code, MARGIN_X + 250.0, 130.0, &Look { color: INK, align: TextAlignH::Center, ..Look::sized(8.0, true) });
    }
    note(&right, PAGE_W - MARGIN_X - 100.0, 100.0, &Look { align: TextAlignH::Right, ..Look::muted(8.0) });
    for it in items {
        p.push_at(index, it);
    }
}

fn lay_blocks(p: &mut Pager, list: &[Block], x: f64, w: f64, depth: usize) {
    if depth > MAX_DEPTH {
        return;
    }
    let mut number = 0usize;
    for b in list {
        // Past the last page nothing more can be placed: stop measuring.
        if p.full {
            return;
        }
        number = if matches!(b.kind, BlockKind::Numbered) { number + 1 } else { 0 };
        let frags = lay_block(p, b, x, w, number, depth);
        if let Some(bg) = b.style.background.as_deref().and_then(Color::from_hex) {
            for f in frags {
                p.behind(f.page, Item::rect(x - 4.0, f.y - 2.0, w + 8.0, f.h + 4.0, bg));
            }
        }
    }
}

fn kids(p: &mut Pager, b: &Block, x: f64, w: f64, depth: usize) {
    lay_blocks(p, &b.children, x + 18.0, (w - 18.0).max(40.0), depth + 1);
}

fn lay_block(p: &mut Pager, b: &Block, x: f64, w: f64, number: usize, depth: usize) -> Vec<Frag> {
    match &b.kind {
        BlockKind::Paragraph => paragraph(p, b, x, w, depth),
        BlockKind::Heading { level } => heading(p, b, x, w, *level),
        BlockKind::Bulleted | BlockKind::Numbered | BlockKind::Checklist { .. } => list_item(p, b, x, w, number, depth),
        BlockKind::Toggle { .. } => {
            let frags = p.text(text_data(&b.text, &Look::of(b, Look::sized(BODY, true))), x, w);
            p.gap(4.0);
            kids(p, b, x, w, depth);
            frags
        }
        BlockKind::Callout { icon } => {
            let mut runs = b.text.clone();
            if !icon.is_empty() {
                runs.insert(0, Run::plain(format!("{icon}  ")));
            }
            let frags = boxed(p, text_data(&runs, &Look::of(b, Look::body())), x, w, Some(SOFT), None);
            kids(p, b, x, w, depth);
            frags
        }
        BlockKind::Quote => {
            let look = Look { italic: true, color: MUTED, ..Look::of(b, Look::body()) };
            let frags = boxed(p, text_data(&b.text, &look), x, w, None, Some(RULE));
            kids(p, b, x, w, depth);
            frags
        }
        BlockKind::Divider => {
            p.gap(6.0);
            p.need(4.0);
            p.push(Item::rect(x, p.y, w, 0.75, RULE));
            p.y += 0.75;
            p.gap(10.0);
            Vec::new()
        }
        BlockKind::Table { table } => {
            table_block(p, table, x, w);
            Vec::new()
        }
        BlockKind::Link { url } => link_block(p, b, url, x, w),
        BlockKind::Decision { who, when, why } => decision(p, b, (who, when, why), x, w),
        _ => lay_simple(p, b, x, w),
    }
}

/// The blocks that are one box or line of their own.
fn lay_simple(p: &mut Pager, b: &Block, x: f64, w: f64) -> Vec<Frag> {
    match &b.kind {
        BlockKind::Image { alt, .. } => {
            let label = if alt.is_empty() { "Image".to_string() } else { format!("Image: {alt}") };
            placeholder(p, &label, x, w, 90.0);
        }
        BlockKind::Embed { node, live, .. } => {
            placeholder(p, &format!("Layer {node} ({})", if *live { "live" } else { "snapshot" }), x, w, 90.0);
        }
        BlockKind::AutoTable { source } => {
            let what = match source {
                AutoSource::Screens => "screens",
                AutoSource::Components => "components",
                AutoSource::Variables => "variables",
            };
            placeholder(p, &format!("Automatic table of the {what}"), x, w, 40.0);
        }
        BlockKind::Shape { shape, fill } => shape_block(p, shape, fill.as_deref(), x),
        BlockKind::Swatch { color } => swatch(p, b, color, x, w),
        BlockKind::Signature { role } => signature(p, b, role, x, w),
        _ => {}
    }
    Vec::new()
}

fn paragraph(p: &mut Pager, b: &Block, x: f64, w: f64, depth: usize) -> Vec<Frag> {
    let frags = p.text(text_data(&b.text, &Look::of(b, Look::body())), x, w);
    p.gap(7.0);
    kids(p, b, x, w, depth);
    frags
}

fn heading(p: &mut Pager, b: &Block, x: f64, w: f64, level: u8) -> Vec<Frag> {
    let size = match level.clamp(1, 4) {
        1 => 20.0,
        2 => 16.0,
        3 => 13.5,
        _ => 12.0,
    };
    let look = Look::of(b, Look::sized(size, true));
    p.gap(if level <= 1 { 12.0 } else { 8.0 });
    // A heading never ends a page: it moves with the lines under it.
    p.need(look.line() + 48.0);
    let frags = p.text(text_data(&b.text, &look), x, w);
    p.y += 4.0;
    frags
}

fn list_item(p: &mut Pager, b: &Block, x: f64, w: f64, number: usize, depth: usize) -> Vec<Frag> {
    let indent = 20.0;
    let look = Look::of(b, Look::body());
    let checked = matches!(b.kind, BlockKind::Checklist { checked: true });
    let look = if checked { Look { color: MUTED, ..look } } else { look };
    let line = look.line();
    p.need(line * 2.0);
    let frags = p.text(text_data(&b.text, &look), x + indent, w - indent);
    if let Some(first) = frags.first() {
        let (mx, my) = (x, first.y);
        match &b.kind {
            BlockKind::Bulleted => {
                let r = 4.0;
                p.push_at(
                    first.page,
                    Item::Shape {
                        x: mx + 6.0,
                        y: my + line / 2.0 - r / 2.0,
                        w: r,
                        h: r,
                        ellipse: true,
                        fill: Some(look.color),
                        stroke: None,
                        radius: 0.0,
                    },
                );
            }
            BlockKind::Numbered => {
                let data = text_data(&plain(&format!("{number}.")), &Look { align: TextAlignH::Right, ..look.clone() });
                // Right-aligned against the text, reaching into the margin for long numbers ("100.").
                let mw = indent + 20.0;
                p.push_at(first.page, Item::Text { x: mx + indent - 5.0 - mw, y: my, w: mw, h: line, data: Box::new(data) });
            }
            _ => {
                let s = 9.0;
                p.push_at(
                    first.page,
                    Item::Shape {
                        x: mx + 2.0,
                        y: my + line / 2.0 - s / 2.0,
                        w: s,
                        h: s,
                        ellipse: false,
                        fill: checked.then_some(INK),
                        stroke: Some((INK, 1.0)),
                        radius: 2.0,
                    },
                );
            }
        }
    }
    p.y += 3.0;
    kids(p, b, x, w, depth);
    frags
}

/// A text inside padding, with a background and/or a bar down its left edge on every page it
/// touches.
fn boxed(p: &mut Pager, data: TextData, x: f64, w: f64, bg: Option<Color>, bar: Option<Color>) -> Vec<Frag> {
    let pad = 8.0;
    let bar_w = if bar.is_some() { 3.0 } else { 0.0 };
    p.gap(4.0);
    p.need(pad * 2.0 + 2.0 * data.style.font_size * LINE / 100.0);
    p.y += pad;
    let frags = p.text(data, x + pad + bar_w + 2.0, (w - 2.0 * pad - bar_w - 2.0).max(40.0));
    p.y += pad;
    let last = frags.len().saturating_sub(1);
    for (i, f) in frags.iter().enumerate() {
        let top = if i == 0 { f.y - pad } else { f.y };
        let bottom = if i == last { f.y + f.h + pad } else { f.y + f.h };
        if let Some(c) = bg {
            p.behind(f.page, Item::Shape { x, y: top, w, h: bottom - top, ellipse: false, fill: Some(c), stroke: None, radius: 4.0 });
        }
        if let Some(c) = bar {
            p.push_at(f.page, Item::rect(x, top, 3.0, bottom - top, c));
        }
    }
    p.gap(6.0);
    frags
}

fn decision(p: &mut Pager, b: &Block, meta: (&str, &str, &str), x: f64, w: f64) -> Vec<Frag> {
    let mut runs = vec![Run { text: "Decision: ".into(), bold: true, color: Some("#2F6FDE".into()), ..Run::default() }];
    runs.extend(b.text.iter().map(|r| Run { bold: true, ..r.clone() }));
    for (label, value) in [("Who", meta.0), ("When", meta.1), ("Why", meta.2)] {
        if !value.is_empty() {
            runs.push(Run::plain(format!("\n{label}: ")));
            runs.push(Run::plain(value));
        }
    }
    boxed(p, text_data(&runs, &Look::of(b, Look::body())), x, w, Some(SOFT_ACCENT), Some(ACCENT))
}

fn link_block(p: &mut Pager, b: &Block, url: &str, x: f64, w: f64) -> Vec<Frag> {
    let mut runs = b.text.clone();
    if runs.iter().all(|r| r.text.is_empty()) {
        runs = plain(url);
    }
    for r in &mut runs {
        r.link = Some(url.to_string());
    }
    let frags = p.text(text_data(&runs, &Look::of(b, Look::body())), x, w);
    p.gap(7.0);
    frags
}

fn table_block(p: &mut Pager, t: &Table, x: f64, w: f64) {
    let cols = t.rows.iter().map(Vec::len).max().unwrap_or(0);
    if cols == 0 {
        return;
    }
    let (pad, cw) = (5.0, w / cols as f64);
    p.gap(4.0);
    for (ri, row) in t.rows.iter().enumerate() {
        let head = t.header && ri == 0;
        let look = Look::sized(10.0, head);
        let cells: Vec<(TextData, f64)> = (0..cols)
            .map(|c| {
                let d = text_data(row.get(c).map_or(&[][..], Vec::as_slice), &look);
                let h = measure(&d, cw - 2.0 * pad).height;
                (d, h)
            })
            .collect();
        let h = cells.iter().map(|c| c.1).fold(0.0, f64::max) + 2.0 * pad;
        p.need(h);
        let y = p.y;
        for (c, (data, th)) in cells.into_iter().enumerate() {
            let cx = x + c as f64 * cw;
            p.push(Item::Shape { x: cx, y, w: cw, h, ellipse: false, fill: head.then_some(SOFT), stroke: Some((RULE, 0.75)), radius: 0.0 });
            p.push(Item::Text { x: cx + pad, y: y + pad, w: cw - 2.0 * pad, h: th, data: Box::new(data) });
        }
        p.y += h;
    }
    p.gap(10.0);
}

/// A box standing in for something this layout does not draw (a picture, a live layer).
fn placeholder(p: &mut Pager, label: &str, x: f64, w: f64, h: f64) {
    p.gap(4.0);
    p.need(h);
    let y = p.y;
    p.push(Item::Shape { x, y, w, h, ellipse: false, fill: Some(SOFT), stroke: Some((RULE, 0.75)), radius: 4.0 });
    let data = text_data(&plain(label), &Look { align: TextAlignH::Center, ..Look::muted(10.0) });
    let th = measure(&data, w - 16.0).height;
    p.push(Item::Text { x: x + 8.0, y: y + (h - th) / 2.0, w: w - 16.0, h: th, data: Box::new(data) });
    p.y += h;
    p.gap(10.0);
}

fn shape_block(p: &mut Pager, shape: &str, fill: Option<&str>, x: f64) {
    let color = fill.and_then(Color::from_hex).unwrap_or(RULE);
    let (w, h) = if shape == "line" { (160.0, 2.0) } else { (120.0, 60.0) };
    p.gap(4.0);
    p.need(h);
    let y = p.y;
    let ellipse = shape == "ellipse";
    p.push(Item::Shape { x, y, w, h, ellipse, fill: Some(color), stroke: None, radius: if ellipse || shape == "line" { 0.0 } else { 4.0 } });
    p.y += h;
    p.gap(10.0);
}

fn swatch(p: &mut Pager, b: &Block, color: &str, x: f64, w: f64) {
    p.gap(4.0);
    p.need(30.0);
    let y = p.y;
    let chip = Color::from_hex(color).unwrap_or(SOFT);
    p.push(Item::Shape { x, y, w: 28.0, h: 28.0, ellipse: false, fill: Some(chip), stroke: Some((RULE, 0.75)), radius: 4.0 });
    let mut runs = vec![Run { text: format!("{color}  "), bold: true, ..Run::default() }];
    runs.extend(b.text.iter().cloned());
    let data = text_data(&runs, &Look::of(b, Look::body()));
    let th = measure(&data, w - 38.0).height;
    p.push(Item::Text { x: x + 38.0, y: y + (28.0 - th).max(0.0) / 2.0, w: w - 38.0, h: th, data: Box::new(data) });
    p.y += 28.0;
    p.gap(10.0);
}

fn signature(p: &mut Pager, b: &Block, role: &str, x: f64, w: f64) {
    p.gap(14.0);
    p.need(64.0);
    let y = p.y;
    p.push(Item::rect(x, y + 36.0, w.min(240.0), 1.0, INK));
    let caption = if !role.is_empty() {
        role.to_string()
    } else if !b.text.is_empty() {
        b.plain_text()
    } else {
        "Signature".to_string()
    };
    let data = text_data(&plain(&caption), &Look::muted(9.0));
    let th = measure(&data, w.min(240.0)).height;
    p.push(Item::Text { x, y: y + 42.0, w: w.min(240.0), h: th, data: Box::new(data) });
    p.y += 64.0;
}
