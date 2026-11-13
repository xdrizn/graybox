//! Documents (G13): written documentation kept inside the project file.
//!
//! A [`Documents`] store holds a tree of [`Doc`]s (a document can hold documents, like a Notion
//! page). Each document has versions; a version is a list of [`Block`]s (heading, paragraph, list,
//! checklist, callout, table, embed of a layer, decision…) made of text [`Run`]s. A version is a
//! draft until it is closed (G13.4); a closed version can never change, and changing it means a new
//! version.
//!
//! The store is saved in the file under `docs`, only when it has a document, so a file without
//! documents is byte for byte what it was before and upstream FigCraft ignores the key. Cloning the
//! store (an undo snapshot) copies pointers, like the node map.
//!
//! Everything here is plain data and checked edits. Files are untrusted: loading repairs the tree
//! and the ids and cuts anything past the caps, so the editor never meets a cycle or a duplicate id.

use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Value, json};

use crate::NodeId;

mod edit;
mod format;
mod table;
pub use edit::{Merge, Split, split_runs};
pub use format::{Mark, format_runs, has_mark};
pub use table::TableOp;

/// Most documents a file holds.
pub const MAX_DOCS: usize = 5_000;
/// Deepest nesting of documents inside documents (a root is 1).
pub const MAX_DEPTH: usize = 12;
/// Most versions a document keeps.
pub const MAX_VERSIONS: usize = 500;
/// Most blocks in one version, nested ones included.
pub const MAX_BLOCKS: usize = 20_000;
/// Deepest nesting of blocks inside blocks (a top-level block is 1).
pub const MAX_BLOCK_DEPTH: usize = 8;
/// Longest title, in characters.
pub const MAX_TITLE: usize = 200;
/// Longest icon (an emoji can be several characters), in characters.
pub const MAX_ICON: usize = 16;
/// Most characters of text in one block.
pub const MAX_TEXT: usize = 100_000;
/// Most runs in one block.
pub const MAX_RUNS: usize = 2_000;
/// Most rows and columns of a table block.
pub const MAX_TABLE: usize = 100;

/// A document's id in its file. Ids are never reused.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DocId(pub u64);

/// A block's id, unique in its file (so a mention or a comment can point at one).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BlockId(pub u64);

/// Why an edit was refused. The text is for the person (or for Claude).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DocError {
    NoDoc(DocId),
    NoBlock(BlockId),
    NoVersion(u32),
    /// The version is closed (signed): it can never change.
    Closed(u32),
    Limit(&'static str),
    Cycle,
    Invalid(&'static str),
}

impl fmt::Display for DocError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoDoc(id) => write!(f, "there is no document {}", id.0),
            Self::NoBlock(id) => write!(f, "there is no block {}", id.0),
            Self::NoVersion(n) => write!(f, "the document has no version {n}"),
            Self::Closed(n) => write!(f, "version {n} is closed and can never change; make a new version to edit"),
            Self::Limit(what) => write!(f, "too many {what}"),
            Self::Cycle => write!(f, "that would put something inside itself"),
            Self::Invalid(what) => write!(f, "{what}"),
        }
    }
}

impl std::error::Error for DocError {}

fn is_false(b: &bool) -> bool {
    !*b
}

/// Whether `s` is a colour written `#RGB`, `#RGBA`, `#RRGGBB` or `#RRGGBBAA`.
pub fn is_hex_color(s: &str) -> bool {
    s.strip_prefix('#').is_some_and(|h| matches!(h.len(), 3 | 4 | 6 | 8) && h.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// `s` without control characters, cut to `max` characters.
fn clean(s: &str, max: usize) -> String {
    s.chars().filter(|c| !c.is_control()).take(max).collect()
}

/// What a run of text points at when it is a mention (`@layer`, `@document`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Mention {
    Layer { node: NodeId },
    Document { doc: DocId },
}

/// A piece of text with one look. A block's text is a list of runs.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Run {
    pub text: String,
    #[serde(skip_serializing_if = "is_false")]
    pub bold: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub italic: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub underline: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub strike: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub code: bool,
    /// Text colour, `#RRGGBB`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// Highlight colour, `#RRGGBB`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub highlight: Option<String>,
    /// A web address this text links to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mention: Option<Mention>,
}

impl Run {
    pub fn plain(text: impl Into<String>) -> Self {
        Self { text: text.into(), ..Self::default() }
    }

    /// The same run with other text (a mention whose words changed is ordinary text).
    fn with_text(&self, text: &str) -> Self {
        let mention = if text == self.text { self.mention } else { None };
        Self { text: text.to_string(), mention, ..self.clone() }
    }

    fn sanitize(&mut self) {
        self.text = self.text.chars().filter(|c| !c.is_control() || *c == '\n' || *c == '\t').collect();
        for c in [&mut self.color, &mut self.highlight] {
            if c.as_deref().is_some_and(|v| !is_hex_color(v)) {
                *c = None;
            }
        }
        if let Some(l) = &mut self.link {
            *l = clean(l, 2_000);
            if l.is_empty() {
                self.link = None;
            }
        }
    }
}

