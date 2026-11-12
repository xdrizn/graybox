//! Pixel mode's timeline (G16.4): a strip over the pill with the sprite's frames across and its
//! layers down. A click on a cell draws on that layer in that frame; a frame's header shows its
//! number and how long it lasts, and drags to move the frame; Shift+click picks a run of frames
//! (for a new tag or one duration for all). Above the frames, the tags; a click on one opens its
//! settings. The bar on top plays the frames (the tag the current frame is in, else all of them),
//! steps through them, adds, duplicates and deletes frames, and turns the onion skin on.

use egui::{Align, Align2, Color32, CornerRadius, Id, Layout, Rect, Sense, Stroke, StrokeKind, Ui, UiBuilder, pos2, vec2};
use figcraft_pixel::sprite::Direction;
use figcraft_pixel::{Sprite, anim, store};
use serde_json::json;

use super::Mode;
use crate::FigApp;
use crate::icons::Icon;
use crate::theme::Tokens;
use crate::widgets::{self, FieldEdit, Lead};

/// A frame column's width.
const CELL: f32 = 28.0;
/// The layer names' column.
const NAMES_W: f32 = 104.0;
/// A layer row's height.
const ROW: f32 = 20.0;
/// The button bar's height (and the open tag's settings row).
const BAR: f32 = 30.0;
/// The tags band's height.
const TAGS_H: f32 = 16.0;
/// The frame headers' height (number and duration).
const HEAD_H: f32 = 28.0;
/// The most layer rows shown (the top ones; the Sprite layers panel lists them all).
const MAX_ROWS: usize = 6;
const PAD: f32 = 8.0;
/// The narrowest strip (room for its buttons).
const MIN_W: f32 = 660.0;

/// Where the frames being played started (the app's clock) and the frames they run through.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Playing {
    /// Set on the first frame drawn after Play.
    pub(crate) since: Option<f64>,
    pub(crate) seq: Vec<usize>,
}

/// The run of frames picked in the timeline: the current frame to `frame_to` (both included).
pub(crate) fn picked(app: &FigApp, frames: usize) -> (usize, usize) {
    let last = frames.saturating_sub(1);
    let a = app.pixel.frame.min(last);
    let b = app.pixel.frame_to.unwrap_or(a).min(last);
    (a.min(b), a.max(b))
}

/// The frames the play button runs through: the tag the current frame is in (the one the layer
/// plays first), else every frame.
pub(crate) fn play_sequence(sprite: &Sprite, frame: usize) -> Vec<usize> {
    let played = anim::played(sprite).filter(|t| (t.from..=t.to).contains(&frame));
    let tag = played.or_else(|| sprite.tags.iter().find(|t| (t.from..=t.to).contains(&frame)));
    anim::sequence(sprite, tag)
}

/// Starts or stops playing the frames in Pixel mode (Enter, or the play button).
pub(crate) fn toggle_play(app: &mut FigApp) {
    if app.pixel.playing.take().is_some() {
        return;
    }
    let Mode::Edit(id) = app.pixel.mode else { return };
    let Some(sprite) = sprite_of(app, id) else { return };
    let seq = play_sequence(&sprite, app.pixel.frame);
    if seq.len() > 1 {
        app.pixel.frame_to = None;
        app.pixel.playing = Some(Playing { since: None, seq });
    }
}

/// One frame back or on (wrapping round).
pub(crate) fn step(app: &mut FigApp, on: bool) {
    let Mode::Edit(id) = app.pixel.mode else { return };
    let Some(frames) = sprite_of(app, id).map(|s| s.frames.len()) else { return };
    app.pixel.playing = None;
    app.pixel.frame_to = None;
    let f = app.pixel.frame.min(frames.saturating_sub(1));
    app.pixel.frame = if on { (f + 1) % frames.max(1) } else { f.checked_sub(1).unwrap_or(frames.saturating_sub(1)) };
}

