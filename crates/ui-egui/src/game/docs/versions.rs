//! A document's versions in the editor (G13.4, G13.7): the bar under the title (which version, its
//! code, "For signature", Close version / New version), the notes on a closed, old or changed
//! version, the signature slots at the end of a version for signature, and the question before
//! closing.

use egui::{Align2, Color32, CornerRadius, Id, Rect, Sense, Stroke, Ui, pos2, vec2};
use figcraft_doc::docs::{BlockId, Doc, DocId, DocVersion};
use figcraft_doc::docs_version::{Signature, Signer};
use figcraft_engine::doc_versions::{code_of, intact, when};
use serde_json::json;

use super::sign;
use super::state::{Mode, SignDraft, State};
use crate::a11y;
use crate::i18n::{tr, trf};
use crate::theme::{Tokens, regular, semibold};
use crate::{FigApp, widgets};

/// What the editor shows of the version on screen, copied out of the file.
pub(super) struct Ver {
    pub number: u32,
    pub last: u32,
    /// Every version: its number, whether it is closed and its code.
    pub all: Vec<(u32, bool, Option<String>)>,
    pub closed: bool,
    pub closed_at: String,
    pub code: Option<String>,
    pub intact: bool,
    pub for_signature: bool,
    pub signers: Vec<Signer>,
    pub signatures: Vec<Signature>,
}

impl Ver {
    pub fn of(ctx: &egui::Context, uid: u64, doc: &Doc, v: &DocVersion) -> Self {
        Self {
            number: v.number,
            last: doc.current().map_or(v.number, |c| c.number),
            all: doc.versions.iter().map(|x| (x.number, x.is_closed(), code_of(x))).collect(),
            closed: v.is_closed(),
            closed_at: v.closed_at.clone().unwrap_or_default(),
            code: code_of(v),
            intact: still_intact(ctx, uid, doc.id, v),
            for_signature: v.for_signature,
            signers: v.signers.clone(),
            signatures: v.signatures.clone(),
        }
    }

    pub fn is_last(&self) -> bool {
        self.number == self.last
    }

    /// Whether this version can be written in: the last one, still a draft.
    pub fn editable(&self) -> bool {
        self.is_last() && !self.closed
    }

    /// Whether slot `i` can be signed now.
    pub fn can_sign(&self, i: usize) -> bool {
        self.closed && self.intact && !self.signatures.iter().any(|g| g.slot == i)
    }
}

/// Seconds an answer of [`still_intact`] is kept.
const RECHECK: f64 = 1.0;

/// Whether a closed version still matches its fingerprint. Hashing a long version every frame costs
/// real time, so the answer is kept for a second per file, document and version: a file reverted to
/// one changed on disk is caught a moment later.
fn still_intact(ctx: &egui::Context, uid: u64, doc: DocId, v: &DocVersion) -> bool {
    if !v.is_closed() {
        return true;
    }
    // A closed version without a fingerprint (a hand-edited file) is not intact either.
    let seal = v.fingerprint.as_deref().unwrap_or_default();
    let key = Id::unique(("gbx_doc_intact", uid, doc.0, v.number, seal));
    let now = ctx.input(|i| i.time);
    if let Some((ok, at)) = ctx.data(|d| d.get_temp::<(bool, f64)>(key))
        && (0.0..RECHECK).contains(&(now - at))
    {
        return ok;
    }
    let ok = intact(v);
    ctx.data_mut(|d| d.insert_temp(key, (ok, now)));
    ok
}

