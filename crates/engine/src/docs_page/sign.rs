//! Signatures on the page (G13.4, G13.7): the signature section of a version "for signature", a
//! Signature block placed anywhere, the marks (drawn strokes or a name in a handwriting font) and the
//! certificate page that ends the PDF of a signed version.

use figcraft_doc::docs::Block;
use figcraft_doc::docs_version::{Mark, Signature, Signer};

use super::{CONTENT_W, INK, Info, Item, Look, MARGIN_X, MUTED, Pager, RULE, SEAL, SOFT, measure, plain, text_data};

/// Handwriting fonts for a typed signature, by style, best first. None ships with Graybox: the first
/// one installed on this computer (or loaded by the person) is used, else a clean italic.
const HANDS: [&[&str]; 3] = [
    &["Dancing Script", "Segoe Script", "Brush Script MT", "Snell Roundhand"],
    &["Caveat", "Ink Free", "Segoe Print", "Bradley Hand", "Kalam"],
    &["Great Vibes", "Lucida Handwriting", "Gabriola", "Apple Chancery", "Parisienne"],
];

/// The font a typed signature in `style` is drawn with, and whether it is a real handwriting font
/// (else the body font in italic).
pub fn hand_font(style: u8) -> (String, bool) {
    let list = HANDS.get(usize::from(style)).copied().unwrap_or(&[]);
    match list.iter().find(|f| figcraft_text::has_family(f)) {
        Some(f) => ((*f).to_string(), true),
        None => ("Inter".to_string(), false),
    }
}

/// One slot of the signature section: who signs, and their signature once given.
struct Slot<'a> {
    name: &'a str,
    role: &'a str,
    sig: Option<&'a Signature>,
}

fn slots<'a>(signers: &'a [Signer], signatures: &'a [Signature]) -> Vec<Slot<'a>> {
    let mut out: Vec<Slot> =
        signers.iter().enumerate().map(|(i, s)| Slot { name: &s.name, role: &s.role, sig: signatures.iter().find(|g| g.slot == i) }).collect();
    if out.is_empty()
        && let Some(g) = signatures.first()
    {
        out.push(Slot { name: &g.name, role: &g.role, sig: Some(g) });
    }
    out
}

/// `YYYY-MM-DD HH:MM UTC`.
pub fn when(secs: u64) -> String {
    figcraft_collab::message::utc_date(secs)
}

/// The height of a slot: the mark, the line, the name, role and date under it.
const SLOT_H: f64 = 100.0;
const MARK_H: f64 = 54.0;

/// The mark of `sig` in the box at (`x`, `y`), `w` by `h`, standing on its bottom edge.
fn mark(p: &mut Pager, sig: &Signature, x: f64, y: f64, w: f64, h: f64) {
    match &sig.mark {
        Mark::Drawn { strokes } => {
            // The strokes were drawn in a 3:1 box: as big as fits, on the line.
            let bw = w.min(h * 3.0);
            let bh = bw / 3.0;
            let (ox, oy) = (x, y + h - bh);
            let pts: Vec<Vec<(f64, f64)>> =
                strokes.iter().map(|s| s.iter().map(|q| (ox + f64::from(q[0]) * bw, oy + f64::from(q[1]) * bh)).collect()).collect();
            p.push(Item::Ink { x: ox, y: oy, w: bw, h: bh, strokes: pts, color: INK, width: 1.6 });
        }
        Mark::Typed { text, style } => {
            let (family, real) = hand_font(*style);
            let look = Look { family: Some(family), italic: !real, color: INK, ..Look::sized(if real { 24.0 } else { 20.0 }, false) };
            let data = text_data(&plain(text), &look);
            let th = measure(&data, w).height;
            p.push(Item::Text { x, y: y + (h - th).max(0.0), w, h: th, data: Box::new(data) });
        }
    }
}

/// A slot at (`x`, `p.y`), `w` wide: the mark when signed, the line, then name, role and date.
fn slot(p: &mut Pager, s: &Slot, x: f64, w: f64) {
    let y = p.y;
    if let Some(sig) = s.sig {
        mark(p, sig, x + 4.0, y, w - 8.0, MARK_H);
    }
    p.push(Item::rect(x, y + MARK_H + 4.0, w, 0.9, INK));
    let name = if s.name.trim().is_empty() { s.sig.map_or("", |g| g.name.as_str()) } else { s.name };
    // No name yet: a quiet label over the line, to be written in by hand.
    let (name, look) = if name.trim().is_empty() { ("Name", Look::muted(10.0)) } else { (name, Look::sized(10.0, true)) };
    let mut line = |text: &str, dy: f64, look: Look| {
        let data = text_data(&plain(text), &look);
        let th = measure(&data, w).height;
        p.push(Item::Text { x, y: y + dy, w, h: th, data: Box::new(data) });
    };
    line(name, MARK_H + 9.0, look);
    if !s.role.trim().is_empty() {
        line(s.role, MARK_H + 23.0, Look::muted(9.0));
    }
    let date = s.sig.map_or_else(|| "Date: ____ / ____ / ________".to_string(), |g| format!("Signed {}", when(g.signed_at)));
    line(&date, MARK_H + 36.0, Look::muted(8.5));
}

