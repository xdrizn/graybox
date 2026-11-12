//! Pixel mode's pill (the bottom toolbar while it is on): the tools, the two colours, the options of
//! the tool in hand, the zoom in art pixels and Done. With no sprite yet: the New sprite size.

use egui::{Align2, Color32, CornerRadius, Pos2, Rect, Sense, Stroke, Ui, pos2, vec2};
use figcraft_pixel::paint::BrushShape;
use figcraft_pixel::{Rgba, canvas};
use serde_json::json;

use super::{Mode, Tool, art_to_page, decode, leave, step_brush, toggle_shape, zoom_of};
use crate::FigApp;
use crate::icons::Icon;
use crate::theme::Tokens;
use crate::toolbar::{DIVIDER, DONE_W, GAP, PAD, TOOL_CELL, divider, done_button, pill, pretty_shortcut, tool_cell};

/// The brush size field's width.
const SIZE_W: f32 = 64.0;
/// The zoom label's width.
const ZOOM_W: f32 = 40.0;

/// An option button in the pill.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Opt {
    Shape,
    Size,
    Perfect,
    Filled,
    Contiguous,
    MirrorX,
    MirrorY,
    Tile,
    Grid,
}

impl Opt {
    fn width(self) -> f32 {
        if self == Opt::Size { SIZE_W } else { TOOL_CELL }
    }
}

/// The options shown for `tool`.
fn options(tool: Tool) -> Vec<Opt> {
    let mut v = Vec::new();
    match tool {
        Tool::Pencil => v.extend([Opt::Shape, Opt::Size, Opt::Perfect]),
        Tool::Eraser | Tool::Line => v.extend([Opt::Shape, Opt::Size]),
        Tool::Rect | Tool::Ellipse => v.extend([Opt::Shape, Opt::Size, Opt::Filled]),
        Tool::Bucket => v.push(Opt::Contiguous),
        Tool::Eyedropper => {}
    }
    if !matches!(tool, Tool::Eyedropper | Tool::Bucket) {
        v.extend([Opt::MirrorX, Opt::MirrorY]);
    }
    v.extend([Opt::Tile, Opt::Grid]);
    v
}

/// Pixel mode's pill, when it is on. True when it drew (the app's own pill is then not shown).
pub(crate) fn toolbar(app: &mut FigApp, ui: &mut Ui) -> bool {
    super::settle(app);
    match app.pixel.mode {
        Mode::Off => false,
        Mode::New => {
            new_pill(app, ui);
            true
        }
        Mode::Edit(_) => {
            edit_pill(app, ui);
            true
        }
    }
}

fn edit_pill(app: &mut FigApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let opts = options(app.pixel.tool);
    let tools_w = Tool::ALL.len() as f32 * (TOOL_CELL + GAP);
    let opts_w: f32 = opts.iter().map(|o| o.width() + GAP).sum();
    let width = PAD * 2.0 + tools_w + DIVIDER + GAP + TOOL_CELL + GAP + DIVIDER + GAP + opts_w + ZOOM_W + GAP + DIVIDER + GAP + DONE_W;
    let rect = pill(ui, &mut app.ui.nav, app.canvas.rect, width, &t);
    let cy = rect.center().y;
    let mut x = rect.min.x + PAD;
    for tool in Tool::ALL {
        let r = Rect::from_center_size(pos2(x + TOOL_CELL / 2.0, cy), vec2(TOOL_CELL, TOOL_CELL));
        let tip = format!("{}  {}", tool.label(), pretty_shortcut(tool.key()));
        if tool_cell(ui, &t, r, ("pixel_tool", tool.label()), tool.icon(), app.pixel.tool == tool, &tip).clicked() {
            app.pixel.tool = tool;
        }
        x += TOOL_CELL + GAP;
    }
    divider(ui, &t, x, cy);
    x += DIVIDER + GAP;
    swatches(app, ui, Rect::from_center_size(pos2(x + TOOL_CELL / 2.0, cy), vec2(TOOL_CELL, TOOL_CELL)), &t);
    x += TOOL_CELL + GAP;
    divider(ui, &t, x, cy);
    x += DIVIDER + GAP;
    for o in opts {
        let r = Rect::from_center_size(pos2(x + o.width() / 2.0, cy), vec2(o.width(), TOOL_CELL));
        option(app, ui, o, r, &t);
        x += o.width() + GAP;
    }
    // How big an art pixel is on screen.
    let zoom = pixel_zoom(app);
    let label = if zoom >= 10.0 { format!("×{zoom:.0}") } else { format!("×{zoom:.1}") };
    let zr = Rect::from_min_size(pos2(x, cy - TOOL_CELL / 2.0), vec2(ZOOM_W, TOOL_CELL));
    ui.interact(zr, ui.scope_id().with("pixel_zoom"), Sense::hover()).on_hover_text("Screen pixels per art pixel");
    ui.painter().text(zr.center(), Align2::CENTER_CENTER, label, crate::theme::regular(11.5), t.text_secondary);
    x += ZOOM_W + GAP;
    if done_button(ui, &t, x, cy, &format!("Done  {}", pretty_shortcut("Esc"))) {
        leave(app);
    }
    color_picker(app, ui, rect, &t);
}

