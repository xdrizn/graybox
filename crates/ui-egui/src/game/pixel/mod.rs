//! Graybox Pixel mode (G16): a pixel layer's sprite edited in place on the canvas, with the rest of
//! the design still around it. The editor keeps the tool settings and a decoded copy of the cel
//! being drawn on; a stroke is previewed with the same `figcraft_pixel` code the `pixel.*` command
//! then runs on release (one undo step), so what is shown while drawing is what is stored.

use egui::Pos2;
use figcraft_doc::NodeId;
use figcraft_geom::{Affine, Point};
use figcraft_pixel::paint::{BrushShape, Ink, Symmetry};
use figcraft_pixel::raster::Pt;
use figcraft_pixel::select::Op;
use figcraft_pixel::{Brush, Canvas, Mask, Rgba, Sprite, store};

use crate::FigApp;
use crate::icons::Icon;

pub(crate) mod board;
pub(crate) mod colors;
mod draw;
mod edges;
mod export;
mod font;
mod input;
mod panel;
mod pill;
#[cfg(test)]
mod tests;
pub(crate) mod timeline;

pub(crate) use draw::draw;
pub(crate) use input::{input, key, off_canvas, paste_events};
pub(crate) use panel::section;
pub(crate) use pill::toolbar;

/// A Pixel mode tool.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) enum Tool {
    #[default]
    Pencil,
    Eraser,
    Bucket,
    Eyedropper,
    Replace,
    Line,
    Rect,
    Ellipse,
    Marquee,
    Lasso,
    Wand,
    Move,
}

impl Tool {
    /// The pill's buttons: a tool alone, or a group of them that one button holds (the shapes, the
    /// selection tools); a click on the group's button when it is in hand picks the next one.
    pub(crate) const SLOTS: [&'static [Tool]; 8] = [
        &[Tool::Pencil],
        &[Tool::Eraser],
        &[Tool::Bucket],
        &[Tool::Eyedropper],
        &[Tool::Replace],
        &[Tool::Line, Tool::Rect, Tool::Ellipse],
        &[Tool::Marquee, Tool::Lasso, Tool::Wand],
        &[Tool::Move],
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Tool::Pencil => "Pencil",
            Tool::Eraser => "Eraser",
            Tool::Bucket => "Paint bucket",
            Tool::Eyedropper => "Eyedropper",
            Tool::Replace => "Replace colour (paints over the second colour only)",
            Tool::Line => "Line",
            Tool::Rect => "Rectangle",
            Tool::Ellipse => "Ellipse",
            Tool::Marquee => "Marquee selection",
            Tool::Lasso => "Lasso selection",
            Tool::Wand => "Magic wand",
            Tool::Move => "Move",
        }
    }

    /// The key that picks it (what pixel editors use).
    pub(crate) fn key(self) -> &'static str {
        match self {
            Tool::Pencil => "B",
            Tool::Eraser => "E",
            Tool::Bucket => "G",
            Tool::Eyedropper => "I",
            Tool::Replace => "R",
            Tool::Line => "L",
            Tool::Rect => "U",
            Tool::Ellipse => "Shift+U",
            Tool::Marquee => "M",
            Tool::Lasso => "Q",
            Tool::Wand => "W",
            Tool::Move => "V",
        }
    }

    pub(crate) fn icon(self) -> Icon {
        match self {
            Tool::Pencil => Icon::Pencil,
            Tool::Eraser => Icon::Eraser,
            Tool::Bucket => Icon::PaintBucket,
            Tool::Eyedropper => Icon::Eyedropper,
            Tool::Replace => Icon::ReplaceColor,
            Tool::Line => Icon::Line,
            Tool::Rect => Icon::Rectangle,
            Tool::Ellipse => Icon::Ellipse,
            Tool::Marquee => Icon::Marquee,
            Tool::Lasso => Icon::Lasso,
            Tool::Wand => Icon::MagicWand,
            Tool::Move => Icon::Move,
        }
    }

    /// Whether it makes or changes a selection rather than drawing.
    pub(crate) fn selects(self) -> bool {
        matches!(self, Tool::Marquee | Tool::Lasso | Tool::Wand | Tool::Move)
    }
}

/// Whether Pixel mode is on and what it edits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Mode {
    #[default]
    Off,
    /// On with no sprite yet: a drag on the canvas makes one.
    New,
    Edit(NodeId),
}