/// The signature section at the end of a version marked for signature: a heading, then the slots
/// two by two.
pub(super) fn section(p: &mut Pager, info: &Info) {
    if !info.for_signature || p.full {
        return;
    }
    let all = slots(info.signers, info.signatures);
    if all.is_empty() {
        return;
    }
    p.gap(28.0);
    // The heading never ends a page without the first row of slots.
    p.need(30.0 + SLOT_H);
    let data = text_data(&plain("Signatures"), &Look::sized(13.5, true));
    p.text(data, MARGIN_X, CONTENT_W);
    p.y += 14.0;
    let col = (CONTENT_W - 32.0) / 2.0;
    for row in all.chunks(2) {
        p.need(SLOT_H);
        for (i, s) in row.iter().enumerate() {
            slot(p, s, MARGIN_X + i as f64 * (col + 32.0), col);
        }
        p.y += SLOT_H + 10.0;
    }
}

/// A Signature block placed anywhere: the slot whose role (or name) it names, with its mark once signed.
pub(super) fn block(p: &mut Pager, b: &Block, role: &str, x: f64, w: f64) {
    p.gap(14.0);
    p.need(SLOT_H);
    let name = b.plain_text();
    let (signers, signatures) = (std::mem::take(&mut p.signers), std::mem::take(&mut p.signatures));
    {
        let all = slots(&signers, &signatures);
        let same = |a: &str, b: &str| !a.trim().is_empty() && a.trim().eq_ignore_ascii_case(b.trim());
        let found = all.iter().find(|s| same(s.role, role)).or_else(|| all.iter().find(|s| same(s.name, &name)));
        let shown = Slot {
            name: if name.trim().is_empty() { found.map_or("", |s| s.name) } else { &name },
            role: if role.trim().is_empty() { found.map_or("Signature", |s| s.role) } else { role },
            sig: found.and_then(|s| s.sig),
        };
        slot(p, &shown, x, w.min(260.0));
    }
    (p.signers, p.signatures) = (signers, signatures);
    p.y += SLOT_H;
}

/// The certificate page of a closed version that is for signature or signed: what was signed (the
/// fingerprint), who signed and when, and the line for the client's gov.br co-signature.
pub(super) fn certificate(p: &mut Pager, info: &Info) {
    if !info.closed || !(info.for_signature || !info.signatures.is_empty()) || p.full {
        return;
    }
    p.page_break();
    if p.full {
        return;
    }
    let text = |p: &mut Pager, t: &str, look: Look| {
        p.text(text_data(&plain(t), &look), MARGIN_X, CONTENT_W);
    };
    text(p, "Signature certificate", Look::sized(20.0, true));
    p.y += 4.0;
    let title = if info.title.trim().is_empty() { "Untitled" } else { info.title.trim() };
    text(p, &format!("{title} · version {}", info.number), Look::muted(10.0));
    p.y += 12.0;
    p.push(Item::rect(MARGIN_X, p.y, CONTENT_W, 0.75, RULE));
    p.y += 16.0;
    // What was signed.
    let y0 = p.y;
    let box_h = 84.0;
    p.push(Item::Shape { x: MARGIN_X, y: y0, w: CONTENT_W, h: box_h, ellipse: false, fill: Some(SOFT), stroke: None, radius: 6.0 });
    p.y += 12.0;
    let inner = |p: &mut Pager, t: &str, look: Look| {
        p.text(text_data(&plain(t), &look), MARGIN_X + 14.0, CONTENT_W - 28.0);
    };
    inner(p, info.code.unwrap_or("—"), Look { color: SEAL, ..Look::sized(16.0, true) });
    p.y += 4.0;
    inner(p, &format!("SHA-256 {}", info.fingerprint.unwrap_or("—")), Look { family: Some("Courier New".into()), ..Look::muted(7.5) });
    p.y += 2.0;
    inner(p, &format!("Closed {}", info.closed_at.unwrap_or("—")), Look::muted(9.0));
    p.y = y0 + box_h + 18.0;
    text(
        p,
        "The code and the SHA-256 fingerprint identify this exact version: any change to its content gives a different fingerprint.",
        Look::muted(9.0),
    );
    p.y += 18.0;
    text(p, "Signers", Look::sized(13.0, true));
    p.y += 10.0;
    for s in slots(info.signers, info.signatures) {
        p.need(SLOT_H);
        slot(p, &s, MARGIN_X, 240.0);
        let how = match s.sig {
            Some(g) if g.via == "link" => "Signed through a Graybox link".to_string(),
            Some(_) => "Signed in Graybox on this computer".to_string(),
            None => "Not signed yet".to_string(),
        };
        let data = text_data(&plain(&how), &Look::muted(9.0));
        let th = measure(&data, CONTENT_W - 270.0).height;
        p.push(Item::Text { x: MARGIN_X + 270.0, y: p.y + MARK_H - th, w: CONTENT_W - 270.0, h: th, data: Box::new(data) });
        p.y += SLOT_H + 8.0;
    }
    // The client's co-signature through gov.br, until signing through a link (G15).
    p.need(110.0);
    p.y += 8.0;
    text(p, "Client co-signature (gov.br)", Look::sized(13.0, true));
    p.y += 6.0;
    text(
        p,
        "The client can co-sign this PDF with their gov.br account at assinador.iti.br. Check that the code above matches the code in every page footer.",
        Look::muted(9.0),
    );
    p.y += 30.0;
    p.push(Item::rect(MARGIN_X, p.y, 260.0, 0.9, INK));
    p.y += 6.0;
    text(p, "Client", Look { color: MUTED, ..Look::muted(9.0) });
}
