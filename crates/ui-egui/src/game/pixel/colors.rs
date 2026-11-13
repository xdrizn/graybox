//! Pixel colours, with Graybox's own colour system (G16.6): the tools' two colours and a swap in
//! the Pixel colors section are edited in the standard colour picker
//! ([`PickerTarget::Pixel`](crate::colorpicker::PickerTarget::Pixel)), which here also offers the
//! file's colour styles and colour variables (the colour then remembers where it came from) and
//! the sprite's own colours.
//!
//! The Pixel colors section (under Selection colours) lists the colours the selected pixel layers
//! (and pixel layers inside the selected frames) are drawn with. Changing one replaces it in every
//! layer and frame of those sprites, as one undo step; colours painted from a style that has
//! changed since can be brought up to date.

use std::collections::BTreeMap;
use std::sync::Arc;

use egui::{Color32, CornerRadius, Sense, Stroke, Ui, vec2};
use figcraft_color::{Color, Paint};
use figcraft_doc::{Document, NodeId, StyleKind, VariableType, VariableValue};
use figcraft_engine::cmd::pixel::link_color;
use figcraft_pixel::{Rgba, canvas, store};
use serde_json::json;

use crate::FigApp;
use crate::colorpicker::PickerTarget;
use crate::theme::Tokens;
use crate::widgets;

/// The most colours the section lists.
const MAX_ROWS: usize = 32;
/// The picker slot of the Pixel colors swap (0 and 1 are the tools' colours).
pub(crate) const SWAP: usize = 2;
/// The most styles or variables the picker lists.
const MAX_LISTED: usize = 256;

/// The pixel layers among `sel` and inside it.
fn pixel_layers(doc: &Document, sel: &[NodeId]) -> Vec<NodeId> {
    let mut out = Vec::new();
    let mut todo: Vec<NodeId> = sel.iter().rev().copied().collect();
    let mut seen = std::collections::BTreeSet::new();
    while let Some(id) = todo.pop() {
        if out.len() >= 256 || seen.len() >= 100_000 {
            break;
        }
        if !seen.insert(id) {
            continue;
        }
        let Some(n) = doc.node(id) else { continue };
        // An instance shows its main's pixels; its own copy isn't drawn, so it is left out.
        if store::is_pixel(&n.plugin_data) && !matches!(n.kind, figcraft_doc::NodeKind::Instance { .. }) {
            out.push(id);
        }
        todo.extend(n.children.iter().rev().copied());
    }
    out
}

/// The colours of those layers' sprites (every layer and frame), most used first.
fn colors_of(doc: &Document, layers: &[NodeId]) -> Vec<Rgba> {
    let mut count: BTreeMap<Rgba, usize> = BTreeMap::new();
    for id in layers {
        let Some(n) = doc.node(*id) else { continue };
        let Some(sprite) = store::load(&n.plugin_data) else { continue };
        for l in &sprite.layers {
            for f in &sprite.frames {
                if !n.plugin_data.contains_key(&store::cel_key(l.id, f.id)) {
                    continue;
                }
                let Some(cel) = store::cel(&n.plugin_data, &sprite, l.id, f.id) else { continue };
                for p in cel.pixels().iter().filter(|p| p[3] > 0) {
                    *count.entry(*p).or_default() += 1;
                }
            }
        }
    }
    let mut v: Vec<(Rgba, usize)> = count.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    v.into_iter().map(|(c, _)| c).collect()
}

/// What the layers' colours came from (colour → link), and how many links are behind their style.
fn links_of(doc: &Document, layers: &[NodeId]) -> (BTreeMap<Rgba, String>, usize) {
    let mut out = BTreeMap::new();
    let mut stale = 0;
    for id in layers {
        let Some(sprite) = doc.node(*id).and_then(|n| store::load(&n.plugin_data)) else { continue };
        for (hex, link) in &sprite.links {
            let Some(c) = canvas::parse_hex(hex) else { continue };
            if link_color(doc, link) != Some(c) {
                stale += 1;
            }
            out.insert(c, link.clone());
        }
    }
    (out, stale)
}

/// The name of what `link` points at (a colour style or variable), when it is still there.
pub(crate) fn link_name(doc: &Document, link: &str) -> Option<String> {
    if let Some(id) = link.strip_prefix("style:") {
        return doc.style(id).map(|s| s.name.clone());
    }
    let id = link.strip_prefix("var:")?;
    doc.variables.get_by_id(id).map(|v| v.name.clone())
}

/// A swap the Pixel colors section has open in the picker: the colour, what it becomes, the
/// layers it changes and (one component selected) the component it can become a variant of.
#[derive(Clone, Debug)]
pub(crate) struct Swap {
    pub(crate) from: Rgba,
    pub(crate) to: Rgba,
    layers: Vec<NodeId>,
    component: Option<NodeId>,
}

