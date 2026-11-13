//! Pictures in documents (G13.2): `/image` makes an empty image block that asks for a picture, a
//! picture shows no wider than itself, and image files dropped on the document go in where they land.

use egui::{Key, Shape};
use figcraft_doc::docs::BlockKind;
use serde_json::json;

use super::blocks_tests::{blocks, click, key, open, shapes, type_text};
use crate::layers_drag_tests::Ui;
use crate::live_tests::{has_text, text_center};

fn png(w: u32, h: u32) -> Vec<u8> {
    figcraft_render::encode_png(&figcraft_render::Pixmap::new(w, h).unwrap()).unwrap()
}

fn data(bytes: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// The kinds of the document's top blocks, by name, with the picture's key for images.
fn kinds(u: &Ui, doc: u64) -> Vec<String> {
    blocks(u, doc)
        .into_iter()
        .map(|b| match b.kind {
            BlockKind::Image { image, .. } => format!("image:{}", if image.is_empty() { "none" } else { "some" }),
            k => k.name(),
        })
        .collect()
}

/// The screen rectangles of the textures drawn (pictures).
fn pictures(s: &[Shape]) -> Vec<egui::Rect> {
    s.iter()
        .filter_map(|s| match s {
            Shape::Mesh(m) if m.texture_id != egui::TextureId::default() => {
                let r = egui::Rect::from_points(&m.vertices.iter().map(|v| v.pos).collect::<Vec<_>>());
                Some(r)
            }
            _ => None,
        })
        .collect()
}

#[test]
fn slash_image_makes_an_empty_image_block_that_asks_for_a_picture() {
    let (mut u, doc) = open(&[("paragraph", "")]);
    let at = text_center(&shapes(&mut u), "Type “/” for blocks");
    click(&mut u, at);
    type_text(&mut u, "/image");
    key(&mut u, Key::Enter);
    assert_eq!(kinds(&u, doc), ["image:none"]);
    let s = shapes(&mut u);
    assert!(has_text(&s, "Choose image"), "an empty image block asks for a picture");
    assert!(has_text(&s, "or drop one here"));
    assert!(super::media::last_choose().is_some());
}

#[test]
fn a_picture_shows_no_wider_than_itself_and_a_big_one_fits_the_column() {
    let (mut u, doc) = open(&[("paragraph", "Above")]);
    u.app.run("doc.block.image", json!({"doc": doc, "data": data(&png(40, 20))}));
    u.app.run("doc.block.image", json!({"doc": doc, "data": data(&png(3000, 1000))}));
    let s = shapes(&mut u);
    let shown = pictures(&s);
    assert_eq!(shown.len(), 2, "both pictures are drawn: {shown:?}");
    let small = shown[0];
    assert!((small.width() - 40.0).abs() < 0.5 && (small.height() - 20.0).abs() < 0.5, "{small:?}");
    let big = shown[1];
    assert!(big.width() < 1000.0 && big.width() > 200.0, "fits the column: {big:?}");
    assert!((big.width() / big.height() - 3.0).abs() < 0.05, "keeps its shape: {big:?}");
    assert!(big.min.y > small.max.y, "one under the other");
    // A picture is not a place to type: no stray keyboard target.
    assert!(!has_text(&s, "Choose image"));
}

/// A clipboard holding a picture copied in another app.
struct Clipboard(Vec<u8>);

impl crate::Services for Clipboard {
    fn clipboard_get_png(&mut self) -> Option<Vec<u8>> {
        Some(self.0.clone())
    }
}

fn layers(u: &Ui) -> usize {
    u.app.session.doc().unwrap().node_count()
}

#[test]
fn a_picture_pasted_in_a_block_goes_in_after_it_and_never_onto_the_canvas() {
    let (mut u, doc) = open(&[("paragraph", "First"), ("paragraph", "Second")]);
    u.app.services = Box::new(Clipboard(png(30, 10)));
    let before = layers(&u);
    let at = text_center(&shapes(&mut u), "First");
    click(&mut u, at);
    u.frame(vec![egui::Event::PasteImage(std::sync::Arc::default())]);
    u.frame(vec![]);
    assert_eq!(kinds(&u, doc), ["paragraph", "image:some", "paragraph"]);
    // With blocks selected (no words focused) it goes after the last of them.
    key(&mut u, Key::Escape);
    u.frame(vec![egui::Event::PasteImage(std::sync::Arc::default())]);
    u.frame(vec![]);
    assert_eq!(kinds(&u, doc), ["paragraph", "image:some", "image:some", "paragraph"]);
    // Nothing open but the document: still never onto the hidden canvas.
    u.frame(vec![egui::Event::PasteImage(std::sync::Arc::default())]);
    u.frame(vec![]);
    assert_eq!(layers(&u), before, "the canvas got nothing");
}

#[derive(Debug)]
struct Dropped(std::path::PathBuf, Vec<u8>);

impl egui::DroppedFile for Dropped {
    fn path(&self) -> &std::path::Path {
        &self.0
    }

    fn bytes(&self) -> Result<Vec<u8>, String> {
        Ok(self.1.clone())
    }
}

#[test]
fn image_files_dropped_on_the_document_go_in_after_the_block_under_the_pointer() {
    let (mut u, doc) = open(&[("paragraph", "First"), ("paragraph", "Second")]);
    let at = text_center(&shapes(&mut u), "First");
    // No path on disk (as in the browser): the bytes are read, then the blocks go in.
    let files = ["Mock-up.png", "notes.txt", "Map.PNG"]
        .iter()
        .map(|n| std::sync::Arc::new(Dropped(std::path::Path::new("gbx-doc-drop").join(n), png(8, 8))) as egui::DroppedFileHandle)
        .collect();
    u.frame_raw(egui::RawInput { events: vec![egui::Event::PointerMoved(at)], dropped_files: files, ..Default::default() });
    for _ in 0..6 {
        u.frame(vec![]);
    }
    assert_eq!(kinds(&u, doc), ["paragraph", "image:some", "image:some", "paragraph"]);
    let alts: Vec<String> = blocks(&u, doc)
        .into_iter()
        .filter_map(|b| match b.kind {
            BlockKind::Image { alt, .. } => Some(alt),
            _ => None,
        })
        .collect();
    assert_eq!(alts, ["Mock-up", "Map"], "named after their files, in the order dropped");
}