/// How a table block is filled: one list of cells per row, each cell a list of runs.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Table {
    pub rows: Vec<Vec<Vec<Run>>>,
    /// The first row is a header.
    #[serde(skip_serializing_if = "is_false")]
    pub header: bool,
}

/// Where an automatic table (G13.5) takes its rows from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AutoSource {
    #[default]
    Screens,
    Components,
    Variables,
}

/// What a block is. The text of a block is in [`Block::text`] for every kind that has text.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum BlockKind {
    Paragraph,
    /// Levels 1 to 4.
    Heading {
        level: u8,
    },
    Bulleted,
    Numbered,
    Checklist {
        #[serde(default)]
        checked: bool,
    },
    Toggle {
        #[serde(default)]
        open: bool,
    },
    Callout {
        #[serde(default)]
        icon: String,
    },
    Quote,
    Divider,
    Table {
        #[serde(default)]
        table: Table,
    },
    /// A link; the block text is its title.
    Link {
        #[serde(default)]
        url: String,
    },
    /// An image of the file (its key in the file's images) with a description.
    Image {
        #[serde(default)]
        image: String,
        #[serde(default)]
        alt: String,
    },
    /// A simple shape drawn in the document (`rectangle`, `ellipse`, `line`).
    Shape {
        #[serde(default)]
        shape: String,
        #[serde(default)]
        fill: Option<String>,
    },
    /// A colour chip with its code.
    Swatch {
        #[serde(default)]
        color: String,
    },
    /// A layer of the file shown in the document: live (follows the layer) or a pinned snapshot.
    Embed {
        node: NodeId,
        #[serde(default)]
        live: bool,
        /// The pinned picture's key in the file's images, when not live.
        #[serde(default)]
        snapshot: Option<String>,
    },
    /// A decision: the block text says what; this says who decided, when and why.
    Decision {
        #[serde(default)]
        who: String,
        #[serde(default)]
        when: String,
        #[serde(default)]
        why: String,
    },
    /// A table made by Graybox from the file (G13.5).
    AutoTable {
        #[serde(default)]
        source: AutoSource,
    },
    /// A place to sign (G15); the block text names the role.
    Signature {
        #[serde(default)]
        role: String,
    },
    /// A kind this build does not know (a file from a newer Graybox). It is read instead of
    /// refusing the whole file, shown as a dim chip, and cannot be made or edited here.
    #[serde(other)]
    Unknown,
}

impl BlockKind {
    fn paragraph() -> Self {
        Self::Paragraph
    }

    /// The kinds a person can name, in the order menus list them.
    pub const NAMES: [&'static str; 20] = [
        "paragraph",
        "heading1",
        "heading2",
        "heading3",
        "heading4",
        "bulleted",
        "numbered",
        "checklist",
        "toggle",
        "callout",
        "quote",
        "divider",
        "table",
        "link",
        "image",
        "shape",
        "swatch",
        "decision",
        "autotable",
        "signature",
    ];

    /// The kind a short name stands for (`heading2`, `bulleted`, `todo`…). Kinds that need data
    /// (an embed's layer) can only be written as an object.
    pub fn from_name(name: &str) -> Option<Self> {
        let n: String = name.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase();
        Some(match n.as_str() {
            "paragraph" | "text" | "p" => Self::Paragraph,
            "heading1" | "h1" | "title" | "heading" => Self::Heading { level: 1 },
            "heading2" | "h2" => Self::Heading { level: 2 },
            "heading3" | "h3" => Self::Heading { level: 3 },
            "heading4" | "h4" => Self::Heading { level: 4 },
            "bulleted" | "bullet" | "bulletedlist" | "list" => Self::Bulleted,
            "numbered" | "numberedlist" => Self::Numbered,
            "checklist" | "todo" | "task" => Self::Checklist { checked: false },
            "toggle" => Self::Toggle { open: false },
            "callout" => Self::Callout { icon: String::new() },
            "quote" => Self::Quote,
            "divider" | "line" | "rule" => Self::Divider,
            "table" => Self::Table { table: Table::default() },
            "link" => Self::Link { url: String::new() },
            "image" => Self::Image { image: String::new(), alt: String::new() },
            "shape" => Self::Shape { shape: "rectangle".into(), fill: None },
            "swatch" | "color" | "colour" => Self::Swatch { color: "#808080".into() },
            "decision" => Self::Decision { who: String::new(), when: String::new(), why: String::new() },
            "autotable" => Self::AutoTable { source: AutoSource::default() },
            "signature" | "sign" => Self::Signature { role: String::new() },
            _ => return None,
        })
    }

    /// A kind from a command parameter: a short name, or an object like `{"type": "heading", "level": 2}`.
    pub fn from_value(v: &Value) -> Option<Self> {
        let kind = match v {
            Value::String(s) => Self::from_name(s)?,
            Value::Object(_) => serde_json::from_value::<Self>(v.clone()).ok()?,
            _ => return None,
        };
        kind.valid().then_some(kind)
    }