fn swap_open(app: &FigApp) -> bool {
    matches!(app.ui.picker, Some(PickerTarget::Pixel { which: SWAP }))
}

/// The section, when the selection holds pixel layers. Returns whether it drew.
pub(crate) fn section(app: &mut FigApp, ui: &mut Ui, doc: &Arc<Document>, sel: &[NodeId]) -> bool {
    // Worked out again only when the document or the selection changes.
    let (layers, colors, (linked, stale)) = crate::selection_cache::cached(ui.ctx(), "graybox_pixel_colors", doc, sel, &[], || {
        let layers = pixel_layers(doc, sel);
        let colors = colors_of(doc, &layers);
        let links = links_of(doc, &layers);
        (layers, colors, links)
    });
    if layers.is_empty() || colors.is_empty() {
        return false;
    }
    let t = Tokens::get(ui.ctx());
    widgets::section_header(ui, "Pixel colors", &[], false);
    // A swap whose picker closed (a click elsewhere) is over.
    if !swap_open(app) {
        app.pixel.swap = None;
    }
    let open = app.pixel.swap.clone();
    let more = colors.len().saturating_sub(MAX_ROWS);
    // One component selected: a swap can also become a new variant of it.
    let component = match sel {
        [one] if doc.node(*one).is_some_and(|n| matches!(n.kind, figcraft_doc::NodeKind::Component { .. })) => Some(*one),
        _ => None,
    };
    super::panel::padded(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
            for c in colors.iter().take(MAX_ROWS) {
                let (r, resp) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
                let editing = open.as_ref().is_some_and(|s| s.from == *c);
                let shown = open.as_ref().filter(|s| s.from == *c).map_or(*c, |s| s.to);
                ui.painter().rect_filled(r, CornerRadius::same(4), c32(shown));
                ui.painter().rect_stroke(
                    r,
                    CornerRadius::same(4),
                    Stroke::new(if editing { 2.0 } else { 1.0 }, if editing { t.accent } else { t.border_strong }),
                    egui::StrokeKind::Inside,
                );
                let from = linked.get(c).and_then(|l| link_name(doc, l)).map(|n| format!("  ·  from {n}")).unwrap_or_default();
                if resp.on_hover_text(format!("{}{from}  ·  click to change it in the art", canvas::to_hex(*c))).clicked() {
                    if editing {
                        crate::colorpicker::close(app);
                    } else {
                        app.pixel.swap = Some(Swap { from: *c, to: *c, layers: layers.clone(), component });
                        app.pixel.links[SWAP] = linked.get(c).cloned();
                        app.open_picker(PickerTarget::Pixel { which: SWAP }, Some(r));
                    }
                }
            }
        })
    });
    if more > 0 {
        widgets::note(ui, &format!("and {more} more"), t.text_secondary);
    }
    if stale > 0 {
        widgets::note(ui, "Colour styles used here have changed since they were painted", t.text_secondary);
        super::panel::padded(ui, |ui| {
            if super::panel::text_button(ui, "Update from styles", "Repaint those pixels with the styles' colours now", &t, true) {
                let ids: Vec<u64> = layers.iter().map(|id| id.0).collect();
                app.run("pixel.styles.follow", json!({"ids": ids}));
            }
        });
    }
    true
}

fn c32(c: Rgba) -> Color32 {
    Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3])
}

/// `c` as the picker's paint.
fn paint_of(c: Rgba) -> Paint {
    let col = Color::from_rgba8(c[0], c[1], c[2], c[3]);
    let mut p = Paint::solid(col.with_alpha(1.0));
    p.opacity = col.a;
    p
}

/// The paint the picker shows for pixel colour `which` (None closes it: Pixel mode is off, or
/// there is no swap open).
pub(crate) fn picker_paint(app: &FigApp, which: usize) -> Option<Paint> {
    let c = match which {
        0 | 1 if super::active(app) => *app.pixel.colors.get(which)?,
        SWAP => app.pixel.swap.as_ref()?.to,
        _ => return None,
    };
    Some(paint_of(c))
}

/// The picker set pixel colour `which` by hand: it no longer comes from a style or variable.
pub(crate) fn picker_write(app: &mut FigApp, which: usize, paint: &Paint) {
    let Some(c) = paint.solid_color() else { return };
    let a = (c.a * paint.opacity).clamp(0.0, 1.0);
    set(app, which, c.with_alpha(a).to_rgba8(), None);
}