fn sprite_of(app: &FigApp, id: figcraft_doc::NodeId) -> Option<Sprite> {
    app.session.doc().ok().and_then(|d| d.node(id)).and_then(|n| store::load(&n.plugin_data))
}

/// Moves the frame shown on while playing, and asks for a repaint when the next one is due.
fn tick(app: &mut FigApp, ctx: &egui::Context, sprite: &Sprite) {
    let Some(mut p) = app.pixel.playing.clone() else { return };
    if p.seq.iter().any(|i| *i >= sprite.frames.len()) {
        app.pixel.playing = None;
        return;
    }
    let now = ctx.input(|i| i.time);
    let since = *p.since.get_or_insert(now);
    app.pixel.playing = Some(p.clone());
    if let Some((frame, next)) = anim::frame_in(sprite, &p.seq, 0, now - since) {
        app.pixel.frame = frame;
        if let Some(next) = next {
            crate::repaint_in(ctx, next);
        }
    }
}

/// Runs `command` on the sprite being edited (`params` gets its id).
fn on_sprite(app: &mut FigApp, id: figcraft_doc::NodeId, command: &str, mut params: serde_json::Value) -> Option<serde_json::Value> {
    if let Some(o) = params.as_object_mut() {
        o.insert("id".into(), json!(id.0));
    }
    app.run(command, params)
}

/// Where the frame columns are: the left edge of the first one shown, which frame that is, how
/// many show, and the sprite they belong to.
struct Grid {
    x0: f32,
    first: usize,
    shown: usize,
    id: figcraft_doc::NodeId,
}

impl Grid {
    /// The left edge of frame `i`'s column.
    fn col(&self, i: usize) -> f32 {
        self.x0 + (i as f32 - self.first as f32) * CELL
    }

    fn columns(&self) -> std::ops::Range<usize> {
        self.first..self.first + self.shown
    }
}

/// The first frame column shown, so the current frame stays in view.
fn scroll(app: &mut FigApp, frames: usize, shown: usize) -> usize {
    let mut first = app.pixel.first_frame.min(frames.saturating_sub(shown));
    if app.pixel.frame < first {
        first = app.pixel.frame;
    } else if app.pixel.frame >= first + shown {
        first = app.pixel.frame + 1 - shown;
    }
    app.pixel.first_frame = first;
    first
}

/// The timeline over the pill (`pill` is the pill's rectangle).
pub(crate) fn timeline(app: &mut FigApp, ui: &mut Ui, pill: Rect) {
    let Mode::Edit(id) = app.pixel.mode else { return };
    let Some(sprite) = sprite_of(app, id) else { return };
    let ctx = ui.ctx().clone();
    tick(app, &ctx, &sprite);
    let t = Tokens::get(&ctx);
    let frames = sprite.frames.len();
    app.pixel.frame = app.pixel.frame.min(frames.saturating_sub(1));
    let rows = sprite.layers.len().min(MAX_ROWS);
    let canvas = app.canvas.rect;
    let max_w = (canvas.width() - 16.0).max(CELL + NAMES_W);
    let fit = (((max_w - NAMES_W - PAD * 2.0) / CELL).floor() as usize).max(1);
    let shown = frames.min(fit);
    let first = scroll(app, frames, shown);
    let open = app.pixel.tag_open.as_ref().and_then(|n| sprite.tag(n)).cloned();
    if open.is_none() {
        app.pixel.tag_open = None;
    }
    let w = (PAD * 2.0 + NAMES_W + shown as f32 * CELL).max(MIN_W).min(max_w);
    let h = PAD + BAR + if open.is_some() { BAR } else { 0.0 } + TAGS_H + HEAD_H + rows as f32 * ROW + PAD;
    let rect = Rect::from_min_size(pos2(canvas.center().x - w / 2.0, pill.min.y - 8.0 - h), vec2(w, h));
    if rect.min.y < canvas.min.y {
        return;
    }
    // The strip takes presses and drags over it (the canvas under it doesn't draw).
    ui.interact(rect, Id::unique("graybox_pixel_timeline"), Sense::CLICK | Sense::DRAG);
    let painter = ui.painter().clone();
    painter.rect_filled(rect.expand(2.0).translate(vec2(0.0, 1.0)), CornerRadius::same(12), Color32::from_black_alpha(18));
    painter.rect_filled(rect, CornerRadius::same(10), t.panel);
    painter.rect_stroke(rect, CornerRadius::same(10), Stroke::new(1.0, t.border), StrokeKind::Inside);
    let mut y = rect.min.y + PAD;
    buttons(app, ui, Rect::from_min_size(pos2(rect.min.x + PAD, y), vec2(w - PAD * 2.0, BAR)), id, &sprite, &t);
    y += BAR;
    if let Some(tag) = &open {
        tag_row(app, ui, Rect::from_min_size(pos2(rect.min.x + PAD, y), vec2(w - PAD * 2.0, BAR)), id, &sprite, tag);
        y += BAR;
    }
    let grid = Grid { x0: rect.min.x + PAD + NAMES_W, first, shown, id };
    painter.text(pos2(rect.min.x + PAD, y + TAGS_H / 2.0), Align2::LEFT_CENTER, "Tags", crate::theme::regular(10.0), t.text_tertiary);
    tags(app, ui, &grid, y, &sprite);
    y += TAGS_H;
    headers(app, ui, &grid, (y, rect.min.y + PAD + BAR, rect.max.y - PAD), &sprite, &t);
    y += HEAD_H;
    cels(app, ui, &grid, pos2(rect.min.x + PAD, y), &sprite, &t);
}