    /// The short name of this kind (`heading2`, `checklist`…), as [`from_name`](Self::from_name) reads it.
    pub fn name(&self) -> String {
        match self {
            Self::Paragraph => "paragraph".into(),
            Self::Heading { level } => format!("heading{}", (*level).clamp(1, 4)),
            Self::Bulleted => "bulleted".into(),
            Self::Numbered => "numbered".into(),
            Self::Checklist { .. } => "checklist".into(),
            Self::Toggle { .. } => "toggle".into(),
            Self::Callout { .. } => "callout".into(),
            Self::Quote => "quote".into(),
            Self::Divider => "divider".into(),
            Self::Table { .. } => "table".into(),
            Self::Link { .. } => "link".into(),
            Self::Image { .. } => "image".into(),
            Self::Shape { .. } => "shape".into(),
            Self::Swatch { .. } => "swatch".into(),
            Self::Embed { .. } => "embed".into(),
            Self::Decision { .. } => "decision".into(),
            Self::AutoTable { .. } => "autotable".into(),
            Self::Signature { .. } => "signature".into(),
            Self::Unknown => "unknown".into(),
        }
    }

    /// Whether the block's [`text`](Block::text) is shown for this kind.
    pub fn has_text(&self) -> bool {
        !matches!(
            self,
            Self::Divider
                | Self::Table { .. }
                | Self::Image { .. }
                | Self::Shape { .. }
                | Self::Embed { .. }
                | Self::AutoTable { .. }
                | Self::Unknown
        )
    }

    /// Whether the data in this kind is in range (heading level, table size, colours).
    pub fn valid(&self) -> bool {
        match self {
            Self::Unknown => false,
            Self::Heading { level } => (1..=4).contains(level),
            Self::Table { table } => {
                table.rows.len() <= MAX_TABLE
                    && table.rows.iter().all(|r| r.len() <= MAX_TABLE && r.iter().all(|c| c.len() <= MAX_RUNS))
                    && table.rows.iter().flatten().flatten().map(|r| r.text.chars().count()).sum::<usize>() <= MAX_TEXT
            }
            Self::Swatch { color } => color.is_empty() || is_hex_color(color),
            Self::Shape { fill, .. } => fill.as_deref().is_none_or(is_hex_color),
            Self::Link { url } => url.chars().count() <= 2_000,
            Self::Callout { icon } => icon.chars().count() <= MAX_ICON,
            Self::Image { image, alt } => image.chars().count() <= 128 && alt.chars().count() <= 2_000,
            Self::Embed { snapshot, .. } => snapshot.as_deref().is_none_or(|s| s.chars().count() <= 128),
            Self::Decision { who, when, why } => [who, when, why].iter().all(|s| s.chars().count() <= 2_000),
            Self::Signature { role } => role.chars().count() <= 200,
            _ => true,
        }
    }

    fn sanitize(&mut self) {
        match self {
            Self::Heading { level } => *level = (*level).clamp(1, 4),
            Self::Table { table } => {
                table.rows.truncate(MAX_TABLE);
                for row in &mut table.rows {
                    row.truncate(MAX_TABLE);
                    for cell in row {
                        cell.truncate(MAX_RUNS);
                        cell.iter_mut().for_each(Run::sanitize);
                    }
                }
            }
            Self::Swatch { color } if !color.is_empty() && !is_hex_color(color) => color.clear(),
            Self::Shape { fill, .. } if fill.as_deref().is_some_and(|f| !is_hex_color(f)) => *fill = None,
            Self::Link { url } => *url = clean(url, 2_000),
            Self::Callout { icon } => *icon = clean(icon, MAX_ICON),
            Self::Image { image, alt } => {
                *image = clean(image, 128);
                *alt = clean(alt, 2_000);
            }
            Self::Embed { snapshot: Some(s), .. } => *s = clean(s, 128),
            Self::Decision { who, when, why } => {
                for s in [who, when, why] {
                    *s = clean(s, 2_000);
                }
            }
            Self::Signature { role } => *role = clean(role, 200),
            _ => {}
        }
    }
}

/// Font, size and colours set on one block (everything optional: unset follows the document).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BlockStyle {
    /// Font family for the whole block.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
    /// Text size in points.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<f32>,
    /// Text colour, `#RRGGBB`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// Background colour of the block, `#RRGGBB`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
}

impl BlockStyle {
    fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// Whether every value is in range.
    pub fn valid(&self) -> bool {
        self.font.as_deref().is_none_or(|f| !f.is_empty() && f.chars().count() <= 100 && !f.chars().any(char::is_control))
            && self.size.is_none_or(|s| s.is_finite() && (6.0..=200.0).contains(&s))
            && self.color.as_deref().is_none_or(is_hex_color)
            && self.background.as_deref().is_none_or(is_hex_color)
    }

    fn sanitize(&mut self) {
        if !self.valid() {
            let ok = |c: &Option<String>| c.as_deref().is_none_or(is_hex_color);
            if self.font.as_deref().is_some_and(|f| f.is_empty() || f.chars().count() > 100 || f.chars().any(char::is_control)) {
                self.font = None;
            }
            if self.size.is_some_and(|s| !s.is_finite() || !(6.0..=200.0).contains(&s)) {
                self.size = None;
            }
            if !ok(&self.color) {
                self.color = None;
            }
            if !ok(&self.background) {
                self.background = None;
            }
        }
    }
}