/// What the pointer is doing.
#[derive(Clone, Debug)]
pub(crate) enum Gesture {
    /// A freehand stroke (pencil or eraser): the art pixels the pointer went through.
    Stroke { points: Vec<Pt>, secondary: bool, erase: bool },
    /// A line, rectangle or ellipse from `from` to `to`.
    Shape { from: Pt, to: Pt, secondary: bool },
    /// Moving a mirror axis (0: the vertical one, 1: the horizontal one).
    Axis(usize),
    /// Dragging out a new sprite (page points).
    NewSprite { from: Point, to: Point },
    /// Making a selection: the marquee's two corners or the lasso's points, and how it combines.
    Select { tool: Tool, points: Vec<Pt>, op: Op },
    /// Moving the selected pixels (or the whole cel) from `from` to `to`.
    Move { from: Pt, to: Pt },
    /// Dragging a board's side or corner (G16.7): which sides (x, y: -1, 0 or 1), from and to on
    /// screen, and Alt (both sides).
    Board { sides: (i8, i8), from: Pos2, to: Pos2, alt: bool },
}

/// The sprite being edited, decoded: the current layer's cel and what lies below and above it.
#[derive(Clone)]
pub(crate) struct Decoded {
    pub(crate) key: u64,
    pub(crate) sprite: Sprite,
    pub(crate) layer: u32,
    pub(crate) frame: usize,
    pub(crate) cel: Canvas,
    below: Canvas,
    above: Canvas,
    /// The sprite's selection, and its outline (unit edges in art pixels).
    pub(crate) selection: Option<Mask>,
    pub(crate) edges: Vec<(Pt, Pt)>,
    /// The layer's opacity, or 0 when hidden.
    opacity: u8,
}

impl Decoded {
    /// The frame as shown with `cel` in place of the current layer's.
    pub(crate) fn picture(&self, cel: &Canvas) -> Canvas {
        let mut out = self.below.clone();
        out.draw_over(cel, self.opacity);
        out.draw_over(&self.above, 255);
        out
    }
}

/// Pixel mode's state: the tool and its settings, kept while the app runs.
pub struct Editor {
    pub(crate) mode: Mode,
    pub(crate) tool: Tool,
    /// The primary (left button) and secondary (right button) colours.
    pub(crate) colors: [Rgba; 2],
    pub(crate) brush: Brush,
    pub(crate) pixel_perfect: bool,
    /// Shade mode (G16.6): the pencil steps colours along the sprite palette instead of painting.
    pub(crate) shade: bool,
    /// Mirror left/right, up/down.
    pub(crate) mirror: [bool; 2],
    /// The mirror axes, doubled (half pixels); set to the middle when a sprite is entered.
    pub(crate) axis: [i64; 2],
    /// The pointer Pixel mode asked for this frame (over a mirror axis handle or a board side), so
    /// the canvas leaves it instead of showing the tool's (G16.7).
    pub(crate) cursor: Option<egui::CursorIcon>,
    pub(crate) tile: bool,
    pub(crate) filled: bool,
    pub(crate) contiguous: bool,
    pub(crate) grid: bool,
    /// The layer drawn on (id) and the frame (index); None: the top layer.
    pub(crate) layer: Option<u32>,
    pub(crate) frame: usize,
    pub(crate) gesture: Option<Gesture>,
    /// The art pixel under the pointer.
    pub(crate) hover: Option<Pt>,
    /// The last art pixel a stroke ended on (Shift-click draws a line on from it).
    pub(crate) last: Option<Pt>,
    /// Where the pointer was last seen on the canvas (a drag released off it ends there).
    pub(crate) pointer: Option<Pos2>,
    pub(crate) decoded: Option<std::sync::Arc<Decoded>>,
    /// The picture on screen and what it was made from.
    pub(crate) texture: Option<(egui::TextureHandle, u64)>,
    pub(crate) checker: Option<egui::TextureHandle>,
    /// The colour style or variable each colour came from (0 primary, 1 secondary, 2 the Pixel
    /// colors swap), as `style:<id>` or `var:<id>`: painting records it on the sprite (G16.6).
    pub(crate) links: [Option<String>; 3],
    /// The Pixel colors swap open in the colour picker.
    pub(crate) swap: Option<colors::Swap>,
    /// The colours the frame being edited uses, and the decode key they were counted for.
    pub(crate) used: Option<(u64, Vec<figcraft_pixel::Rgba>)>,
    /// What Copy took: the pixels and where their top left was.
    pub(crate) clipboard: Option<(Canvas, i64, i64)>,
    /// A copy waiting to go on the system clipboard (it needs the frame's context).
    pub(crate) to_system: Option<Canvas>,
    /// This frame pasted with the V key (the paste event a browser sends with it is then not a second paste).
    pub(crate) pasted: bool,
    /// This frame cut with the X key (likewise, a cut event with it is not a second cut).
    pub(crate) cut: bool,
    /// The New sprite size: width, height, scale.
    pub(crate) new_size: [u32; 3],
    /// The timeline (G16.4): the other end of a run of frames picked with Shift (the current
    /// frame is the first end), the first frame column shown, the frames playing, the tag whose
    /// settings are open, the frame header being dragged.
    pub(crate) frame_to: Option<usize>,
    pub(crate) first_frame: usize,
    pub(crate) playing: Option<timeline::Playing>,
    pub(crate) tag_open: Option<String>,
    pub(crate) frame_drag: Option<usize>,
    /// Onion skin: the frames before and after shown faintly, and their pictures (decode keys).
    pub(crate) onion: bool,
    pub(crate) onion_tex: Vec<(u64, egui::TextureHandle)>,
    /// The Export and Import settings (G16.5).
    pub(crate) export: export::Prefs,
    /// The Pixel art section of the Design panel is folded away.
    pub(crate) panel_closed: bool,
}