/// Screen pixels per art pixel of the sprite being edited.
fn pixel_zoom(app: &mut FigApp) -> f32 {
    let Mode::Edit(id) = app.pixel.mode else { return 1.0 };
    let Some(d) = decode(app, id) else { return 1.0 };
    art_to_page(app, id, &d.sprite).map_or(1.0, |xf| zoom_of(app, xf))
}

fn c32(c: Rgba) -> Color32 {
    Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3])
}

/// A swatch: the colour over a small checkerboard (so transparency shows).
fn swatch(painter: &egui::Painter, r: Rect, c: Rgba, border: Color32) {
    let half = r.width() / 2.0;
    painter.rect_filled(r, CornerRadius::same(3), Color32::from_gray(0xc8));
    painter.rect_filled(Rect::from_min_size(r.min, vec2(half, half)), CornerRadius::ZERO, Color32::from_gray(0xa0));
    painter.rect_filled(Rect::from_min_size(r.min + vec2(half, half), vec2(half, half)), CornerRadius::ZERO, Color32::from_gray(0xa0));
    painter.rect_filled(r, CornerRadius::same(3), c32(c));
    painter.rect_stroke(r, CornerRadius::same(3), Stroke::new(1.0, border), egui::StrokeKind::Inside);
}

/// The primary and secondary colours (the secondary behind, lower right). A click opens a picker;
/// `X` swaps them.
fn swatches(app: &mut FigApp, ui: &mut Ui, r: Rect, t: &Tokens) {
    let back = Rect::from_min_size(r.min + vec2(12.0, 12.0), vec2(16.0, 16.0));
    let front = Rect::from_min_size(r.min + vec2(4.0, 4.0), vec2(16.0, 16.0));
    let resp2 = ui.interact(back, ui.scope_id().with("pixel_color2"), Sense::click()).on_hover_text("Secondary colour (right button)  ·  X swaps");
    let resp1 = ui.interact(front, ui.scope_id().with("pixel_color1"), Sense::click()).on_hover_text("Primary colour  ·  X swaps");
    swatch(ui.painter(), back, app.pixel.colors[1], t.border_strong);
    swatch(ui.painter(), front, app.pixel.colors[0], t.border_strong);
    if resp1.clicked() {
        app.pixel.picking = if app.pixel.picking == Some(0) { None } else { Some(0) };
    } else if resp2.clicked() {
        app.pixel.picking = if app.pixel.picking == Some(1) { None } else { Some(1) };
    }
}

