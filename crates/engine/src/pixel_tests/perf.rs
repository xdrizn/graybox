//! Pixel art (G16.6): how long the work done per stroke and per drawn frame takes on a big sprite
//! (256 × 256, 8 layers, 24 frames). Ignored in the normal run (timings depend on the machine):
//! `cargo test -p figcraft-engine --lib pixel_tests::perf -- --ignored --nocapture`.

use std::time::Instant;

use figcraft_doc::NodeId;
use serde_json::json;

use super::ex;
use crate::Session;

#[test]
#[ignore = "a timing report, run by hand"]
fn a_256_sprite_with_8_layers_and_24_frames_stays_quick() {
    let mut s = Session::new();
    let id = ex(&mut s, "pixel.create", json!({"width": 256, "height": 256, "scale": 2}))["id"].as_u64().unwrap();
    let mut layers = vec![1u64];
    for _ in 1..8 {
        layers.push(ex(&mut s, "pixel.layer.add", json!({"id": id}))["layer"].as_u64().unwrap());
    }
    // Every layer gets noisy art (worst case for the stored cels), then 23 copies of the frame.
    for (i, l) in layers.iter().enumerate() {
        let rows: Vec<String> = (0..256).map(|y| (0..256).map(|x| if (x * 7 + y * 13 + i * 5) % 11 < 4 { 'a' } else { 'b' }).collect()).collect();
        ex(&mut s, "pixel.paint", json!({"id": id, "layer": l, "rows": rows, "palette": {"a": format!("#{:02x}3050", i * 30), "b": "#e0c080"}}));
    }
    for _ in 1..24 {
        ex(&mut s, "pixel.frame.add", json!({"id": id, "copy": 0}));
    }
    let time = |label: &str, n: u32, f: &mut dyn FnMut()| {
        let t = Instant::now();
        for _ in 0..n {
            f();
        }
        eprintln!("{label}: {:.2} ms", t.elapsed().as_secs_f64() * 1000.0 / f64::from(n));
    };
    let mut k = 0;
    time("stroke (one undo step, image fill made again)", 10, &mut || {
        k += 1;
        ex(&mut s, "pixel.stroke", json!({"id": id, "points": [[k, 10], [k + 40, 60]], "size": 3, "color": "#ff0000", "layer": layers[3]}));
    });
    time("stroke on frame 6 (not the one at rest)", 10, &mut || {
        k += 1;
        ex(
            &mut s,
            "pixel.stroke",
            json!({"id": id, "points": [[k, 10], [k + 40, 60]], "size": 3, "color": "#ff0000", "layer": layers[3], "frame": 5}),
        );
    });
    let doc = s.doc().unwrap().clone();
    let n = doc.node(NodeId(id)).unwrap().clone();
    time("animation clock (per pixel layer, per presented frame)", 200, &mut || {
        let mut clock = None;
        figcraft_render::pixel::tick(&doc, &n, 1.25, &mut clock);
    });
    time("sprite description parsed", 200, &mut || {
        let _ = figcraft_pixel::store::load(&n.plugin_data);
    });
    let sprite = figcraft_pixel::store::load(&n.plugin_data).unwrap();
    time("frame hash (render cache key)", 200, &mut || {
        let _ = figcraft_pixel::store::frame_hash(&n.plugin_data, &sprite, 5);
    });
    let fid = sprite.frames[5].id;
    let cel = figcraft_pixel::store::cel(&n.plugin_data, &sprite, layers[3] as u32, fid).unwrap();
    time("one cel read", 50, &mut || {
        let _ = figcraft_pixel::store::cel(&n.plugin_data, &sprite, layers[3] as u32, fid);
    });
    let mut data = n.plugin_data.clone();
    time("one cel stored", 20, &mut || {
        figcraft_pixel::store::put_cel(&mut data, layers[3] as u32, fid, &cel);
    });
    time("document copied (the undo step)", 20, &mut || {
        let _ = doc.clone();
    });
    time("frame composed from 8 cels (render cache miss)", 10, &mut || {
        let _ = figcraft_pixel::store::frame_image(&n.plugin_data, &sprite, 5);
    });
    time("pixel fonts refreshed (every UI frame)", 200, &mut || {
        crate::cmd::pixel::font::refresh(&doc);
    });
}