/// The tags: a band over their frames; a click opens the tag's settings and picks its frames.
fn tags(app: &mut FigApp, ui: &mut Ui, grid: &Grid, y: f32, sprite: &Sprite) {
    let painter = ui.painter().clone();
    for (k, tag) in sprite.tags.iter().enumerate() {
        let (a, b) = (tag.from.max(grid.first), tag.to.min(grid.first + grid.shown - 1));
        if a > b {
            continue;
        }
        let r = Rect::from_min_max(pos2(grid.col(a) + 1.0, y + 1.0), pos2(grid.col(b) + CELL - 1.0, y + TAGS_H - 1.0));
        let hue = tag_color(k);
        let plays = if sprite.play.as_deref() == Some(tag.name.as_str()) { ", plays in Prototype" } else { "" };
        let times = if tag.repeat == 0 { "loops (Repeat 0)".to_string() } else { format!("plays {} times", tag.repeat) };
        let tip = format!("{}: frames {}–{}, {}, {times}{plays}", tag.name, tag.from + 1, tag.to + 1, direction_label(tag.direction));
        let resp = ui.interact(r, Id::unique(("graybox_pixel_tag", grid.id.0, k)), Sense::click()).on_hover_text(tip);
        let selected = app.pixel.tag_open.as_deref() == Some(tag.name.as_str());
        painter.rect_filled(r, CornerRadius::same(3), if selected || resp.hovered() { hue } else { hue.gamma_multiply(0.75) });
        let label = widgets::elide_font(ui, &tag.name, r.width() - 6.0, crate::theme::semibold(10.0));
        painter.text(pos2(r.min.x + 4.0, r.center().y), Align2::LEFT_CENTER, label, crate::theme::semibold(10.0), Color32::WHITE);
        if resp.clicked() {
            app.pixel.playing = None;
            app.pixel.tag_open = if selected { None } else { Some(tag.name.clone()) };
            app.pixel.frame = tag.from;
            app.pixel.frame_to = (tag.to != tag.from).then_some(tag.to);
        }
    }
}