/// One block of a document.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Block {
    /// A block read without an id gets a fresh one when the file loads.
    #[serde(default)]
    pub id: BlockId,
    #[serde(default = "BlockKind::paragraph")]
    pub kind: BlockKind,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub text: Vec<Run>,
    #[serde(default, skip_serializing_if = "BlockStyle::is_default")]
    pub style: BlockStyle,
    /// Blocks inside this one (a toggle's content, an indented list item).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Block>,
}

impl Block {
    pub fn new(id: BlockId, kind: BlockKind) -> Self {
        Self { id, kind, text: Vec::new(), style: BlockStyle::default(), children: Vec::new() }
    }

    /// The text of the block without any look.
    pub fn plain_text(&self) -> String {
        self.text.iter().map(|r| r.text.as_str()).collect()
    }

    /// Replace the text, keeping the look of what did not change: one run keeps its look; with
    /// several runs only the characters that differ are cut out and put in (the new ones take the
    /// look of the run they land in), so typing in a block with bold words or links keeps them.
    pub fn set_plain_text(&mut self, text: &str) {
        if text.is_empty() {
            self.text = Vec::new();
            return;
        }
        if self.text.len() <= 1 {
            let kept = self.text.first().map(|only| only.with_text(text));
            self.text = vec![kept.unwrap_or_else(|| Run::plain(text))];
            return;
        }
        let old: Vec<char> = self.plain_text().chars().collect();
        let new: Vec<char> = text.chars().collect();
        let common = old.len().min(new.len());
        // The unchanged start and end; what is between them is what the edit replaced.
        let head = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
        let tail = old.iter().rev().zip(new.iter().rev()).take(common - head).take_while(|(a, b)| a == b).count();
        let cut_end = old.len() - tail;
        let inserted: String = new.iter().skip(head).take(new.len() - head - tail).collect();
        // The new characters join the run that holds the one before them (the first at the very start).
        let mut target = 0;
        let mut at = 0;
        for (i, r) in self.text.iter().enumerate() {
            if at < head {
                target = i;
            }
            at += r.text.chars().count();
        }
        let mut at = 0;
        let mut out = Vec::with_capacity(self.text.len());
        for (i, mut r) in std::mem::take(&mut self.text).into_iter().enumerate() {
            let chars: Vec<char> = r.text.chars().collect();
            let (start, end) = (at, at + chars.len());
            at = end;
            let keep_head = head.clamp(start, end) - start;
            let keep_tail = cut_end.clamp(start, end) - start;
            // Words typed right after a mention are not part of it: they start a plain run.
            if i == target && !inserted.is_empty() && r.mention.is_some() && keep_head == chars.len() {
                out.push(r);
                out.push(Run::plain(inserted.clone()));
                continue;
            }
            let mut t: String = chars.iter().take(keep_head).collect();
            if i == target {
                t.push_str(&inserted);
            }
            t.extend(chars.iter().skip(keep_tail));
            if !t.is_empty() {
                // A mention whose words were edited is ordinary text now.
                if t != r.text {
                    r.mention = None;
                }
                r.text = t;
                out.push(r);
            }
        }
        self.text = out;
    }

    fn text_len(&self) -> usize {
        self.text.iter().map(|r| r.text.chars().count()).sum()
    }
}

/// Whether a version can still be edited.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VersionState {
    #[default]
    Draft,
    /// Closed (signed): never edited again.
    Closed,
}

/// One version of a document.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DocVersion {
    /// 1, 2, 3… in order.
    #[serde(default)]
    pub number: u32,
    #[serde(default)]
    pub state: VersionState,
    /// SHA-256 of the closed blocks' canonical JSON, hex (G13.4 fills it when closing).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,
    /// When the version was closed, `YYYY-MM-DD HH:MM UTC` (G13.4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closed_at: Option<String>,
    #[serde(default)]
    pub blocks: Vec<Block>,
    /// "For signature": the version ends with a signature section, one slot per signer (G13.7).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub for_signature: bool,
    /// Who signs, in slot order: the designer first, then the client's signers. Part of what the
    /// fingerprint covers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub signers: Vec<crate::docs_version::Signer>,
    /// The signatures given on the closed version (outside the fingerprint, which they record).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub signatures: Vec<crate::docs_version::Signature>,
}

impl DocVersion {
    pub(crate) fn draft(number: u32) -> Self {
        Self {
            number,
            state: VersionState::Draft,
            fingerprint: None,
            closed_at: None,
            blocks: Vec::new(),
            for_signature: false,
            signers: Vec::new(),
            signatures: Vec::new(),
        }
    }

    pub fn is_closed(&self) -> bool {
        self.state == VersionState::Closed
    }

    /// Blocks in this version, nested ones included.
    pub fn block_count(&self) -> usize {
        count(&self.blocks)
    }

    /// The block with this id anywhere in the version.
    pub fn block(&self, id: BlockId) -> Option<&Block> {
        find(&self.blocks, id)
    }

