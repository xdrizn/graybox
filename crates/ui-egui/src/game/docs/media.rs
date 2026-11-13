//! Pictures in the document editor (G13.2): an image block shows its picture as wide as the column
//! allows (never wider than the picture itself, never taller than [`MAX_H`]); an empty one asks for a
//! picture; image files dropped on an open document go in as image blocks where they were dropped,
//! and a picture pasted goes in after the block written in.

use egui::{Align2, Color32, ColorImage, CornerRadius, Id, Rect, Sense, Stroke, StrokeKind, TextureHandle, Ui, pos2, vec2};
use figcraft_color::ImageFilters;
use figcraft_doc::docs::{BlockId, DocId};
use serde_json::{Value, json};

use super::blocks::View;
use super::state::State;
use crate::FigApp;
use crate::a11y;
use crate::i18n::tr;
use crate::theme::{Tokens, regular};

/// Image files a document takes.
const IMAGE_FILES: [&str; 7] = ["png", "jpg", "jpeg", "webp", "gif", "tif", "tiff"];
/// The tallest a picture shows, in points.
const MAX_H: f32 = 560.0;
/// The height of an empty image block.
const EMPTY_H: f32 = 72.0;

#[cfg(test)]
thread_local! {
    static CHOOSE: std::cell::Cell<Option<Rect>> = const { std::cell::Cell::new(None) };
}

/// Where the last "Choose image" button of an empty image block was drawn (tests).
#[cfg(test)]
pub(super) fn last_choose() -> Option<Rect> {
    CHOOSE.with(std::cell::Cell::get)
}

/// Image block `block` showing `image` (its key in the file's images; empty: no picture yet), its left
/// edge `indent` into the column `col`.
#[allow(clippy::too_many_arguments)]
pub(super) fn image(
    app: &mut FigApp,
    ui: &mut Ui,
    t: &Tokens,
    doc: DocId,
    block: BlockId,
    image: &str,
    alt: &str,
    col: Rect,
    indent: f32,
    editable: bool,
) {
    let room = (col.width() - indent).max(40.0);
    let size = app.session.doc().ok().and_then(|d| d.images.get(image)).map(|i| (i.width, i.height));
    let Some((w, h)) = size.filter(|(w, h)| *w > 0 && *h > 0) else {
        empty(app, ui, t, doc, block, col, indent, editable);
        return;
    };
    let (w, h) = (w as f32, h as f32);
    let k = (room / w).min(MAX_H / h).min(1.0);
    let shown = vec2(w * k, h * k);
    let (row, resp) = ui.allocate_exact_size(vec2(col.width(), shown.y + 8.0), Sense::hover());
    let r = Rect::from_min_size(pos2(row.min.x + indent, row.min.y + 4.0), shown);
    a11y::image(&resp, if alt.is_empty() { tr("Image") } else { alt });
    let ppp = f64::from(ui.ctx().pixels_per_point());
    match texture(app, ui.ctx(), image, f64::from(shown.x.max(shown.y)) * ppp) {
        Some(tex) => {
            ui.painter().image(tex.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        }
        None => {
            ui.painter().rect_filled(r, CornerRadius::same(4), t.field);
        }
    }
}

/// An image block with no picture yet: a "Choose image" button, or a dim box on a version that cannot
/// change.
#[allow(clippy::too_many_arguments)]
fn empty(app: &mut FigApp, ui: &mut Ui, t: &Tokens, doc: DocId, block: BlockId, col: Rect, indent: f32, editable: bool) {
    let (row, _) = ui.allocate_exact_size(vec2(col.width(), EMPTY_H + 8.0), Sense::hover());
    let r = Rect::from_min_max(pos2(row.min.x + indent, row.min.y + 4.0), pos2(row.max.x, row.max.y - 4.0));
    ui.painter().rect_filled(r, CornerRadius::same(6), t.field);
    ui.painter().rect_stroke(r, CornerRadius::same(6), Stroke::new(1.0, t.border), StrokeKind::Inside);
    if !editable {
        ui.painter().text(r.center(), Align2::CENTER_CENTER, tr("Image"), regular(12.0), t.text_tertiary);
        return;
    }
    let label = tr("Choose image");
    let galley = ui.painter().layout_no_wrap(label.to_string(), regular(12.0), t.text);
    let button = Rect::from_center_size(pos2(r.center().x, r.center().y - 9.0), vec2(galley.size().x + 28.0, 28.0));
    #[cfg(test)]
    CHOOSE.with(|c| c.set(Some(button)));
    let resp = ui.interact(button, Id::unique(("gbx_doc_image_choose", doc.0, block.0)), Sense::click());
    a11y::button(&resp, label);
    ui.painter().rect_filled(button, CornerRadius::same(6), if resp.hovered() { t.hover } else { t.panel });
    ui.painter().rect_stroke(button, CornerRadius::same(6), Stroke::new(1.0, t.border_strong), StrokeKind::Inside);
    ui.painter().galley(button.center() - galley.size() / 2.0, galley, t.text);
    ui.painter().text(pos2(r.center().x, button.max.y + 12.0), Align2::CENTER_CENTER, tr("or drop one here"), regular(11.0), t.text_tertiary);
    if resp.clicked() {
        choose(app, doc, block);
    }
}

/// Ask for a picture file for image block `block`.
pub(super) fn choose(app: &mut FigApp, doc: DocId, block: BlockId) {
    app.pick_file(tr("Choose image"), &IMAGE_FILES, true, move |app, file| {
        let mut params = json!({"doc": doc.0, "id": block.0});
        for key in ["path", "data"] {
            if let Some(v) = file.get(key) {
                params[key] = v.clone();
            }
        }
        if let Some(stem) = file_stem(&file) {
            params["alt"] = json!(stem);
        }
        app.run("doc.block.image", params);
    });
}

/// The name of a picked file without its folder and extension (the picture's first description).
fn file_stem(file: &Value) -> Option<String> {
    let name = file.get("name").or_else(|| file.get("path")).and_then(Value::as_str)?;
    std::path::Path::new(name).file_stem().map(|s| s.to_string_lossy().to_string()).filter(|s| !s.is_empty())
}

/// The picture `hash` as a texture at most `side` device pixels on its longer side, kept between
/// frames while that size holds.
pub(super) fn texture(app: &FigApp, ctx: &egui::Context, hash: &str, side: f64) -> Option<TextureHandle> {
    if !side.is_finite() || side <= 0.0 {
        return None;
    }
    // Sizes in steps, so resizing the window does not decode the picture again every frame.
    let step = (side / 256.0).ceil().max(1.0) * 256.0;
    let id = Id::unique(("gbx_doc_image_tex", hash));
    if let Some((_, tex)) = ctx.data(|d| d.get_temp::<(u64, TextureHandle)>(id)).filter(|(s, _)| *s == step as u64) {
        return Some(tex);
    }
    let doc = app.session.doc().ok()?;
    let pix = figcraft_render::images::shown(hash, doc.images.get(hash), &ImageFilters::default(), doc.color_profile, step)?;
    // No bigger than shown, and never past what the graphics card takes (egui refuses that).
    let limit = (step as usize).min(ctx.input(|i| i.max_texture_side)).max(1);
    let img = shrink(crate::render_worker::screen_image(&pix, doc.color_profile), limit);
    let tex = ctx.load_texture(format!("gbx-doc-image-{hash}"), img, egui::TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, (step as u64, tex.clone())));
    Some(tex)
}