/// The frame headers: number and duration. A click picks the frame (Shift: a run of frames); a
/// drag moves it. `ys` is the headers' top and the strip's top and bottom (for the drop line).
fn headers(app: &mut FigApp, ui: &mut Ui, grid: &Grid, ys: (f32, f32, f32), sprite: &Sprite, t: &Tokens) {
    let (y, top, bottom) = ys;
    let painter = ui.painter().clone();
    let ctx = ui.ctx().clone();
    let frames = sprite.frames.len();
    let (pa, pb) = picked(app, frames);
    let shift = ctx.input(|i| i.modifiers.shift);
    let mut drop_at = None;
    for i in grid.columns() {
        let Some(f) = sprite.frames.get(i) else { continue };
        let r = Rect::from_min_size(pos2(grid.col(i), y), vec2(CELL, HEAD_H)).shrink(1.0);
        let resp = ui.interact(r, Id::unique(("graybox_pixel_frame", grid.id.0, i)), Sense::click_and_drag());
        let on = (pa..=pb).contains(&i);
        let fill = match (on, i == app.pixel.frame) {
            (true, true) => t.accent,
            (true, false) => t.accent.gamma_multiply(0.45),
            _ if resp.hovered() => t.hover,
            _ => Color32::TRANSPARENT,
        };
        painter.rect_filled(r, CornerRadius::same(4), fill);
        let (ink, dim) = if on { (Color32::WHITE, Color32::WHITE.gamma_multiply(0.8)) } else { (t.text, t.text_tertiary) };
        painter.text(pos2(r.center().x, r.min.y + 8.0), Align2::CENTER_CENTER, format!("{}", i + 1), crate::theme::semibold(11.0), ink);
        painter.text(pos2(r.center().x, r.max.y - 7.0), Align2::CENTER_CENTER, format!("{}", f.duration), crate::theme::regular(9.0), dim);
        let resp = resp.on_hover_text(format!("Frame {}, {} ms. Drag to move it; Shift+click picks a run of frames", i + 1, f.duration));
        if resp.clicked() {
            app.pixel.playing = None;
            if shift {
                app.pixel.frame_to = Some(i);
            } else {
                app.pixel.frame = i;
                app.pixel.frame_to = None;
            }
        }
        if resp.drag_started() {
            app.pixel.frame_drag = Some(i);
        }
        if app.pixel.frame_drag == Some(i)
            && let Some(p) = ctx.pointer_interact_pos()
        {
            let to = (((p.x - grid.x0) / CELL).floor() + grid.first as f32).clamp(0.0, frames.saturating_sub(1) as f32) as usize;
            painter.vline(grid.col(to) + if to > i { CELL } else { 0.0 }, top..=bottom, Stroke::new(2.0, t.accent));
            if resp.drag_stopped() {
                drop_at = Some((i, to));
            }
        }
    }
    if let Some((from, to)) = drop_at {
        app.pixel.frame_drag = None;
        if from != to && on_sprite(app, grid.id, "pixel.frame.move", json!({"frame": from, "index": to})).is_some() {
            app.pixel.frame = to;
            app.pixel.frame_to = None;
        }
    }
    if !ctx.input(|i| i.pointer.any_down()) {
        app.pixel.frame_drag = None;
    }
}