/// Pixel colour `which` becomes `c`, from `link` (a colour style or variable) or by hand.
fn set(app: &mut FigApp, which: usize, c: Rgba, link: Option<String>) {
    match which {
        0 | 1 => {
            if let Some(slot) = app.pixel.colors.get_mut(which) {
                *slot = c;
            }
        }
        _ => {
            if let Some(s) = app.pixel.swap.as_mut() {
                s.to = c;
            }
        }
    }
    if let Some(l) = app.pixel.links.get_mut(which) {
        *l = link;
    }
}

/// A colour the picker lists: the colour, its name, and its link.
struct Choice {
    color: Rgba,
    name: String,
    link: String,
}

/// The file's colour styles, by folder (the part of the name before the last `/`), in file order.
fn style_choices(doc: &Document) -> Vec<(String, Vec<Choice>)> {
    let mut folders: Vec<(String, Vec<Choice>)> = Vec::new();
    for s in doc.styles.iter().take(MAX_LISTED) {
        let StyleKind::Paint { paints } = &s.kind else { continue };
        let Some(c) = paints.iter().find_map(|p| p.solid_color()) else { continue };
        let folder = s.name.rsplit_once('/').map(|(f, _)| f.to_string()).unwrap_or_default();
        let choice = Choice { color: c.to_rgba8(), name: s.name.clone(), link: format!("style:{}", s.id) };
        match folders.iter_mut().find(|(f, _)| *f == folder) {
            Some((_, list)) => list.push(choice),
            None => folders.push((folder, vec![choice])),
        }
    }
    folders
}

/// The file's colour variables, each in its collection's default mode.
fn variable_choices(doc: &Document) -> Vec<Choice> {
    let mut out = Vec::new();
    for v in doc.variables.variables.iter().filter(|v| v.kind == VariableType::Color) {
        if let Some(VariableValue::Color(c)) = doc.variables.resolve(&v.id, &BTreeMap::new()) {
            out.push(Choice { color: c.to_rgba8(), name: v.name.clone(), link: format!("var:{}", v.id) });
        }
        if out.len() >= MAX_LISTED {
            break;
        }
    }
    out
}

/// The picker's extra rows for pixel colour `which`: the file's colour styles (by folder), its
/// colour variables, the sprite's own colours, and for a swap its Replace / As variant / Cancel.
pub(crate) fn picker_rows(app: &mut FigApp, ui: &mut Ui, which: usize, t: &Tokens) {
    let (styles, vars) = match app.session.doc() {
        Ok(doc) => (style_choices(doc), variable_choices(doc)),
        Err(_) => return,
    };
    let own: Vec<Choice> = sprite_colors(app).into_iter().map(|c| Choice { color: c, name: canvas::to_hex(c), link: String::new() }).collect();
    let current = app.pixel.links.get(which).cloned().flatten();
    let mut picked: Option<(Rgba, Option<String>)> = None;
    let mut row = |ui: &mut Ui, label: &str, list: &[Choice]| {
        if list.is_empty() {
            return;
        }
        ui.horizontal(|ui| {
            ui.add_space(16.0);
            widgets::label(ui, label, t.text_secondary);
        });
        egui::Frame::NONE.inner_margin(egui::Margin::symmetric(16, 0)).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
                for ch in list {
                    let (r, resp) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::click());
                    ui.painter().rect_filled(r, CornerRadius::same(3), c32(ch.color));
                    let on = !ch.link.is_empty() && current.as_deref() == Some(ch.link.as_str());
                    ui.painter().rect_stroke(
                        r.expand(if on { 1.5 } else { 0.0 }),
                        CornerRadius::same(3),
                        Stroke::new(if on { 2.0 } else { 1.0 }, if on { t.accent } else { t.border }),
                        egui::StrokeKind::Inside,
                    );
                    if resp.on_hover_text(&ch.name).clicked() {
                        picked = Some((ch.color, (!ch.link.is_empty()).then(|| ch.link.clone())));
                    }
                }
            });
        });
    };
    for (folder, list) in &styles {
        let label = if folder.is_empty() { "Colour styles".to_string() } else { format!("Colour styles · {folder}") };
        row(ui, &label, list);
    }
    row(ui, "Colour variables", &vars);
    row(ui, "In this sprite", &own);
    if let Some((c, link)) = picked {
        set(app, which, c, link);
    }
    if which == SWAP {
        swap_buttons(app, ui, t);
    }
}