    /// Where a block sits: inside which block (`None`: the top of the version) and at which place
    /// among its siblings. For putting a new block next to it.
    pub fn locate(&self, id: BlockId) -> Option<(Option<BlockId>, usize)> {
        fn walk(blocks: &[Block], parent: Option<BlockId>, id: BlockId) -> Option<(Option<BlockId>, usize)> {
            if let Some(i) = blocks.iter().position(|b| b.id == id) {
                return Some((parent, i));
            }
            blocks.iter().find_map(|b| walk(&b.children, Some(b.id), id))
        }
        walk(&self.blocks, None, id)
    }
}

/// One document.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Doc {
    /// A document read without an id (0) is dropped when the file loads.
    #[serde(default)]
    pub id: DocId,
    #[serde(default)]
    pub title: String,
    /// An emoji or a short symbol shown before the title (empty: none).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub icon: String,
    /// The document that holds this one (found again from `children` when the file is read).
    #[serde(skip)]
    pub parent: Option<DocId>,
    /// The documents inside this one, in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<DocId>,
    /// Never empty; the last one is the current version.
    #[serde(default)]
    pub versions: Vec<DocVersion>,
}

impl Doc {
    /// The version people edit and read by default: the last.
    pub fn current(&self) -> Option<&DocVersion> {
        self.versions.last()
    }

    /// The version numbered `n`.
    pub fn version(&self, n: u32) -> Option<&DocVersion> {
        self.versions.iter().find(|v| v.number == n)
    }
}

/// Where a block goes: inside `parent` (or at the top of the version) at `index` (or last).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Place {
    pub parent: Option<BlockId>,
    pub index: Option<usize>,
}

/// All the documents of a file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Documents {
    docs: imbl::OrdMap<DocId, Arc<Doc>>,
    roots: Vec<DocId>,
    next_doc: u64,
    next_block: u64,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Repr {
    docs: Vec<Doc>,
    roots: Vec<DocId>,
    #[serde(rename = "nextDoc")]
    next_doc: u64,
    #[serde(rename = "nextBlock")]
    next_block: u64,
}

impl Serialize for Documents {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let docs: Vec<&Doc> = self.walk().into_iter().filter_map(|id| self.get(id)).collect();
        let mut st = serde_json::Map::new();
        st.insert("roots".into(), json!(self.roots));
        st.insert("nextDoc".into(), json!(self.next_doc));
        st.insert("nextBlock".into(), json!(self.next_block));
        let docs = serde_json::to_value(docs).map_err(serde::ser::Error::custom)?;
        st.insert("docs".into(), docs);
        Value::Object(st).serialize(s)
    }
}

impl<'de> Deserialize<'de> for Documents {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Repr::deserialize(d).map(Self::from_repr)
    }
}

impl Documents {
    pub fn is_empty(&self) -> bool {
        self.docs.is_empty()
    }

    pub fn len(&self) -> usize {
        self.docs.len()
    }

    pub fn get(&self, id: DocId) -> Option<&Doc> {
        self.docs.get(&id).map(Arc::as_ref)
    }

    /// The documents at the top, in order.
    pub fn roots(&self) -> &[DocId] {
        &self.roots
    }

    /// The siblings of `parent` (the roots for `None`).
    pub fn siblings(&self, parent: Option<DocId>) -> &[DocId] {
        match parent {
            None => &self.roots,
            Some(p) => self.get(p).map_or(&[], |d| d.children.as_slice()),
        }
    }

    /// Every document in tree order (a parent, then its children).
    pub fn walk(&self) -> Vec<DocId> {
        let mut out = Vec::with_capacity(self.docs.len());
        let mut stack: Vec<DocId> = self.roots.iter().rev().copied().collect();
        while let Some(id) = stack.pop() {
            if let Some(d) = self.get(id) {
                out.push(id);
                stack.extend(d.children.iter().rev().copied());
            }
        }
        out
    }

    /// How deep `id` sits (a root is 1; 0 for an unknown id).
    pub fn depth(&self, id: DocId) -> usize {
        let mut n = 0;
        let mut at = Some(id);
        while let Some(i) = at.filter(|_| n <= MAX_DEPTH + 1) {
            let Some(d) = self.get(i) else { break };
            n += 1;
            at = d.parent;
        }
        n
    }

    /// The longest chain of documents inside `id`, `id` counted (1 for a leaf).
    fn height(&self, id: DocId) -> usize {
        let mut deepest = 0;
        let mut stack = vec![(id, 1usize)];
        while let Some((i, h)) = stack.pop() {
            deepest = deepest.max(h);
            if h <= MAX_DEPTH + 1
                && let Some(d) = self.get(i)
            {
                stack.extend(d.children.iter().map(|c| (*c, h + 1)));
            }
        }
        deepest
    }

    /// Whether `maybe` is `of` or sits anywhere inside it.
    pub fn is_inside(&self, maybe: DocId, of: DocId) -> bool {
        let mut at = Some(maybe);
        for _ in 0..=MAX_DEPTH + 1 {
            match at {
                Some(i) if i == of => return true,
                Some(i) => at = self.get(i).and_then(|d| d.parent),
                None => return false,
            }
        }
        false
    }