/// The layer rows (top layer first, at most [`MAX_ROWS`]) from `at`: a dot where the cel has
/// pixels, a ring where it is empty. A click draws on that layer in that frame.
fn cels(app: &mut FigApp, ui: &mut Ui, grid: &Grid, at: egui::Pos2, sprite: &Sprite, t: &Tokens) {
    let painter = ui.painter().clone();
    let full: Vec<Vec<bool>> = {
        let Some(n) = app.session.doc().ok().and_then(|d| d.node(grid.id)) else { return };
        let layer = |l: &figcraft_pixel::Layer| sprite.frames.iter().map(|f| n.plugin_data.contains_key(&store::cel_key(l.id, f.id))).collect();
        sprite.layers.iter().rev().take(MAX_ROWS).map(layer).collect()
    };
    let (pa, pb) = picked(app, sprite.frames.len());
    let current = super::current_layer(&app.pixel, sprite);
    for (r_i, l) in sprite.layers.iter().rev().take(MAX_ROWS).enumerate() {
        let row = Rect::from_min_size(pos2(at.x, at.y + r_i as f32 * ROW), vec2(NAMES_W + grid.shown as f32 * CELL, ROW));
        let here = current == Some(l.id);
        if here {
            painter.rect_filled(row, CornerRadius::same(3), t.hover);
        }
        let name = widgets::elide_font(ui, &l.name, NAMES_W - 10.0, crate::theme::regular(11.0));
        let ink = if l.visible { t.text } else { t.text_tertiary };
        painter.text(pos2(row.min.x + 4.0, row.center().y), Align2::LEFT_CENTER, name, crate::theme::regular(11.0), ink);
        for i in grid.columns() {
            let c = Rect::from_min_size(pos2(grid.col(i), row.min.y), vec2(CELL, ROW));
            let resp = ui.interact(c, Id::unique(("graybox_pixel_cel", grid.id.0, l.id, i)), Sense::click());
            if (pa..=pb).contains(&i) {
                painter.rect_filled(c.shrink2(vec2(1.0, 0.0)), CornerRadius::ZERO, t.accent.gamma_multiply(if here { 0.3 } else { 0.15 }));
            }
            if resp.hovered() {
                painter.rect_stroke(c.shrink(1.0), CornerRadius::same(3), Stroke::new(1.0, t.border_strong), StrokeKind::Inside);
            }
            if full.get(r_i).and_then(|v| v.get(i)).copied().unwrap_or(false) {
                painter.circle_filled(c.center(), 3.5, ink);
            } else {
                painter.circle_stroke(c.center(), 3.0, Stroke::new(1.0, t.text_tertiary));
            }
            if resp.clicked() {
                app.pixel.playing = None;
                app.pixel.layer = Some(l.id);
                app.pixel.frame = i;
                app.pixel.frame_to = None;
            }
        }
    }
}

/// The bar: play, step, new, duplicate, delete, duration, onion skin, new tag.
fn buttons(app: &mut FigApp, ui: &mut Ui, bar: Rect, id: figcraft_doc::NodeId, sprite: &Sprite, t: &Tokens) {
    let mut ui = ui.new_child(UiBuilder::new().max_rect(bar).layout(Layout::left_to_right(Align::Center)));
    ui.spacing_mut().item_spacing.x = 4.0;
    let frames = sprite.frames.len();
    let (pa, pb) = picked(app, frames);
    if widgets::icon_button(&mut ui, Icon::ChevronLeft, "Previous frame  ,") {
        step(app, false);
    }
    let playing = app.pixel.playing.is_some();
    let (r, resp) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::click());
    let resp = resp.on_hover_text(if playing { "Stop" } else { "Play the frames (the current tag's, else all)" });
    if resp.hovered() {
        ui.painter().rect_filled(r, CornerRadius::same(5), t.hover);
    }
    if playing {
        ui.painter().rect_filled(Rect::from_center_size(r.center(), vec2(9.0, 9.0)), CornerRadius::same(1), t.icon);
    } else {
        crate::icons::paint(ui.painter(), r.shrink(5.0), Icon::Play, t.icon);
    }
    if resp.clicked() {
        toggle_play(app);
    }
    if widgets::icon_button(&mut ui, Icon::ChevronRight, "Next frame  .") {
        step(app, true);
    }
    divider(&mut ui, t);
    if widgets::icon_button(&mut ui, Icon::Plus, "New empty frame after this one") {
        add_frame(app, id, None);
    }
    if text_button(&mut ui, "Duplicate", "Duplicate this frame (its pixels and duration)", t, true) {
        add_frame(app, id, Some(app.pixel.frame));
    }
    if text_button(&mut ui, "Delete", "Delete the picked frames", t, frames > 1 && pb - pa + 1 < frames) {
        app.pixel.playing = None;
        let list: Vec<usize> = (pa..=pb).collect();
        on_sprite(app, id, "pixel.frame.delete", json!({"frames": list}));
        app.pixel.frame = pa.min(frames.saturating_sub(pb - pa + 2));
        app.pixel.frame_to = None;
    }
    divider(&mut ui, t);
    let dur = sprite.frames.get(pa).map(|f| f64::from(f.duration));
    let same = (pa..=pb).all(|i| sprite.frames.get(i).map(|f| f64::from(f.duration)) == dur);
    let tip = if pa == pb { "How long the frame shows (ms)".to_string() } else { format!("How long frames {}–{} show (ms)", pa + 1, pb + 1) };
    let edit = widgets::number_field(
        &mut ui,
        Id::unique(("graybox_pixel_duration", id.0, pa, pb)),
        Lead::Icon(Icon::Clock, &tip),
        if same { dur } else { None },
        "ms",
        84.0,
        10.0,
    );
    if let Some(FieldEdit::Set(v) | FieldEdit::Scrub(v, true)) = edit {
        let list: Vec<usize> = (pa..=pb).collect();
        on_sprite(app, id, "pixel.frame.set", json!({"frames": list, "duration": v}));
    }
    divider(&mut ui, t);
    let onion = app.pixel.onion;
    if toggle_button(&mut ui, "Onion skin", "Show the frames before (red) and after (blue) faintly", t, onion) {
        app.pixel.onion = !onion;
    }
    let room = sprite.tags.len() < figcraft_pixel::sprite::MAX_TAGS;
    if text_button(&mut ui, "New tag", "Name the picked frames as a tag (such as idle or hover)", t, room)
        && let Some(v) = on_sprite(app, id, "pixel.tag.add", json!({"from": pa, "to": pb}))
    {
        app.pixel.tag_open = v.get("name").and_then(|n| n.as_str()).map(str::to_string);
    }
}