/// The version bar under the title.
pub(super) fn bar(app: &mut FigApp, ui: &mut Ui, st: &mut State, t: &Tokens, doc: DocId, ver: &Ver) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        let state = if ver.closed { tr("closed") } else { tr("draft") };
        let label = trf("Version {n} · {state}", &[("n", &ver.number), ("state", &state)]);
        let w = ui.painter().layout_no_wrap(label.clone(), regular(11.0), t.text).size().x + 40.0;
        let resp = widgets::dropdown(ui, &label, w, None);
        let menu = Id::unique(("gbx_doc_versions", doc.0));
        crate::right::menu_popup(&resp).id(menu).show(|ui| {
            use crate::right::{menu_check, menu_item};
            ui.set_min_width(240.0);
            for (n, closed, code) in ver.all.iter().rev() {
                let state = if *closed { tr("closed") } else { tr("draft") };
                let row = match code {
                    Some(c) => format!("{} · {c}", trf("Version {n} · {state}", &[("n", n), ("state", &state)])),
                    None => trf("Version {n} · {state}", &[("n", n), ("state", &state)]),
                };
                if menu_check(ui, *n == ver.number && st.mode == Mode::Write, &row, "") {
                    st.viewing = (*n != ver.last).then_some((doc, *n));
                    st.mode = Mode::Write;
                    ui.close();
                }
            }
            ui.separator();
            if ver.all.len() > 1 && menu_item(ui, tr("Compare with the version before"), "") {
                let to = if ver.number > 1 { ver.number } else { ver.last };
                super::compare::open_diff(app, st, doc, to);
                ui.close();
            }
            if menu_item(ui, tr("Decision log"), "") {
                super::compare::open_decisions(app, st, doc);
                ui.close();
            }
        });
        if let Some(code) = &ver.code {
            let r = ui.label(egui::RichText::new(code).font(semibold(11.0)).color(if ver.intact { t.text_secondary } else { t.danger }));
            r.on_hover_text(tr("The version's code: it is on every page of its PDF"));
        }
        if ver.editable() {
            ui.add_space(6.0);
            if switch(ui, t, ver.for_signature, tr("For signature")) {
                app.run("doc.version.for_signature", json!({"doc": doc.0, "on": !ver.for_signature}));
            }
        }
        // The next step for this version, at the right end.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if !ver.is_last() {
                return;
            }
            if !ver.closed {
                if widgets::button(ui, tr("Close version…"), false, true).clicked() {
                    st.confirm_close = Some(doc);
                }
            } else if widgets::button(ui, tr("New version"), false, true).clicked() && app.run("doc.version.new", json!({"doc": doc.0})).is_some() {
                st.viewing = None;
            }
        });
    });
}

/// A switch with its label; true when clicked.
fn switch(ui: &mut Ui, t: &Tokens, on: bool, label: &str) -> bool {
    let galley = ui.painter().layout_no_wrap(label.to_string(), regular(11.0), t.text);
    let (r, resp) = ui.allocate_exact_size(vec2(30.0 + 8.0 + galley.size().x, 24.0), Sense::click());
    a11y::toggle(&resp, label, on);
    let track = Rect::from_min_size(pos2(r.min.x, r.center().y - 8.0), vec2(28.0, 16.0));
    let fill = if on {
        t.accent
    } else if resp.hovered() {
        t.border_strong
    } else {
        t.border
    };
    ui.painter().rect_filled(track, CornerRadius::same(8), fill);
    let knob = if on { track.max.x - 8.0 } else { track.min.x + 8.0 };
    ui.painter().circle_filled(pos2(knob, track.center().y), 6.0, Color32::WHITE);
    ui.painter().galley(pos2(track.max.x + 8.0, r.center().y - galley.size().y / 2.0), galley, t.text);
    resp.clicked()
}

/// A note in a soft box under the bar: `title` in bold, then `text`, in `ink`.
fn note(ui: &mut Ui, t: &Tokens, ink: Color32, title: &str, text: &str, action: Option<&str>) -> bool {
    let mut clicked = false;
    egui::Frame::NONE
        .fill(ink.gamma_multiply(if t.dark { 0.16 } else { 0.09 }))
        .stroke(Stroke::new(1.0, ink.gamma_multiply(0.35)))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(egui::Margin::symmetric(14, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.set_width((ui.available_width() - if action.is_some() { 150.0 } else { 0.0 }).max(120.0));
                    ui.label(egui::RichText::new(title).font(semibold(12.0)).color(t.text));
                    ui.label(egui::RichText::new(text).font(regular(11.5)).color(t.text_secondary));
                });
                if let Some(a) = action {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        clicked = widgets::button(ui, a, false, true).clicked();
                    });
                }
            });
        });
    clicked
}

