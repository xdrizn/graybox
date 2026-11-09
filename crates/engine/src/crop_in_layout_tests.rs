//! Graybox: Ctrl-drag cropping inside auto layout (`canvas/tools/drag.rs` resize).

mod tests {
    use crate::Session;
    use crate::tools::{PointerEvent, PointerKind};
    use figcraft_color::{ImageScaleMode, PaintKind};
    use figcraft_doc::NodeId;
    use serde_json::json;

    /// Graybox: Ctrl-dragging an image's edge crops it (the picture stays put) also inside an auto
    /// layout frame.
    #[test]
    fn ctrl_drag_crops_an_image_inside_auto_layout() {
        let mut s = Session::new();
        let row = s.execute("frame.create", &json!({"x": 0, "y": 0, "width": 300, "height": 200, "props": {"layout": "horizontal"}})).unwrap();
        let row = NodeId(row["id"].as_u64().unwrap());
        let png = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";
        let img = s.execute("image.place", &json!({"data": png, "parent": row, "width": 100, "height": 100})).unwrap();
        let img = NodeId(img["id"].as_u64().unwrap());
        s.execute("select.set", &json!({"ids": [img]})).unwrap();
        for (kind, x) in [(PointerKind::Down, 100.0), (PointerKind::Move, 80.0), (PointerKind::Move, 60.0), (PointerKind::Up, 60.0)] {
            let mut e = PointerEvent::at(kind, x, 50.0, 1.0);
            e.cmd = true;
            s.pointer(&e).unwrap();
        }
        let n = s.doc().unwrap().node(img).unwrap().clone();
        assert!((n.width - 60.0).abs() < 1e-6, "{}", n.width);
        let Some(PaintKind::Image(paint)) = n.fills.first().map(|p| &p.kind) else { panic!("no image") };
        assert_eq!(paint.scale_mode, ImageScaleMode::Crop);
        let (x0, _, x1, _) = figcraft_color::ImagePaint::crop_rect(paint.crop.unwrap());
        assert!((x0 - 0.0).abs() < 1e-6 && (x1 - 100.0 / 60.0).abs() < 1e-6, "the picture keeps its 100 px: {x0}..{x1}");
    }
}