impl Default for Editor {
    fn default() -> Self {
        Self {
            mode: Mode::Off,
            tool: Tool::Pencil,
            colors: [[0x1b, 0x1d, 0x29, 255], [0xf3, 0xf1, 0xea, 255]],
            brush: Brush::default(),
            pixel_perfect: true,
            shade: false,
            mirror: [false, false],
            axis: [0, 0],
            cursor: None,
            tile: false,
            filled: false,
            contiguous: true,
            grid: true,
            layer: None,
            frame: 0,
            gesture: None,
            hover: None,
            last: None,
            pointer: None,
            decoded: None,
            texture: None,
            checker: None,
            links: [None, None, None],
            swap: None,
            used: None,
            clipboard: None,
            to_system: None,
            pasted: false,
            cut: false,
            new_size: [32, 32, 4],
            frame_to: None,
            first_frame: 0,
            playing: None,
            tag_open: None,
            frame_drag: None,
            onion: false,
            onion_tex: Vec::new(),
            export: export::Prefs::default(),
            panel_closed: false,
        }
    }
}

/// Whether Pixel mode is on.
pub(crate) fn active(app: &FigApp) -> bool {
    app.pixel.mode != Mode::Off
}

/// Whether layer `id` is a pixel layer.
pub(crate) fn is_pixel_layer(app: &FigApp, id: NodeId) -> bool {
    app.session.doc().ok().and_then(|d| d.node(id)).is_some_and(|n| store::is_pixel(&n.plugin_data))
}

/// Pixel mode on for layer `id`: selected, in Design, the mirror axes in its middle.
pub(crate) fn enter(app: &mut FigApp, id: NodeId) {
    let Some(sprite) = app
        .session
        .doc()
        .ok()
        .and_then(|d| d.node(id))
        .filter(|n| !matches!(n.kind, figcraft_doc::NodeKind::Instance { .. }))
        .and_then(|n| store::load(&n.plugin_data))
    else {
        return;
    };
    let same = app.pixel.mode == Mode::Edit(id);
    app.pixel.mode = Mode::Edit(id);
    app.pixel.gesture = None;
    if !same {
        app.pixel.axis = [i64::from(sprite.width), i64::from(sprite.height)];
        app.pixel.layer = None;
        app.pixel.frame = 0;
        app.pixel.frame_to = None;
        app.pixel.first_frame = 0;
        app.pixel.playing = None;
        app.pixel.tag_open = None;
        app.pixel.decoded = None;
        app.pixel.last = None;
    }
    app.ui.right_tab = crate::RightTab::Design;
    if app.session.selection() != vec![id] {
        app.run("select.set", serde_json::json!({"ids": [id.0]}));
    }
}

/// Pixel mode on from the mode switcher: the selected pixel layer, or a new sprite to drag out.
pub(crate) fn enter_from_switcher(app: &mut FigApp) {
    match app.session.selection().as_slice() {
        [id] if is_pixel_layer(app, *id) => enter(app, *id),
        _ => {
            app.pixel.mode = Mode::New;
            app.pixel.gesture = None;
            app.ui.right_tab = crate::RightTab::Design;
        }
    }
}