/// The notes on a version that cannot be written in: an older one, a closed one, one changed
/// outside Graybox.
pub(super) fn notes(ui: &mut Ui, st: &mut State, t: &Tokens, ver: &Ver) {
    let code = ver.code.clone().unwrap_or_default();
    if ver.closed && !ver.intact {
        ui.add_space(12.0);
        note(
            ui,
            t,
            t.danger,
            tr("Changed outside Graybox"),
            &trf(
                "This version no longer matches its fingerprint {code}: the file was edited by another program. It cannot be signed.",
                &[("code", &code)],
            ),
            None,
        );
    }
    if !ver.is_last() {
        ui.add_space(12.0);
        let go = trf("Go to version {n}", &[("n", &ver.last)]);
        if note(ui, t, t.accent, tr("An earlier version"), tr("You are reading an earlier version. Only the last version can change."), Some(&go)) {
            st.viewing = None;
        }
    } else if ver.closed && ver.intact {
        ui.add_space(12.0);
        let text = trf(
            "Closed {when}. It can no longer change; every page of its PDF carries the code {code}.",
            &[("when", &ver.closed_at), ("code", &code)],
        );
        note(ui, t, t.success, tr("This version is closed"), &text, None);
    }
}

/// How tall a signature slot is in the editor.
const SLOT_H: f32 = 132.0;
/// A slot with fields (on a draft) is taller: the name and role fields, then the date.
const SLOT_EDIT_H: f32 = 152.0;
const MARK_H: f32 = 64.0;

/// The signature section at the end of a version for signature: a slot per signer, two by two.
pub(super) fn signatures(app: &mut FigApp, ui: &mut Ui, st: &mut State, t: &Tokens, doc: DocId, ver: &Ver) {
    if !ver.for_signature {
        return;
    }
    ui.add_space(28.0);
    ui.label(egui::RichText::new(tr("Signatures")).font(semibold(18.0)).color(t.text));
    ui.add_space(10.0);
    let gap = 24.0;
    let w = ((ui.available_width() - gap) / 2.0).max(160.0);
    let mut signers = ver.signers.clone();
    let mut changed = false;
    for (row, pair) in ver.signers.chunks(2).enumerate() {
        let h = if ver.editable() { SLOT_EDIT_H } else { SLOT_H };
        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::hover());
        for (k, s) in pair.iter().enumerate() {
            let i = row * 2 + k;
            let cell = Rect::from_min_size(pos2(r.min.x + k as f32 * (w + gap), r.min.y), vec2(w, h));
            let sig = ver.signatures.iter().find(|g| g.slot == i);
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(cell).id_salt(("gbx_doc_slot", doc.0, ver.number, i)));
            if let Some(new) = slot(&mut child, st, t, doc, ver, i, s, sig, ver.signers.len() > 1)
                && let Some(x) = signers.get_mut(i)
            {
                match new {
                    Some(n) => *x = n,
                    None => {
                        signers.remove(i);
                    }
                }
                changed = true;
                break;
            }
        }
        if changed {
            break;
        }
        ui.add_space(12.0);
    }
    if ver.editable() && ver.signers.len() < figcraft_doc::docs_version::MAX_SIGNERS {
        let resp = ui.add(
            egui::Label::new(egui::RichText::new(format!("+ {}", tr("Add a signer"))).font(regular(12.0)).color(t.accent_text)).sense(Sense::click()),
        );
        a11y::button(&resp, tr("Add a signer"));
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if resp.clicked() {
            signers.push(Signer::default());
            changed = true;
        }
    }
    if changed {
        let list: Vec<_> = signers.iter().map(|s| json!({"name": s.name, "role": s.role})).collect();
        app.run("doc.version.signers", json!({"doc": doc.0, "signers": list}));
    }
}