    pub(crate) fn doc_mut(&mut self, id: DocId) -> Result<&mut Doc, DocError> {
        self.docs.get_mut(&id).map(Arc::make_mut).ok_or(DocError::NoDoc(id))
    }

    fn siblings_mut(&mut self, parent: Option<DocId>) -> Result<&mut Vec<DocId>, DocError> {
        match parent {
            None => Ok(&mut self.roots),
            Some(p) => Ok(&mut self.doc_mut(p)?.children),
        }
    }

    /// Make a document (with an empty first version) inside `parent` at `index` (last by default).
    pub fn create(&mut self, title: &str, parent: Option<DocId>, index: Option<usize>) -> Result<DocId, DocError> {
        if self.docs.len() >= MAX_DOCS {
            return Err(DocError::Limit("documents"));
        }
        if let Some(p) = parent {
            if self.get(p).is_none() {
                return Err(DocError::NoDoc(p));
            }
            if self.depth(p) >= MAX_DEPTH {
                return Err(DocError::Limit("levels of documents"));
            }
        }
        self.next_doc = self.next_doc.max(1);
        let id = DocId(self.next_doc);
        self.next_doc = self.next_doc.saturating_add(1);
        let doc = Doc { id, title: clean(title, MAX_TITLE), icon: String::new(), parent, children: Vec::new(), versions: vec![DocVersion::draft(1)] };
        self.docs.insert(id, Arc::new(doc));
        let list = self.siblings_mut(parent)?;
        let at = index.map_or(list.len(), |i| i.min(list.len()));
        list.insert(at, id);
        Ok(id)
    }

    pub fn rename(&mut self, id: DocId, title: &str) -> Result<(), DocError> {
        self.doc_mut(id)?.title = clean(title, MAX_TITLE);
        Ok(())
    }

    /// Set the icon (empty clears it).
    pub fn set_icon(&mut self, id: DocId, icon: &str) -> Result<(), DocError> {
        if icon.chars().count() > MAX_ICON || icon.chars().any(char::is_control) {
            return Err(DocError::Invalid("an icon is one emoji or symbol"));
        }
        self.doc_mut(id)?.icon = icon.trim().to_string();
        Ok(())
    }

    /// Put `id` inside `parent` (`None`: at the top) at `index` among the siblings left once it is
    /// taken out (last by default).
    pub fn move_to(&mut self, id: DocId, parent: Option<DocId>, index: Option<usize>) -> Result<(), DocError> {
        if self.get(id).is_none() {
            return Err(DocError::NoDoc(id));
        }
        if let Some(p) = parent {
            if self.get(p).is_none() {
                return Err(DocError::NoDoc(p));
            }
            if self.is_inside(p, id) {
                return Err(DocError::Cycle);
            }
            if self.depth(p) + self.height(id) > MAX_DEPTH {
                return Err(DocError::Limit("levels of documents"));
            }
        }
        let old = self.get(id).and_then(|d| d.parent);
        self.siblings_mut(old)?.retain(|c| *c != id);
        let list = self.siblings_mut(parent)?;
        let at = index.map_or(list.len(), |i| i.min(list.len()));
        list.insert(at, id);
        self.doc_mut(id)?.parent = parent;
        Ok(())
    }

    /// Remove `id` and the documents inside it; returns every id removed.
    pub fn delete(&mut self, id: DocId) -> Result<Vec<DocId>, DocError> {
        let parent = self.get(id).ok_or(DocError::NoDoc(id))?.parent;
        let mut gone = Vec::new();
        let mut stack = vec![id];
        while let Some(i) = stack.pop() {
            if let Some(d) = self.docs.remove(&i) {
                stack.extend(d.children.iter().copied());
                gone.push(i);
            }
        }
        self.siblings_mut(parent)?.retain(|c| *c != id);
        Ok(gone)
    }

    // ---- blocks ----

    /// The version `number` (the last by default) of `doc`, to edit: refused when it is closed.
    pub(crate) fn draft_mut(&mut self, doc: DocId, number: Option<u32>) -> Result<&mut DocVersion, DocError> {
        let d = self.doc_mut(doc)?;
        let v = match number {
            Some(n) => d.versions.iter_mut().find(|v| v.number == n).ok_or(DocError::NoVersion(n))?,
            None => d.versions.last_mut().ok_or(DocError::NoVersion(0))?,
        };
        if v.is_closed() { Err(DocError::Closed(v.number)) } else { Ok(v) }
    }

    /// Close or reopen the last version without checks (tests only: closing for real is G13.4).
    #[cfg(test)]
    pub(crate) fn update_version_state_for_tests(&mut self, doc: DocId, state: VersionState) {
        if let Ok(d) = self.doc_mut(doc)
            && let Some(v) = d.versions.last_mut()
        {
            v.state = state;
        }
    }

    fn fresh_block_id(&mut self) -> BlockId {
        self.next_block = self.next_block.max(1);
        let id = BlockId(self.next_block);
        self.next_block = self.next_block.saturating_add(1);
        id
    }