/// A new frame after the current one (empty, or a copy of frame `copy`), then shown.
fn add_frame(app: &mut FigApp, id: figcraft_doc::NodeId, copy: Option<usize>) {
    app.pixel.playing = None;
    let p = match copy {
        Some(c) => json!({"index": app.pixel.frame + 1, "copy": c}),
        None => json!({"index": app.pixel.frame + 1}),
    };
    if let Some(i) = on_sprite(app, id, "pixel.frame.add", p).and_then(|v| v.get("frame").and_then(serde_json::Value::as_u64)) {
        app.pixel.frame = usize::try_from(i).unwrap_or(0);
        app.pixel.frame_to = None;
    }
}

/// The open tag's settings: name, which way it plays, how many times, whether the layer plays it
/// in Prototype and Play, and Delete.
fn tag_row(app: &mut FigApp, ui: &mut Ui, row: Rect, id: figcraft_doc::NodeId, sprite: &Sprite, tag: &figcraft_pixel::Tag) {
    let t = Tokens::get(ui.ctx());
    let mut ui = ui.new_child(UiBuilder::new().max_rect(row).layout(Layout::left_to_right(Align::Center)));
    ui.spacing_mut().item_spacing.x = 6.0;
    if let Some(name) = widgets::text_field_named(&mut ui, Id::unique(("graybox_pixel_tag_name", id.0, &tag.name)), &tag.name, 90.0, "Tag name")
        && name.trim() != tag.name
        && !name.trim().is_empty()
        && let Some(v) = on_sprite(app, id, "pixel.tag.set", json!({"tag": tag.name, "name": name.trim()}))
    {
        app.pixel.tag_open = v.get("name").and_then(|n| n.as_str()).map(str::to_string);
        return;
    }
    let dirs = [Direction::Forward, Direction::Reverse, Direction::PingPong];
    let active = dirs.iter().position(|d| *d == tag.direction).unwrap_or(0);
    let picked = ui.allocate_ui(vec2(222.0, 24.0), |ui| widgets::segmented_text(ui, &["Forward", "Reverse", "Ping-pong"], active)).inner;
    if let Some(k) = picked
        && let Some(d) = dirs.get(k)
    {
        on_sprite(app, id, "pixel.tag.set", json!({"tag": tag.name, "direction": direction_name(*d)}));
    }
    let edit = widgets::number_field(
        &mut ui,
        Id::unique(("graybox_pixel_tag_repeat", id.0, &tag.name)),
        Lead::Text("Repeat"),
        Some(f64::from(tag.repeat)),
        "×",
        92.0,
        1.0,
    );
    if let Some(FieldEdit::Set(v) | FieldEdit::Scrub(v, true)) = edit {
        on_sprite(app, id, "pixel.tag.set", json!({"tag": tag.name, "repeat": v.max(0.0).round()}));
    }
    let plays = sprite.play.as_deref() == Some(tag.name.as_str());
    if ui.allocate_ui(vec2(130.0, 24.0), |ui| widgets::checkbox(ui, plays, "Plays in Prototype")).inner {
        on_sprite(app, id, "pixel.set", json!({"play": if plays { "" } else { tag.name.as_str() }}));
    }
    if text_button(&mut ui, "Delete tag", "Delete this tag (its frames stay)", &t, true) {
        on_sprite(app, id, "pixel.tag.delete", json!({"tag": tag.name}));
        app.pixel.tag_open = None;
    }
}