/// One option button.
fn option(app: &mut FigApp, ui: &mut Ui, o: Opt, r: Rect, t: &Tokens) {
    let e = &mut app.pixel;
    match o {
        Opt::Size => {
            let resp = ui.interact(r, ui.scope_id().with("pixel_size"), Sense::click_and_drag()).on_hover_text("Brush size  [ ]");
            ui.painter().rect_filled(r.shrink2(vec2(0.0, 3.0)), CornerRadius::same(5), t.field);
            let minus = Rect::from_min_size(r.min, vec2(18.0, r.height()));
            let plus = Rect::from_min_size(pos2(r.max.x - 18.0, r.min.y), vec2(18.0, r.height()));
            let font = crate::theme::regular(12.0);
            ui.painter().text(minus.center(), Align2::CENTER_CENTER, "−", font.clone(), t.icon_secondary);
            ui.painter().text(plus.center(), Align2::CENTER_CENTER, "+", font.clone(), t.icon_secondary);
            ui.painter().text(r.center(), Align2::CENTER_CENTER, format!("{}", e.brush.size), font, t.text);
            if resp.clicked()
                && let Some(p) = resp.interact_pointer_pos()
            {
                step_brush(e, p.x > r.center().x);
            }
            let scroll = if resp.hovered() { ui.input(|i| i.smooth_scroll_delta.y) } else { 0.0 };
            if scroll != 0.0 {
                step_brush(e, scroll > 0.0);
            }
            if resp.dragged() {
                let dx = resp.drag_delta().x;
                let acc = ui.data_mut(|d| {
                    let a = d.get_temp_mut_or_default::<f32>(egui::Id::unique("pixel_size_drag"));
                    *a += dx;
                    let steps = (*a / 6.0).trunc();
                    *a -= steps * 6.0;
                    steps as i32
                });
                for _ in 0..acc.unsigned_abs() {
                    step_brush(e, acc > 0);
                }
            }
        }
        Opt::Shape => {
            let round = e.brush.shape == BrushShape::Circle;
            let resp =
                ui.interact(r, ui.scope_id().with("pixel_shape"), Sense::click()).on_hover_text(if round { "Round brush" } else { "Square brush" });
            if resp.hovered() {
                ui.painter().rect_filled(r, CornerRadius::same(5), t.hover);
            }
            let c = r.center();
            if round {
                ui.painter().circle_filled(c, 6.0, t.icon);
            } else {
                ui.painter().rect_filled(Rect::from_center_size(c, vec2(11.0, 11.0)), CornerRadius::ZERO, t.icon);
            }
            if resp.clicked() {
                toggle_shape(e);
            }
        }
        _ => {
            let (icon, on, tip) = match o {
                Opt::Perfect => (Icon::PixelPerfect, e.pixel_perfect, "Pixel-perfect lines (no L corners)"),
                Opt::Filled => (Icon::Fill, e.filled, "Filled shape"),
                Opt::Contiguous => (Icon::LinkOn, e.contiguous, "Only what touches (off: every pixel of that colour)"),
                Opt::MirrorX => (Icon::FlipH, e.mirror[0], "Mirror left and right (drag the pink handle to move the axis)"),
                Opt::MirrorY => (Icon::FlipV, e.mirror[1], "Mirror up and down (drag the pink handle to move the axis)"),
                Opt::Tile => (Icon::Tiles, e.tile, "Tile mode: drawing wraps round the edges"),
                _ => (Icon::Grid, e.grid, "Pixel grid"),
            };
            if tool_cell(ui, t, r, ("pixel_opt", tip), icon, on, tip).clicked() {
                match o {
                    Opt::Perfect => e.pixel_perfect = !e.pixel_perfect,
                    Opt::Filled => e.filled = !e.filled,
                    Opt::Contiguous => e.contiguous = !e.contiguous,
                    Opt::MirrorX => e.mirror[0] = !e.mirror[0],
                    Opt::MirrorY => e.mirror[1] = !e.mirror[1],
                    Opt::Tile => e.tile = !e.tile,
                    _ => e.grid = !e.grid,
                }
            }
        }
    }
}

/// The open colour's picker above the pill: a colour wheel, its hex, and the colours the sprite
/// already uses.
fn color_picker(app: &mut FigApp, ui: &mut Ui, pill: Rect, t: &Tokens) {
    let Some(which) = app.pixel.picking else { return };
    let which = which.min(1);
    let used = used_colors(app);
    let mut color = app.pixel.colors[which];
    let anchor = pos2(pill.min.x + PAD + 7.0 * (TOOL_CELL + GAP), pill.min.y - 8.0);
    let area = egui::Area::new(egui::Id::unique("graybox_pixel_picker")).order(egui::Order::Foreground).pivot(Align2::LEFT_BOTTOM).fixed_pos(anchor);
    let shown = area.show(ui.ctx(), |ui| {
        egui::Frame::popup(ui.style()).fill(t.panel).show(ui, |ui| {
            ui.set_width(220.0);
            ui.label(if which == 0 { "Primary colour" } else { "Secondary colour" });
            let mut c = c32(color);
            if egui::color_picker::color_picker_color32(ui, &mut c, egui::color_picker::Alpha::OnlyBlend) {
                color = c.to_srgba_unmultiplied();
            }
            let id = egui::Id::unique(("graybox_pixel_hex", which));
            let mut hex = ui.data(|d| d.get_temp::<String>(id)).unwrap_or_else(|| canvas::to_hex(color));
            let field = ui.add(egui::TextEdit::singleline(&mut hex).desired_width(100.0));
            if field.changed()
                && let Some(c) = canvas::parse_hex(&hex)
            {
                color = c;
            }
            if field.has_focus() {
                ui.data_mut(|d| d.insert_temp(id, hex));
            } else {
                ui.data_mut(|d| d.remove::<String>(id));
            }
            if !used.is_empty() {
                ui.add_space(4.0);
                ui.label("In this sprite");
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(3.0, 3.0);
                    for c in &used {
                        let (r, resp) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::click());
                        swatch(ui.painter(), r, *c, t.border);
                        if resp.on_hover_text(canvas::to_hex(*c)).clicked() {
                            color = *c;
                        }
                    }
                });
            }
        });
    });
    app.pixel.colors[which] = color;
    let outside =
        ui.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !shown.response.rect.contains(p) && !pill.contains(p)));
    if outside {
        app.pixel.closed_picker = true;
    }
    if outside || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.pixel.picking = None;
    }
}