/// One slot: the mark (or what to do to get one), the line, the name, the role and the date. On a
/// draft the name and role are fields. Returns a change: `Some(Some(signer))` edited,
/// `Some(None)` removed.
#[allow(clippy::too_many_arguments)]
fn slot(
    ui: &mut Ui,
    st: &mut State,
    t: &Tokens,
    doc: DocId,
    ver: &Ver,
    i: usize,
    s: &Signer,
    sig: Option<&Signature>,
    removable: bool,
) -> Option<Option<Signer>> {
    let cell = ui.max_rect();
    let mark = Rect::from_min_size(cell.min, vec2(cell.width(), MARK_H));
    let hovered = ui.rect_contains_pointer(cell);
    let mut out = None;
    match sig {
        Some(g) => sign::paint_mark(ui, mark.shrink2(vec2(6.0, 4.0)), &g.mark, t.text),
        None if ver.can_sign(i) => {
            let b = Rect::from_center_size(mark.center(), vec2(120.0, 30.0));
            let resp = ui.interact(b, Id::unique(("gbx_doc_sign", doc.0, ver.number, i)), Sense::click());
            a11y::button(&resp, tr("Sign…"));
            ui.painter().rect_filled(b, CornerRadius::same(6), if resp.hovered() { t.accent_hover } else { t.accent });
            ui.painter().text(b.center(), Align2::CENTER_CENTER, tr("Sign…"), semibold(12.0), t.accent_on);
            if resp.clicked() {
                st.sign = Some(SignDraft { doc, version: ver.number, slot: i, name: s.name.clone(), role: s.role.clone(), ..SignDraft::default() });
            }
        }
        None if !ver.closed => {
            ui.painter().text(
                pos2(mark.min.x + 2.0, mark.max.y - 10.0),
                Align2::LEFT_CENTER,
                tr("Signed once the version is closed"),
                regular(11.0),
                t.text_tertiary,
            );
        }
        None => {}
    }
    let line_y = mark.max.y + 4.0;
    ui.painter().line_segment([pos2(cell.min.x, line_y), pos2(cell.max.x, line_y)], Stroke::new(1.0, t.text_secondary));
    let field_w = cell.width();
    let mut y = line_y + 8.0;
    if ver.editable() {
        let edit = |ui: &mut Ui, key: &str, value: &str, hint: &str, y: f32| {
            let r = Rect::from_min_size(pos2(cell.min.x, y), vec2(field_w, 24.0));
            let mut c = ui.new_child(egui::UiBuilder::new().max_rect(r));
            widgets::text_field(&mut c, Id::unique(("gbx_doc_signer", doc.0, i, key)), value, field_w, hint)
        };
        if let Some(n) = edit(ui, "name", &s.name, tr("Name"), y) {
            out = Some(Some(Signer { name: n, ..s.clone() }));
        }
        y += 28.0;
        if let Some(r) = edit(ui, "role", &s.role, tr("Role"), y) {
            out = Some(Some(Signer { role: r, ..s.clone() }));
        }
        if removable && hovered {
            let x = Rect::from_min_size(pos2(cell.max.x - 22.0, cell.min.y), vec2(22.0, 22.0));
            let resp = ui.interact(x, Id::unique(("gbx_doc_signer_remove", doc.0, i)), Sense::click());
            a11y::button(&resp, tr("Remove this signer"));
            let resp = resp.on_hover_text(tr("Remove this signer"));
            if resp.hovered() {
                ui.painter().rect_filled(x, CornerRadius::same(4), t.hover);
            }
            let c = x.center();
            let ink = Stroke::new(1.3, t.icon_secondary);
            ui.painter().line_segment([c + vec2(-4.0, -4.0), c + vec2(4.0, 4.0)], ink);
            ui.painter().line_segment([c + vec2(4.0, -4.0), c + vec2(-4.0, 4.0)], ink);
            if resp.clicked() {
                out = Some(None);
            }
        }
    } else {
        let name = if s.name.trim().is_empty() { sig.map_or("", |g| g.name.as_str()) } else { s.name.as_str() };
        let (name, font, ink) = if name.trim().is_empty() { (tr("Name"), regular(12.0), t.text_tertiary) } else { (name, semibold(12.0), t.text) };
        ui.painter().text(pos2(cell.min.x, y + 8.0), Align2::LEFT_CENTER, name, font, ink);
        y += 22.0;
        if !s.role.trim().is_empty() {
            ui.painter().text(pos2(cell.min.x, y + 6.0), Align2::LEFT_CENTER, &s.role, regular(11.0), t.text_secondary);
        }
    }
    date_line(ui, t, sig, pos2(cell.min.x, line_y + if ver.editable() { 74.0 } else { 56.0 }));
    out
}