/// Pixel mode off (back to Design).
pub(crate) fn leave(app: &mut FigApp) {
    app.pixel.mode = Mode::Off;
    app.pixel.gesture = None;
    app.pixel.playing = None;
    app.pixel.decoded = None;
    app.pixel.hover = None;
    if matches!(app.ui.picker, Some(crate::colorpicker::PickerTarget::Pixel { which: 0 | 1 })) {
        crate::colorpicker::close(app);
    }
}

/// The `pixel.mode` request a command made.
pub(crate) fn ui_request(app: &mut FigApp, data: &serde_json::Value) {
    let on = data.get("on").and_then(serde_json::Value::as_bool).unwrap_or(true);
    match data.get("id").and_then(figcraft_engine::params::id_of) {
        Some(id) if on => enter(app, id),
        _ if on => enter_from_switcher(app),
        _ => leave(app),
    }
}

/// Keeps the mode consistent with the app: the layer still there and a pixel layer, Design shown.
pub(crate) fn settle(app: &mut FigApp) {
    app.pixel.pasted = false;
    app.pixel.cut = false;
    if app.pixel.mode == Mode::Off {
        return;
    }
    if app.ui.right_tab != crate::RightTab::Design || app.session.ds().is_err() {
        leave(app);
        return;
    }
    if let Mode::Edit(id) = app.pixel.mode {
        if !is_pixel_layer(app, id) {
            leave(app);
            return;
        }
        // Colours taken from a style follow it (G16.6).
        colors::follow_links(app);
        // Another layer selected (Layers panel, select all, undo): another sprite is edited in
        // its place; anything else ends Pixel mode.
        let sel = app.session.selection();
        if sel != [id] {
            match sel.as_slice() {
                [other] if is_pixel_layer(app, *other) => enter(app, *other),
                _ => leave(app),
            }
        }
    }
}

/// The ink for a stroke or shape with the current settings.
pub(crate) fn ink(e: &Editor, secondary: bool, erase: bool) -> Ink {
    Ink {
        color: e.colors[usize::from(secondary)],
        brush: e.brush,
        symmetry: Symmetry { x2: e.mirror[0].then_some(e.axis[0]), y2: e.mirror[1].then_some(e.axis[1]) },
        tile: e.tile,
        erase,
        // Replace colour paints over the other colour only.
        replace: (e.tool == Tool::Replace && !erase).then(|| e.colors[usize::from(!secondary)]),
    }
}

/// The current layer id of `sprite` (the chosen one if it still exists, else the top one).
pub(crate) fn current_layer(e: &Editor, sprite: &Sprite) -> Option<u32> {
    e.layer.filter(|l| sprite.layer_index(*l).is_some()).or_else(|| sprite.layers.last().map(|l| l.id))
}

/// The sprite of layer `id`, decoded for drawing on (shared, and reused while nothing it is made of changed).
pub(crate) fn decode(app: &mut FigApp, id: NodeId) -> Option<std::sync::Arc<Decoded>> {
    use std::hash::{Hash, Hasher};
    let doc = app.session.doc().ok()?;
    let n = doc.node(id)?;
    let sprite = store::load(&n.plugin_data)?;
    let layer = current_layer(&app.pixel, &sprite)?;
    let frame = app.pixel.frame.min(sprite.frames.len().saturating_sub(1));
    let frame_id = sprite.frames.get(frame)?.id;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (id, layer, frame).hash(&mut h);
    n.plugin_data.get(store::KEY).hash(&mut h);
    n.plugin_data.get(store::SELECTION_KEY).hash(&mut h);
    for l in &sprite.layers {
        n.plugin_data.get(&store::cel_key(l.id, frame_id)).hash(&mut h);
    }
    let key = h.finish();
    if let Some(d) = app.pixel.decoded.as_ref().filter(|d| d.key == key) {
        return Some(d.clone());
    }
    let index = sprite.layer_index(layer)?;
    let mut below = Canvas::new(sprite.width, sprite.height)?;
    let mut above = Canvas::new(sprite.width, sprite.height)?;
    for (i, l) in sprite.layers.iter().enumerate() {
        if i == index || !l.visible {
            continue;
        }
        let c = store::cel(&n.plugin_data, &sprite, l.id, frame_id)?;
        if i < index { below.draw_over(&c, l.opacity) } else { above.draw_over(&c, l.opacity) }
    }
    let current = sprite.layers.get(index)?;
    let opacity = if current.visible { current.opacity } else { 0 };
    let cel = store::cel(&n.plugin_data, &sprite, layer, frame_id)?;
    let selection = store::selection(&n.plugin_data, &sprite);
    let edges = selection.as_ref().map(Mask::edges).unwrap_or_default();
    let d = std::sync::Arc::new(Decoded { key, sprite, layer, frame, cel, below, above, selection, edges, opacity });
    app.pixel.decoded = Some(d.clone());
    Some(d)
}