/// The distinct colours of the frame being edited (at most 64, most used first).
fn used_colors(app: &mut FigApp) -> Vec<Rgba> {
    let Mode::Edit(id) = app.pixel.mode else { return Vec::new() };
    let Some(d) = decode(app, id) else { return Vec::new() };
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

/// The pill with no sprite yet: how to make one, and its size.
fn new_pill(app: &mut FigApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let hint = "Drag on the canvas to make a sprite, or";
    let galley = ui.painter().layout_no_wrap(hint.to_string(), crate::theme::regular(12.0), t.text_secondary);
    let fields_w = 3.0 * 52.0 + 2.0 * 14.0 + 18.0;
    let create_w = 64.0;
    let width = PAD * 2.0 + galley.size().x + GAP + create_w + GAP + DIVIDER + GAP + fields_w + GAP + DIVIDER + GAP + DONE_W;
    let rect = pill(ui, &mut app.ui.nav, app.canvas.rect, width, &t);
    let cy = rect.center().y;
    let mut x = rect.min.x + PAD;
    let w = galley.size().x;
    ui.painter().galley(pos2(x, cy - galley.size().y / 2.0), galley, t.text_secondary);
    x += w + GAP;
    let create = Rect::from_min_size(pos2(x, cy - 14.0), vec2(create_w, 28.0));
    let resp =
        ui.interact(create, ui.scope_id().with("pixel_new_create"), Sense::click()).on_hover_text("A sprite of this size in the middle of the view");
    ui.painter().rect_filled(create, CornerRadius::same(6), if resp.hovered() { t.hover } else { t.field });
    ui.painter().text(create.center(), Align2::CENTER_CENTER, "Create", crate::theme::semibold(11.5), t.text);
    if resp.clicked() {
        create_in_view(app);
    }
    x += create_w + GAP;
    divider(ui, &t, x, cy);
    x += DIVIDER + GAP;
    let labels = ["W", "H", "×"];
    let ranges = [(1, 1024), (1, 1024), (1, 64)];
    for i in 0..3 {
        if i > 0 {
            let sep = if i == 2 { "at" } else { "×" };
            ui.painter().text(pos2(x + 7.0, cy), Align2::CENTER_CENTER, sep, crate::theme::regular(11.0), t.text_tertiary);
            x += 14.0;
        }
        let r = Rect::from_min_size(pos2(x, cy - 12.0), vec2(52.0, 24.0));
        let (lo, hi) = ranges[i];
        let v = &mut app.pixel.new_size[i];
        let tip = ["Width (art pixels)", "Height (art pixels)", "Scale (screen pixels per art pixel)"][i];
        ui.put(r, egui::DragValue::new(v).range(lo..=hi).prefix(format!("{} ", labels[i])).speed(0.2)).on_hover_text(tip);
        x += 52.0;
    }
    x += 18.0 + GAP;
    if done_button(ui, &t, x, cy, &format!("Done  {}", pretty_shortcut("Esc"))) {
        leave(app);
    }
}

/// A sprite of the New sprite size in the middle of the view, edited at once.
fn create_in_view(app: &mut FigApp) {
    let [w, h, s] = app.pixel.new_size;
    let c = app.canvas.rect.center();
    let mid = app.to_page(Pos2::new(c.x, c.y));
    let (pw, ph) = (f64::from(w * s), f64::from(h * s));
    let made =
        app.run("pixel.create", json!({"x": (mid.x - pw / 2.0).round(), "y": (mid.y - ph / 2.0).round(), "width": w, "height": h, "scale": s}));
    if let Some(id) = made.as_ref().and_then(|r| figcraft_engine::params::id(r, "id")) {
        super::enter(app, id);
    }
}