/// `img` made at most `limit` pixels on its longer side, each new pixel the average of the block of
/// pixels it stands for.
fn shrink(img: ColorImage, limit: usize) -> ColorImage {
    let [w, h] = img.size;
    let k = w.max(h).div_ceil(limit.max(1));
    if k <= 1 {
        return img;
    }
    let (nw, nh) = (w.div_ceil(k), h.div_ceil(k));
    let mut out = Vec::with_capacity(nw * nh);
    for y in 0..nh {
        for x in 0..nw {
            let mut sum = [0u32; 4];
            let mut n = 0u32;
            for sy in y * k..((y + 1) * k).min(h) {
                for sx in x * k..((x + 1) * k).min(w) {
                    if let Some(p) = img.pixels.get(sy * w + sx) {
                        for (s, c) in sum.iter_mut().zip(p.to_array()) {
                            *s += u32::from(c);
                        }
                        n += 1;
                    }
                }
            }
            let avg = sum.map(|s| u8::try_from(s / n.max(1)).unwrap_or(u8::MAX));
            out.push(Color32::from_rgba_premultiplied(avg[0], avg[1], avg[2], avg[3]));
        }
    }
    ColorImage::new([nw, nh], out)
}

/// Files dropped on the open document: pictures go in as image blocks after the block under the
/// pointer (`rows` are the blocks' rows, in `views` order; at the end when the pointer is under them
/// all), one after the other; a dropped document opens as a file of its own, as on the canvas.
pub(super) fn take_drops(app: &mut FigApp, ctx: &egui::Context, doc: DocId, views: &[View], rows: &[Rect], area: Rect) {
    let dropped = ctx.input(|i| i.raw.dropped_files.clone());
    if dropped.is_empty() {
        crate::open::drop_overlay(ctx, area, tr("Drop images to add them here"));
        return;
    }
    // Windows sends no pointer position while files are dragged in: ask the system where it is.
    let at = app
        .services
        .cursor_screen_pixels()
        .and_then(|[x, y]| {
            let (inner, ppp) = ctx.input(|i| (i.viewport().inner_rect, i.pixels_per_point));
            let ppp = f64::from(ppp.max(0.01));
            let inner = inner?;
            let p = pos2((x / ppp) as f32 - inner.min.x, (y / ppp) as f32 - inner.min.y);
            (p.x.is_finite() && p.y.is_finite()).then_some(p)
        })
        .or_else(|| ctx.input(|i| i.pointer.hover_pos()));
    let mut place = drop_place(app, doc, views, rows, at.map(|p| p.y));
    for f in dropped {
        let path = f.path().to_path_buf();
        if crate::is_document(&path) {
            crate::pending_open::open_dropped(app, &f);
            continue;
        }
        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let picture = path.extension().and_then(|e| e.to_str()).is_some_and(|e| IMAGE_FILES.iter().any(|x| x.eq_ignore_ascii_case(e)));
        if !picture {
            continue;
        }
        let mut params = json!({"doc": doc.0, "index": place.1});
        if let Some(parent) = place.0 {
            params["parent"] = json!(parent.0);
        }
        if let Some(stem) = path.file_stem().map(|s| s.to_string_lossy().to_string()).filter(|s| !s.is_empty()) {
            params["alt"] = json!(stem);
        }
        let read = if path.exists() {
            params["path"] = json!(path.to_string_lossy());
            None
        } else {
            Some(crate::dropped_bytes(&f, &name))
        };
        let target = app.session.ds().ok().map(|d| crate::pending_open::ImportTarget { uid: d.uid, page: d.page, name: d.doc.name.clone() });
        app.ui.pending_opens.push(crate::pending_open::PendingOpen { label: name, params, command: "doc.block.image", target, read });
        place.1 += 1;
    }
}