    /// Add a block of `kind` with `text` at `place`; returns its id.
    pub fn insert_block(&mut self, doc: DocId, version: Option<u32>, place: Place, kind: BlockKind, text: Vec<Run>) -> Result<BlockId, DocError> {
        if !kind.valid() {
            return Err(DocError::Invalid("that block's data is out of range"));
        }
        let id = self.fresh_block_id();
        let mut block = Block::new(id, kind);
        block.kind.sanitize();
        block.text = text;
        check_text(&block)?;
        block.text.iter_mut().for_each(Run::sanitize);
        let v = self.draft_mut(doc, version)?;
        if v.block_count() >= MAX_BLOCKS {
            return Err(DocError::Limit("blocks"));
        }
        let depth = match place.parent {
            Some(p) => depth_of(&v.blocks, p).ok_or(DocError::NoBlock(p))? + 1,
            None => 1,
        };
        if depth > MAX_BLOCK_DEPTH {
            return Err(DocError::Limit("levels of blocks"));
        }
        let list = list_mut(&mut v.blocks, place.parent).ok_or(DocError::Invalid("no such parent block"))?;
        let at = place.index.map_or(list.len(), |i| i.min(list.len()));
        list.insert(at, block);
        Ok(id)
    }

    /// Change one block with `edit`. The block is only replaced when `edit` and the checks pass.
    pub fn update_block(
        &mut self,
        doc: DocId,
        version: Option<u32>,
        id: BlockId,
        edit: impl FnOnce(&mut Block) -> Result<(), DocError>,
    ) -> Result<(), DocError> {
        let v = self.draft_mut(doc, version)?;
        let block = find_mut(&mut v.blocks, id).ok_or(DocError::NoBlock(id))?;
        let mut copy = block.clone();
        edit(&mut copy)?;
        if !copy.kind.valid() || !copy.style.valid() {
            return Err(DocError::Invalid("that block's data is out of range"));
        }
        check_text(&copy)?;
        copy.kind.sanitize();
        copy.text.iter_mut().for_each(Run::sanitize);
        // Children and the id stay as they were: an edit changes the block, not the tree.
        copy.children = std::mem::take(&mut block.children);
        copy.id = id;
        *block = copy;
        Ok(())
    }

    /// Move a block (with what is inside it) to `place`.
    pub fn move_block(&mut self, doc: DocId, version: Option<u32>, id: BlockId, place: Place) -> Result<(), DocError> {
        let v = self.draft_mut(doc, version)?;
        let moving = find(&v.blocks, id).ok_or(DocError::NoBlock(id))?;
        let height = height_of(moving);
        if let Some(p) = place.parent {
            if p == id || find(&moving.children, p).is_some() {
                return Err(DocError::Cycle);
            }
            let under = depth_of(&v.blocks, p).ok_or(DocError::NoBlock(p))?;
            if under + height > MAX_BLOCK_DEPTH {
                return Err(DocError::Limit("levels of blocks"));
            }
        } else if height > MAX_BLOCK_DEPTH {
            return Err(DocError::Limit("levels of blocks"));
        }
        let block = take(&mut v.blocks, id).ok_or(DocError::NoBlock(id))?;
        let list = list_mut(&mut v.blocks, place.parent).ok_or(DocError::Invalid("no such parent block"))?;
        let at = place.index.map_or(list.len(), |i| i.min(list.len()));
        list.insert(at, block);
        Ok(())
    }

    /// Remove a block and what is inside it.
    pub fn delete_block(&mut self, doc: DocId, version: Option<u32>, id: BlockId) -> Result<(), DocError> {
        let v = self.draft_mut(doc, version)?;
        take(&mut v.blocks, id).map(|_| ()).ok_or(DocError::NoBlock(id))
    }

    // ---- loading ----

    fn from_repr(r: Repr) -> Self {
        let mut s = Self { docs: imbl::OrdMap::new(), roots: r.roots, next_doc: r.next_doc, next_block: r.next_block };
        for mut doc in r.docs.into_iter().take(MAX_DOCS) {
            if s.docs.contains_key(&doc.id) || doc.id.0 == 0 {
                continue;
            }
            doc.parent = None;
            doc.title = clean(&doc.title, MAX_TITLE);
            doc.icon = clean(&doc.icon, MAX_ICON);
            s.docs.insert(doc.id, Arc::new(doc));
        }
        s.repair_tree();
        s.repair_blocks();
        s
    }

    /// Make the tree a forest again: every document once, parents found from the children lists,
    /// nesting within the cap, and what no list reaches put at the top.
    fn repair_tree(&mut self) {
        let mut seen: BTreeSet<DocId> = BTreeSet::new();
        let mut roots = Vec::new();
        let listed: Vec<DocId> = std::mem::take(&mut self.roots);
        // Documents some other document lists as a child are not roots; the rest are, then (if a
        // cycle hides them from every root) whatever is still unreached.
        let referenced: BTreeSet<DocId> = self.docs.values().flat_map(|d| d.children.iter().copied()).collect();
        let unlisted: Vec<DocId> = self.docs.keys().copied().filter(|id| !referenced.contains(id)).collect();
        let rest: Vec<DocId> = self.docs.keys().copied().collect();
        for id in listed.into_iter().chain(unlisted).chain(rest) {
            if self.docs.contains_key(&id) && !seen.contains(&id) {
                roots.push(id);
                self.attach(id, None, 1, &mut seen);
            }
        }
        self.roots = roots;
        let top = self.docs.keys().map(|k| k.0).max().unwrap_or(0).saturating_add(1);
        self.next_doc = self.next_doc.max(top);
    }