/// Replace, As variant (one component selected) and Cancel under a swap's picker.
fn swap_buttons(app: &mut FigApp, ui: &mut Ui, t: &Tokens) {
    let Some(swap) = app.pixel.swap.clone() else { return };
    let (mut replace, mut variant, mut cancel) = (false, false, false);
    egui::Frame::NONE.inner_margin(egui::Margin::symmetric(16, 4)).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = vec2(6.0, 4.0);
            let button = |ui: &mut Ui, label: &str, w: f32, main: bool| {
                let (r, resp) = ui.allocate_exact_size(vec2(w, 24.0), Sense::click());
                let fill = match (main, resp.hovered()) {
                    (true, false) => t.accent,
                    (true, true) => t.accent.gamma_multiply(0.85),
                    (false, true) => t.hover,
                    (false, false) => t.field,
                };
                ui.painter().rect_filled(r, CornerRadius::same(5), fill);
                let ink = if main { Color32::WHITE } else { t.text };
                ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, label, crate::theme::semibold(11.5), ink);
                resp.clicked()
            };
            replace = button(ui, "Replace", 72.0, true);
            if swap.component.is_some() {
                variant = button(ui, "As variant", 78.0, false);
            }
            cancel = button(ui, "Cancel", 60.0, false);
        });
    });
    let link = app.pixel.links.get(SWAP).cloned().flatten();
    if replace && swap.from != swap.to {
        let ids: Vec<u64> = swap.layers.iter().map(|id| id.0).collect();
        let (from, to) = (canvas::to_hex(swap.from), canvas::to_hex(swap.to));
        app.run("pixel.replaceColor", json!({"ids": ids, "from": from, "to": to, "link": link}));
    }
    if let (true, Some(id)) = (variant, swap.component)
        && swap.from != swap.to
    {
        let (from, to) = (canvas::to_hex(swap.from), canvas::to_hex(swap.to));
        app.run("pixel.paletteVariant", json!({"id": id.0, "value": to.clone(), "swap": [[from, to]]}));
    }
    if replace || variant || cancel {
        app.pixel.swap = None;
        crate::colorpicker::close(app);
    }
}

/// The distinct colours of the frame being edited (at most 64, most used first).
pub(crate) fn sprite_colors(app: &mut FigApp) -> Vec<Rgba> {
    let super::Mode::Edit(id) = app.pixel.mode else { return Vec::new() };
    let Some(d) = super::decode(app, id) else { return Vec::new() };
    if let Some((k, v)) = &app.pixel.used
        && *k == d.key
    {
        return v.clone();
    }
    let mut count: std::collections::HashMap<Rgba, usize> = std::collections::HashMap::new();
    for p in d.picture(&d.cel).pixels().iter().filter(|p| p[3] > 0) {
        *count.entry(*p).or_default() += 1;
    }
    let mut v: Vec<(Rgba, usize)> = count.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let v: Vec<Rgba> = v.into_iter().take(64).map(|(c, _)| c).collect();
    app.pixel.used = Some((d.key, v.clone()));
    v
}

/// The colours Shade steps along: the colour style folder (or variable group) the first colour
/// came from, dark to light. None: the engine's own ramp (the sprite's colours).
pub(crate) fn shade_ramp(app: &FigApp) -> Option<Vec<String>> {
    let link = app.pixel.links.first().cloned().flatten()?;
    let doc = app.session.doc().ok()?;
    let name = link_name(doc, &link)?;
    let (folder, _) = name.rsplit_once('/')?;
    let prefix = format!("{folder}/");
    let mut ramp: Vec<Rgba> = if link.starts_with("style:") {
        style_choices(doc).into_iter().filter(|(f, _)| f == folder).flat_map(|(_, l)| l).map(|c| c.color).collect()
    } else {
        variable_choices(doc)
            .into_iter()
            .filter(|c| c.name.strip_prefix(prefix.as_str()).is_some_and(|rest| !rest.contains('/')))
            .map(|c| c.color)
            .collect()
    };
    if ramp.len() < 2 {
        return None;
    }
    ramp.sort_by_key(|c| 299 * u32::from(c[0]) + 587 * u32::from(c[1]) + 114 * u32::from(c[2]));
    ramp.dedup();
    Some(ramp.into_iter().map(canvas::to_hex).collect())
}

/// The tools' colours follow the colour style or variable they came from when it changes (and
/// forget it when it is gone).
pub(crate) fn follow_links(app: &mut FigApp) {
    let Ok(doc) = app.session.doc() else { return };
    let now: Vec<Option<Option<Rgba>>> = app.pixel.links.iter().take(2).map(|l| l.as_deref().map(|l| link_color(doc, l))).collect();
    for (i, c) in now.into_iter().enumerate() {
        match c {
            Some(Some(c)) => {
                if let Some(slot) = app.pixel.colors.get_mut(i) {
                    *slot = c;
                }
            }
            Some(None) => {
                if let Some(l) = app.pixel.links.get_mut(i) {
                    *l = None;
                }
            }
            None => {}
        }
    }
}
