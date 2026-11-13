//! What the page layout of a document turns into (G13.3): real frames and text layers (for the PDF
//! and for the card on the canvas), and the card that follows its document.

use figcraft_color::{Color, Paint};
use figcraft_doc::docs::{Doc, DocId, DocVersion};
use figcraft_doc::{Document, Node, NodeId, NodeKind};
use figcraft_geom::Affine;

use crate::docs_page::{Info, Item, Layout, PAGE_H, PAGE_W, layout};
use crate::{EngineError, Result};

/// A card's marker: the id of the document it shows.
pub const CARD_DOC: &str = "graybox.doc";
/// The version number the card shows. A card of a closed version is frozen; one of a draft follows
/// every edit.
pub const CARD_VERSION: &str = "graybox.doc.version";
/// What the card was built from (a hash of the document's words and the layout revision), so an
/// unchanged document does not rebuild its card.
pub const CARD_HASH: &str = "graybox.doc.hash";
/// Bump when the page layout changes, so cards built by an older build are laid out again.
const LAYOUT_REV: &str = "1";
/// Space between the pages of a card, in points.
const CARD_GAP: f64 = 28.0;

fn other(e: impl std::fmt::Display) -> EngineError {
    EngineError::Other(e.to_string())
}

/// The page furniture for a version of `doc`.
fn info<'a>(doc: &'a Doc, version: &'a DocVersion, code: Option<&'a str>) -> Info<'a> {
    Info { title: &doc.title, icon: &doc.icon, number: version.number, closed: version.is_closed(), code }
}

/// The fingerprint code a version shows (G13.4 gives it its short form; until a version is closed
/// there is none).
fn code_of(version: &DocVersion) -> Option<&str> {
    version.fingerprint.as_deref()
}

fn solid(color: Color) -> Vec<Paint> {
    vec![Paint::solid(color)]
}

fn node_for(doc: &mut Document, item: &Item) -> Node {
    let id = doc.alloc_id();
    match item {
        Item::Text { x, y, w, h, data } => {
            let name: String = data.characters.chars().take(24).collect();
            let color = data.style.fills.as_ref().and_then(|f| f.first()).and_then(Paint::solid_color).unwrap_or(Color::BLACK);
            let mut n = Node::new(id, NodeKind::Text((**data).clone()), name, *w, *h);
            n.fills = solid(color);
            n.transform = Affine::translate((*x, *y));
            n
        }
        Item::Shape { x, y, w, h, ellipse, fill, stroke, radius } => {
            let kind = if *ellipse { NodeKind::Ellipse { start: 0.0, sweep: std::f64::consts::TAU, inner: 0.0 } } else { NodeKind::Rectangle };
            let mut n = Node::new(id, kind, if *ellipse { "Ellipse" } else { "Box" }, *w, *h);
            n.fills = fill.map(solid).unwrap_or_default();
            if let Some((color, weight)) = stroke {
                n.strokes = solid(*color);
                n.stroke.weight = *weight;
            }
            n.corners.radii = [*radius; 4];
            n.transform = Affine::translate((*x, *y));
            n
        }
    }
}

/// One frame per page under `parent`, side by side from `origin` with `gap` between them. A
/// `bordered` page (the card on the canvas) has a thin outline; a printed one does not.
pub fn build_pages(doc: &mut Document, parent: NodeId, l: &Layout, origin: (f64, f64), gap: f64, bordered: bool) -> Result<Vec<NodeId>> {
    let mut frames = Vec::new();
    for (i, items) in l.pages.iter().enumerate() {
        let id = doc.alloc_id();
        let mut page = Node::new(id, NodeKind::Frame, format!("Page {}", i + 1), PAGE_W, PAGE_H);
        page.fills = solid(Color::WHITE);
        page.clip = true;
        page.transform = Affine::translate((origin.0 + i as f64 * (PAGE_W + gap), origin.1));
        if bordered {
            page.strokes = solid(Color::from_rgb8_const(0xD9, 0xDC, 0xE3));
            page.stroke.weight = 1.0;
            // A card's pages are the document's, not canvas layers: edits made to them on the canvas
            // would be lost at the next refresh, so they are locked (the card itself still moves).
            page.locked = true;
        }
        let frame = doc.insert(page, parent, None).map_err(other)?;
        for item in items {
            let node = node_for(doc, item);
            doc.insert(node, frame, None).map_err(other)?;
        }
        frames.push(frame);
    }
    Ok(frames)
}