fn direction_name(d: Direction) -> &'static str {
    match d {
        Direction::Forward => "forward",
        Direction::Reverse => "reverse",
        Direction::PingPong => "pingPong",
    }
}

fn direction_label(d: Direction) -> &'static str {
    match d {
        Direction::Forward => "forward",
        Direction::Reverse => "reverse",
        Direction::PingPong => "ping-pong",
    }
}

/// A tag's band colour (original, distinct hues in turn).
fn tag_color(k: usize) -> Color32 {
    const HUES: [[u8; 3]; 6] =
        [[0x3a, 0x63, 0xc4], [0xc4, 0x83, 0x3a], [0x3a, 0x9c, 0x6a], [0xa3, 0x4a, 0xb8], [0xc8, 0x34, 0x47], [0x2f, 0x8f, 0xa8]];
    let [r, g, b] = HUES.get(k % HUES.len()).copied().unwrap_or([0x3a, 0x63, 0xc4]);
    Color32::from_rgb(r, g, b)
}

/// A small text button; true when clicked.
fn text_button(ui: &mut Ui, label: &str, tip: &str, t: &Tokens, enabled: bool) -> bool {
    let font = crate::theme::regular(11.0);
    let w = ui.painter().layout_no_wrap(label.to_string(), font.clone(), t.text).size().x + 14.0;
    let (r, resp) = ui.allocate_exact_size(vec2(w, 22.0), if enabled { Sense::click() } else { Sense::hover() });
    let fill = if enabled && resp.hovered() { t.hover } else { t.field };
    ui.painter().rect_filled(r, CornerRadius::same(4), fill);
    ui.painter().text(r.center(), Align2::CENTER_CENTER, label, font, if enabled { t.text } else { t.text_tertiary });
    enabled && resp.on_hover_text(tip).clicked()
}

/// A small text toggle (accent when on); true when clicked.
fn toggle_button(ui: &mut Ui, label: &str, tip: &str, t: &Tokens, on: bool) -> bool {
    let font = crate::theme::regular(11.0);
    let w = ui.painter().layout_no_wrap(label.to_string(), font.clone(), t.text).size().x + 14.0;
    let (r, resp) = ui.allocate_exact_size(vec2(w, 22.0), Sense::click());
    let fill = if on {
        t.accent
    } else if resp.hovered() {
        t.hover
    } else {
        t.field
    };
    ui.painter().rect_filled(r, CornerRadius::same(4), fill);
    ui.painter().text(r.center(), Align2::CENTER_CENTER, label, font, if on { Color32::WHITE } else { t.text });
    resp.on_hover_text(tip).clicked()
}

/// A thin upright line between groups of buttons.
fn divider(ui: &mut Ui, t: &Tokens) {
    let (r, _) = ui.allocate_exact_size(vec2(1.0, 18.0), Sense::hover());
    ui.painter().rect_filled(r, CornerRadius::ZERO, t.border);
}