/// A Signature block placed anywhere in the document (G13.7): the slot of the signer whose role (or
/// name) it names, with their mark once they signed. On a draft its name and role are fields.
#[allow(clippy::too_many_arguments)]
pub(super) fn signature_block(app: &mut FigApp, ui: &mut Ui, t: &Tokens, doc: DocId, ver: &Ver, block: BlockId, name: &str, role: &str, indent: f32) {
    let same = |a: &str, b: &str| !a.trim().is_empty() && a.trim().eq_ignore_ascii_case(b.trim());
    let slot = ver.signers.iter().position(|s| same(&s.role, role)).or_else(|| ver.signers.iter().position(|s| same(&s.name, name)));
    let sig = slot.and_then(|i| ver.signatures.iter().find(|g| g.slot == i));
    ui.add_space(8.0);
    let w = (ui.available_width() - indent).clamp(160.0, 280.0);
    let h = if ver.editable() { SLOT_EDIT_H } else { SLOT_H };
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::hover());
    let cell = Rect::from_min_size(pos2(r.min.x + indent, r.min.y), vec2(w, r.height()));
    let mark = Rect::from_min_size(cell.min, vec2(w, MARK_H));
    if let Some(g) = sig {
        sign::paint_mark(ui, mark.shrink2(vec2(6.0, 4.0)), &g.mark, t.text);
    }
    let line_y = mark.max.y + 4.0;
    ui.painter().line_segment([pos2(cell.min.x, line_y), pos2(cell.max.x, line_y)], Stroke::new(1.0, t.text_secondary));
    let y = line_y + 8.0;
    if ver.editable() {
        let field = |ui: &mut Ui, key: &str, value: &str, hint: &str, y: f32| {
            let fr = Rect::from_min_size(pos2(cell.min.x, y), vec2(w, 24.0));
            let mut c = ui.new_child(egui::UiBuilder::new().max_rect(fr));
            widgets::text_field(&mut c, Id::unique(("gbx_doc_sigblock", doc.0, block.0, key)), value, w, hint)
        };
        if let Some(n) = field(ui, "name", name, tr("Name"), y) {
            app.run("doc.block.set", json!({"doc": doc.0, "id": block.0, "text": n}));
        }
        if let Some(role) = field(ui, "role", role, tr("Role"), y + 28.0) {
            app.run("doc.block.turn_into", json!({"doc": doc.0, "id": block.0, "kind": {"type": "signature", "role": role}}));
        }
    } else {
        let (shown, font, ink) = if name.trim().is_empty() { (tr("Name"), regular(12.0), t.text_tertiary) } else { (name, semibold(12.0), t.text) };
        ui.painter().text(pos2(cell.min.x, y + 8.0), Align2::LEFT_CENTER, shown, font, ink);
        if !role.trim().is_empty() {
            ui.painter().text(pos2(cell.min.x, y + 28.0), Align2::LEFT_CENTER, role, regular(11.0), t.text_secondary);
        }
    }
    date_line(ui, t, sig, pos2(cell.min.x, line_y + if ver.editable() { 74.0 } else { 56.0 }));
}

/// Under a slot, left-centred on `at`: when it was signed, or "Date" and a line to write it on.
fn date_line(ui: &Ui, t: &Tokens, sig: Option<&Signature>, at: egui::Pos2) {
    match sig {
        Some(g) => {
            ui.painter().text(at, Align2::LEFT_CENTER, trf("Signed {when}", &[("when", &when(g.signed_at))]), regular(11.0), t.text_tertiary);
        }
        None => {
            let r = ui.painter().text(at, Align2::LEFT_CENTER, tr("Date"), regular(11.0), t.text_tertiary);
            let y = r.max.y - 1.0;
            ui.painter().line_segment([pos2(r.max.x + 6.0, y), pos2(r.max.x + 126.0, y)], Stroke::new(1.0, t.border_strong));
        }
    }
}

/// The question before closing the last version.
pub(super) fn confirm_close(app: &mut FigApp, ctx: &egui::Context, st: &mut State) {
    let Some(doc) = st.confirm_close else { return };
    let Some(number) = app.session.doc().ok().and_then(|d| d.docs.get(doc)).and_then(Doc::current).filter(|v| !v.is_closed()).map(|v| v.number)
    else {
        st.confirm_close = None;
        return;
    };
    let mut open = true;
    let mut decided = None;
    let title = trf("Close version {n}?", &[("n", &number)]);
    crate::dialogs::window(&title, ctx).open(&mut open).show(ctx, |ui| {
        let t = Tokens::get(ui.ctx());
        ui.set_width(340.0);
        ui.label(
            egui::RichText::new(tr(
                "A closed version never changes: Graybox records its fingerprint and puts its code on every page of the PDF. To change it later, start a new version.",
            ))
            .font(regular(11.0))
            .color(t.text_secondary),
        );
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            if widgets::button(ui, tr("Cancel"), false, true).clicked() {
                decided = Some(false);
            }
            if widgets::button(ui, tr("Close version"), true, true).clicked() {
                decided = Some(true);
            }
        });
    });
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        decided = Some(false);
    }
    if decided == Some(true) && app.run("doc.version.close", json!({"doc": doc.0})).is_some() {
        st.viewing = None;
    }
    if decided.is_some() || !open {
        st.confirm_close = None;
    }
}