/// Asks for pointer `icon` this frame: the canvas then leaves it instead of showing the tool's.
pub(crate) fn set_cursor(app: &mut FigApp, ctx: &egui::Context, icon: egui::CursorIcon) {
    app.pixel.cursor = Some(icon);
    ctx.set_cursor_icon(icon);
}

/// Whether Pixel mode asked for its own pointer this frame ([`set_cursor`]).
pub(crate) fn has_cursor(app: &FigApp) -> bool {
    app.pixel.cursor.is_some()
}

/// Layer `id`'s art-pixel space to page space: (its world transform, page units per art pixel).
pub(crate) fn art_to_page(app: &FigApp, id: NodeId, sprite: &Sprite) -> Option<Affine> {
    let doc = app.session.doc().ok()?;
    let n = doc.node(id)?;
    let s = f64::from(sprite.scale.max(1));
    // G16.7: the layer hugs its used pixels, so the board starts that far up and left of it.
    let [x, y, ..] = figcraft_pixel::store::shown_box(&n.plugin_data, sprite);
    Some(doc.world_transform(id) * Affine::scale(s) * Affine::translate((-f64::from(x), -f64::from(y))))
}

/// The art pixel under screen point `pos` (may be outside the sprite).
pub(crate) fn art_at(app: &FigApp, id: NodeId, sprite: &Sprite, pos: Pos2) -> Option<Pt> {
    let xf = art_to_page(app, id, sprite)?;
    if xf.determinant().abs() < 1e-12 {
        return None;
    }
    let p = xf.inverse() * app.to_page(pos);
    (p.x.is_finite() && p.y.is_finite()).then(|| (p.x.floor().clamp(-1e6, 1e6) as i64, p.y.floor().clamp(-1e6, 1e6) as i64))
}

/// Art point (`x`, `y`, fractional) on screen.
pub(crate) fn art_to_screen(app: &FigApp, xf: Affine, x: f64, y: f64) -> Pos2 {
    let p = xf * Point::new(x, y);
    app.to_screen(p.x, p.y)
}

/// The sprite's box on screen (its four corners: top left, top right, bottom right, bottom left).
pub(crate) fn screen_quad(app: &FigApp, xf: Affine, w: f64, h: f64) -> [Pos2; 4] {
    [art_to_screen(app, xf, 0.0, 0.0), art_to_screen(app, xf, w, 0.0), art_to_screen(app, xf, w, h), art_to_screen(app, xf, 0.0, h)]
}

/// Screen pixels an art pixel spans (along its width).
pub(crate) fn zoom_of(app: &FigApp, xf: Affine) -> f32 {
    let a = art_to_screen(app, xf, 0.0, 0.0);
    let b = art_to_screen(app, xf, 1.0, 0.0);
    a.distance(b)
}

/// Brush size one step bigger or smaller (`[` and `]`).
pub(crate) fn step_brush(e: &mut Editor, up: bool) {
    let size = if up { e.brush.size.saturating_add(1) } else { e.brush.size.saturating_sub(1) };
    e.brush = Brush::new(size, e.brush.shape);
}

/// The other brush shape.
pub(crate) fn toggle_shape(e: &mut Editor) {
    let shape = if e.brush.shape == BrushShape::Square { BrushShape::Circle } else { BrushShape::Square };
    e.brush = Brush::new(e.brush.size, shape);
}

/// The palette steps a stroke shades by (G16.6): the pencil in shade mode moves one colour on
/// with the main button and one back with the other; 0 paints as usual.
pub(crate) fn shade_step(e: &Editor, secondary: bool, erase: bool) -> i32 {
    match (e.shade && e.tool == Tool::Pencil && !erase, secondary) {
        (false, _) => 0,
        (true, false) => 1,
        (true, true) => -1,
    }
}