    fn attach(&mut self, id: DocId, parent: Option<DocId>, depth: usize, seen: &mut BTreeSet<DocId>) {
        seen.insert(id);
        let Some(children) = self.docs.get(&id).map(|d| d.children.clone()) else { return };
        let mut kept = Vec::new();
        for c in children {
            if depth < MAX_DEPTH && self.docs.contains_key(&c) && !seen.contains(&c) {
                kept.push(c);
                self.attach(c, Some(id), depth + 1, seen);
            }
        }
        if let Some(d) = self.docs.get_mut(&id).map(Arc::make_mut) {
            d.parent = parent;
            d.children = kept;
        }
    }

    /// Versions numbered 1.., a first version when there is none, block ids unique, the caps held.
    fn repair_blocks(&mut self) {
        let mut seen: BTreeSet<BlockId> = BTreeSet::new();
        let mut top = self.next_block;
        let ids: Vec<DocId> = self.docs.keys().copied().collect();
        for id in ids {
            let Some(d) = self.docs.get_mut(&id).map(Arc::make_mut) else { continue };
            d.versions.truncate(MAX_VERSIONS);
            if d.versions.is_empty() {
                d.versions.push(DocVersion::draft(1));
            }
            for (i, v) in d.versions.iter_mut().enumerate() {
                v.number = u32::try_from(i + 1).unwrap_or(u32::MAX);
                let mut budget = MAX_BLOCKS;
                // Ids are unique within a version: a new version keeps its blocks' ids, so the diff
                // pairs them.
                seen.clear();
                fix_blocks(&mut v.blocks, 1, &mut seen, &mut top, &mut budget);
                crate::docs_version::repair(v);
            }
        }
        self.next_block = top;
    }
}

fn check_text(b: &Block) -> Result<(), DocError> {
    if b.text.len() > MAX_RUNS || b.text_len() > MAX_TEXT {
        return Err(DocError::Limit("characters in one block"));
    }
    Ok(())
}

/// Sanitize the blocks of a version read from a file: unique ids, depth and count within caps.
fn fix_blocks(blocks: &mut Vec<Block>, depth: usize, seen: &mut BTreeSet<BlockId>, next: &mut u64, budget: &mut usize) {
    let allowed = (*budget).min(blocks.len());
    blocks.truncate(allowed);
    *budget -= allowed;
    for b in blocks.iter_mut() {
        if b.id.0 == 0 || !seen.insert(b.id) {
            *next = (*next).max(1);
            let mut fresh = BlockId(*next);
            while !seen.insert(fresh) {
                *next = next.saturating_add(1);
                fresh = BlockId(*next);
            }
            b.id = fresh;
        }
        *next = (*next).max(b.id.0.saturating_add(1));
        b.kind.sanitize();
        b.style.sanitize();
        b.text.truncate(MAX_RUNS);
        b.text.iter_mut().for_each(Run::sanitize);
        if b.text_len() > MAX_TEXT {
            b.text.clear();
        }
        if depth >= MAX_BLOCK_DEPTH {
            b.children.clear();
        } else {
            fix_blocks(&mut b.children, depth + 1, seen, next, budget);
        }
    }
}

fn count(blocks: &[Block]) -> usize {
    blocks.iter().map(|b| 1 + count(&b.children)).sum()
}

fn find(blocks: &[Block], id: BlockId) -> Option<&Block> {
    blocks.iter().find_map(|b| if b.id == id { Some(b) } else { find(&b.children, id) })
}

fn find_mut(blocks: &mut [Block], id: BlockId) -> Option<&mut Block> {
    for b in blocks {
        if b.id == id {
            return Some(b);
        }
        if let Some(found) = find_mut(&mut b.children, id) {
            return Some(found);
        }
    }
    None
}

/// Take a block out of the tree.
fn take(blocks: &mut Vec<Block>, id: BlockId) -> Option<Block> {
    if let Some(i) = blocks.iter().position(|b| b.id == id) {
        return Some(blocks.remove(i));
    }
    blocks.iter_mut().find_map(|b| take(&mut b.children, id))
}

/// The list a block goes into: the top of the version or the children of `parent`.
fn list_mut(blocks: &mut Vec<Block>, parent: Option<BlockId>) -> Option<&mut Vec<Block>> {
    match parent {
        None => Some(blocks),
        Some(p) => find_mut(blocks, p).map(|b| &mut b.children),
    }
}

/// How deep block `id` sits (a top-level block is 1).
fn depth_of(blocks: &[Block], id: BlockId) -> Option<usize> {
    blocks.iter().find_map(|b| if b.id == id { Some(1) } else { depth_of(&b.children, id).map(|d| d + 1) })
}

/// The longest chain of blocks from `b` down, `b` counted.
fn height_of(b: &Block) -> usize {
    1 + b.children.iter().map(height_of).max().unwrap_or(0)
}