/// A picture pasted while the document is shown (copied in another app: the clipboard holds no
/// text) goes in after the block written in, else after the last block selected, else at the end.
pub(super) fn take_paste(app: &mut FigApp, ctx: &egui::Context, st: &State, doc: DocId, views: &[View]) {
    if !ctx.input(|i| i.raw.events.iter().any(|e| matches!(e, egui::Event::PasteImage(_)))) {
        return;
    }
    let Some(png) = clipboard_picture(app) else { return };
    let after = st.writing.or_else(|| views.iter().rev().find(|v| st.picked.contains(&v.id)).map(|v| v.id));
    let end = || (None, views.iter().filter(|v| v.parent.is_none()).count());
    let (parent, index) = after.and_then(|id| super::blocks::place_after(app, doc, id)).unwrap_or_else(end);
    use base64::Engine as _;
    let mut params = json!({"doc": doc.0, "index": index, "data": base64::engine::general_purpose::STANDARD.encode(png)});
    if let Some(parent) = parent {
        params["parent"] = json!(parent.0);
    }
    app.run("doc.block.image", params);
}

/// The clipboard's picture as PNG bytes (None when it holds none, or one too big to take).
fn clipboard_picture(app: &mut FigApp) -> Option<Vec<u8>> {
    if let Some(png) = app.services.clipboard_get_png().filter(|png| png.len() <= crate::MAX_CLIPBOARD_PNG_BYTES) {
        return Some(png);
    }
    let (w, h, rgba) = app.services.clipboard_get_image()?;
    let img = image::RgbaImage::from_raw(w, h, rgba)?;
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).ok()?;
    Some(out.into_inner()).filter(|png| png.len() <= crate::MAX_CLIPBOARD_PNG_BYTES)
}

/// Where a picture dropped at height `y` goes: after the last block that starts above it, inside the
/// same block as that one; at the end of the document when there is none or no height.
fn drop_place(app: &FigApp, doc: DocId, views: &[View], rows: &[Rect], y: Option<f32>) -> (Option<BlockId>, usize) {
    let over = y.and_then(|y| views.iter().zip(rows).rev().find(|(_, r)| r.min.y <= y).map(|(v, _)| v.id));
    let end = || (None, views.iter().filter(|v| v.parent.is_none()).count());
    match over {
        Some(id) => super::blocks::place_after(app, doc, id).unwrap_or_else(end),
        None if y.is_some() && rows.first().is_some_and(|r| y.is_some_and(|y| y < r.min.y)) => (None, 0),
        None => end(),
    }
}