/// A version as a PDF, one A4 page per page of the layout, and how many pages it has.
pub fn pdf(doc: &Doc, version: &DocVersion) -> Result<(Vec<u8>, usize)> {
    let l = layout(&info(doc, version, code_of(version)), &version.blocks);
    let mut scratch = Document::new(doc.title.clone());
    let page = scratch.first_page().ok_or_else(|| other("no page to lay the document on"))?;
    let frames = build_pages(&mut scratch, page, &l, (0.0, 0.0), 40.0, false)?;
    let opts = figcraft_pdf::PdfOptions { title: Some(doc.title.clone()), ..Default::default() };
    Ok((figcraft_pdf::export_pdf(&scratch, &frames, &opts).map_err(other)?, frames.len()))
}

/// A version as Markdown.
pub fn markdown(doc: &Doc, version: &DocVersion) -> String {
    figcraft_doc::docs_md::to_markdown(&doc.title, &doc.icon, version, code_of(version))
}

/// FNV-1a: stable across builds, so a hash saved in the file means the same later.
fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3))
}

fn content_hash(doc: &Doc, version: &DocVersion) -> String {
    let bytes = serde_json::to_vec(&(&doc.title, &doc.icon, version)).unwrap_or_default();
    format!("{LAYOUT_REV}-{:016x}", fnv(&bytes))
}

/// Make the card at `at` for version `number` of `id` (the last one when `None`) on `page`.
pub fn create_card(doc: &mut Document, page: NodeId, id: DocId, number: Option<u32>, at: (f64, f64)) -> Result<NodeId> {
    let (title, number) = {
        let d = doc.docs.get(id).ok_or_else(|| other(format!("no document {}", id.0)))?;
        let v = match number {
            Some(n) => d.version(n),
            None => d.current(),
        }
        .ok_or_else(|| other("no such version"))?;
        (d.title.clone(), v.number)
    };
    let card_id = doc.alloc_id();
    let mut card = Node::new(card_id, NodeKind::Frame, title, PAGE_W, PAGE_H);
    card.fills = Vec::new();
    card.transform = Affine::translate(at);
    card.plugin_data.insert(CARD_DOC.into(), id.0.to_string());
    card.plugin_data.insert(CARD_VERSION.into(), number.to_string());
    let card_id = doc.insert(card, page, None).map_err(other)?;
    refresh_card(doc, card_id, false)?;
    Ok(card_id)
}

/// The document and version a card shows.
fn card_target(n: &Node) -> Option<(DocId, u32)> {
    let id = n.plugin_data.get(CARD_DOC)?.parse().ok()?;
    let number = n.plugin_data.get(CARD_VERSION)?.parse().ok()?;
    Some((DocId(id), number))
}

/// Every card in the file.
pub fn cards(doc: &Document) -> Vec<NodeId> {
    doc.nodes().filter(|n| n.plugin_data.contains_key(CARD_DOC)).map(|n| n.id).collect()
}

/// Lay a card out again if its document's words changed since it was built, or always when `force`
/// (a card whose pages were deleted or moved by hand).
pub fn refresh_card(doc: &mut Document, card: NodeId, force: bool) -> Result<bool> {
    let Some((id, number)) = doc.node(card).and_then(card_target) else { return Ok(false) };
    // A card whose document or version is gone stays as it was: a record, not a link.
    let Some((d, v)) = doc.docs.get(id).and_then(|d| Some((d, d.version(number)?))) else { return Ok(false) };
    let hash = content_hash(d, v);
    if !force && doc.node(card).and_then(|n| n.plugin_data.get(CARD_HASH)).is_some_and(|h| *h == hash) {
        return Ok(false);
    }
    let l = layout(&info(d, v, code_of(v)), &v.blocks);
    let title = if d.title.trim().is_empty() { "Untitled" } else { d.title.trim() };
    let name = format!("{}{title} · v{}", if d.icon.is_empty() { String::new() } else { format!("{} ", d.icon) }, v.number);
    let name = if v.is_closed() { format!("{name} (closed)") } else { name };
    doc.remove_children(card).map_err(other)?;
    build_pages(doc, card, &l, (0.0, 0.0), CARD_GAP, true)?;
    let count = l.pages.len().max(1) as f64;
    if let Some(n) = doc.node_mut(card) {
        n.width = count * PAGE_W + (count - 1.0) * CARD_GAP;
        n.height = PAGE_H;
        n.name = name;
        n.plugin_data.insert(CARD_HASH.into(), hash);
    }
    Ok(true)
}

/// Bring the cards of document `only` (every card when `None`) up to date with their document.
pub fn refresh_cards(doc: &mut Document, only: Option<DocId>) {
    if doc.docs.is_empty() {
        return;
    }
    let shows = |c: NodeId| only.is_none_or(|d| doc.node(c).and_then(card_target).is_some_and(|(id, _)| id == d));
    let wanted: Vec<NodeId> = cards(doc).into_iter().filter(|c| shows(*c)).collect();
    for card in wanted {
        // Best effort: a card that cannot be rebuilt keeps its last look and never blocks an edit.
        let _ = refresh_card(doc, card, false);
    }
}
