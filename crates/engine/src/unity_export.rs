//! Graybox: export screens for Unity. Each frame sent there gets a folder (named after it) in the
//! chosen Unity export folder with `layout.json` and `sprites/*.png`:
//!
//! - a layer that paints (a fill, a stroke, an image, a shape) becomes a sprite of its own look,
//!   without the layers inside it, which follow as its children;
//! - a 9-sliced layer becomes one sprite at its base size, its layers included, with its cuts as the
//!   sprite's border (left, bottom, right, top, as Unity takes it);
//! - a text stays text (characters, font, size, colour, alignment);
//! - each layer keeps its box (relative to its parent, top-left origin) and its constraints, which
//!   become RectTransform anchors.
//!
//! The Graybox Live Link package for Unity (`integrations/unity/`) watches the folder and rebuilds the
//! screen when `layout.json` changes.

use figcraft_doc::{Document, HConstraint, Node, NodeId, NodeKind, VConstraint};
use figcraft_geom::Rect;
use serde_json::{Value, json};

use crate::automation::file_access;
use crate::{EngineError, Result};

/// How many layers one screen exports at most.
const MAX_LAYERS: usize = 4096;

/// Export screen (frame) `frame` of `doc` into `dir/<name>/`. Returns the folder written and how
/// many sprites it holds.
pub(crate) fn export(doc: &Document, frame: NodeId, dir: &str, scale: f64) -> Result<(String, usize)> {
    let root = doc.node(frame).ok_or_else(|| EngineError::Other("no such frame".into()))?;
    let name = folder_name(&root.name, frame);
    let out = std::path::Path::new(dir).join(&name);
    let sprites = out.join("sprites");
    file_access::create_dir_all(&sprites)?;
    let mut count = 0usize;
    let mut written = 0usize;
    let base = doc.world_bounds(frame).unwrap_or_default();
    let tree = node_json(doc, root, base, &sprites, scale, &mut count, &mut written, true)?;
    // The same layers in a flat list, each naming its parent (Unity's JsonUtility reads no deeper
    // than ten levels of nesting).
    let mut nodes = Vec::new();
    flatten(&tree, 0, &mut nodes);
    let layout = json!({
        "graybox": 1,
        "name": root.name,
        "id": frame.0,
        "width": root.width,
        "height": root.height,
        "scale": scale,
        "root": tree,
        "nodes": nodes,
    });
    let text = serde_json::to_string_pretty(&layout).map_err(|e| EngineError::Other(e.to_string()))?;
    file_access::write(out.join("layout.json"), text.as_bytes())?;
    Ok((out.to_string_lossy().to_string(), written))
}

/// A folder name from a frame's name: letters, digits, spaces, `-` and `_` kept; the id when
/// nothing is left.
fn folder_name(name: &str, id: NodeId) -> String {
    let clean: String = name.chars().map(|c| if c.is_alphanumeric() || matches!(c, ' ' | '-' | '_') { c } else { '_' }).collect();
    let clean = clean.trim().trim_matches('_').to_string();
    if clean.is_empty() { format!("Screen {}", id.0) } else { clean }
}

fn h_anchor(c: HConstraint) -> &'static str {
    match c {
        HConstraint::Left => "left",
        HConstraint::Right => "right",
        HConstraint::LeftRight => "stretch",
        HConstraint::Center => "center",
        HConstraint::Scale => "scale",
    }
}

fn v_anchor(c: VConstraint) -> &'static str {
    match c {
        VConstraint::Top => "top",
        VConstraint::Bottom => "bottom",
        VConstraint::TopBottom => "stretch",
        VConstraint::Center => "center",
        VConstraint::Scale => "scale",
    }
}

/// Whether `n` paints something of its own (not counting its layers).
fn paints(n: &Node) -> bool {
    n.fills.iter().any(|p| p.visible) || n.has_visible_stroke() || n.effects.iter().any(|e| e.visible)
}

/// A sprite of `id` (only its own look unless `with_layers`), over page box `b`, written as
/// `sprites/<id>.png`. Returns its file name.
fn sprite(doc: &Document, id: NodeId, b: Rect, with_layers: bool, sprites: &std::path::Path, scale: f64) -> Result<Option<String>> {
    let mut d = doc.clone();
    if let Some(n) = d.node_mut(id) {
        if !with_layers {
            n.children.clear();
        }
        // The layer's opacity goes to the layout (Unity applies it with a CanvasGroup, which its
        // layers inherit), not into the picture too.
        n.opacity = 1.0;
        // Its 3D rotation too: Unity turns the RectTransform (G17).
        n.plugin_data.remove(figcraft_doc::tilt::KEY);
    }
    let Some(pix) = figcraft_render::render_area(&d, &[id], b, scale) else { return Ok(None) };
    let png = figcraft_render::encode_png(&pix).map_err(EngineError::Other)?;
    let file = format!("{}.png", id.0);
    file_access::write(sprites.join(&file), &png)?;
    Ok(Some(format!("sprites/{file}")))
}

/// `v` and the layers inside it, top first, each without its children and with `parent` (0 for
/// the screen).
fn flatten(v: &Value, parent: u64, out: &mut Vec<Value>) {
    let mut one = v.clone();
    if let Some(o) = one.as_object_mut() {
        o.remove("children");
        o.insert("parent".into(), json!(parent));
    }
    out.push(one);
    let id = v["id"].as_u64().unwrap_or(0);
    for c in v["children"].as_array().into_iter().flatten() {
        flatten(c, id, out);
    }
}

#[allow(clippy::too_many_arguments)]
fn node_json(
    doc: &Document,
    n: &Node,
    parent_box: Rect,
    sprites: &std::path::Path,
    scale: f64,
    count: &mut usize,
    written: &mut usize,
    is_root: bool,
) -> Result<Value> {
    *count += 1;
    if *count > MAX_LAYERS {
        return Err(EngineError::Other(format!("a screen exports at most {MAX_LAYERS} layers")));
    }
    let b = doc.world_bounds(n.id).unwrap_or(parent_box);
    let mut v = json!({
        "id": n.id.0,
        "name": n.name,
        "type": n.kind.type_name().to_lowercase(),
        "x": b.x0 - parent_box.x0,
        "y": b.y0 - parent_box.y0,
        "width": b.width(),
        "height": b.height(),
        "opacity": n.opacity,
        "anchors": {"h": h_anchor(n.constraints.horizontal), "v": v_anchor(n.constraints.vertical)},
    });
    if is_root {
        v["anchors"] = json!({"h": "stretch", "v": "stretch"});
    }
    // A tilted layer (G17): Unity's own Euler X and Y (the same numbers), about the rect's centre.
    // Perspective shows with a Screen Space - Camera or World Space canvas.
    if let Some(t) = figcraft_doc::tilt::of(n) {
        v["rotation3d"] = json!({"x": t.x, "y": t.y, "perspective": t.perspective});
    }
    let mut children = Vec::new();
    match &n.kind {
        NodeKind::Text(t) => {
            let style = &t.style;
            let color = n.fills.iter().find_map(|p| p.solid_color()).map(|c| c.to_hex());
            v["text"] = json!({
                "characters": t.characters,
                "fontFamily": style.font_family,
                "fontStyle": style.font_style,
                "fontSize": style.font_size,
                "autoResize": json!(t.auto_resize),
                "color": color,
                "align": format!("{:?}", t.align_h).to_lowercase(),
                "alignVertical": match t.align_v {
                    figcraft_doc::TextAlignV::Top => "top",
                    figcraft_doc::TextAlignV::Middle => "center",
                    figcraft_doc::TextAlignV::Bottom => "bottom",
                },
            });
        }
        _ => {
            if let Some((slice, _)) = figcraft_doc::nine_slice::effective(doc, n.id) {
                // One sprite at the base size, layers included, its cuts as the border. Pixel art
                // is the layer's own look (its base is the sprite's size): its layers, like a
                // button's label, follow as children (G16).
                let pixel = figcraft_render::pixel::source(doc, n).is_some();
                let (bw, bh) = slice.base;
                let mut base_doc = doc.clone();
                if let Some(m) = base_doc.node_mut(n.id) {
                    m.width = bw;
                    m.height = bh;
                    m.plugin_data.remove(figcraft_doc::nine_slice::KEY);
                }
                let base_box = Rect::new(b.x0, b.y0, b.x0 + bw, b.y0 + bh);
                if let Some(file) = sprite(&base_doc, n.id, base_box, !pixel, sprites, scale)? {
                    *written += 1;
                    let [t, r, btm, l] = slice.insets;
                    v["sprite"] = json!(file);
                    v["border"] = json!([l * scale, btm * scale, r * scale, t * scale]);
                }
                if pixel {
                    for c in n.children.iter().filter_map(|c| doc.node(*c)).filter(|c| c.visible && !c.is_mask) {
                        children.push(node_json(doc, c, b, sprites, scale, count, written, false)?);
                    }
                }
            } else {
                let whole = matches!(n.kind, NodeKind::BooleanOp { .. } | NodeKind::Vector { .. }) || n.children.is_empty();
                // Its own look (or a whole vector or boolean group, drawn as one).
                let wants = paints(n) || (whole && !n.children.is_empty() && !is_root);
                if wants && let Some(file) = sprite(doc, n.id, b, whole, sprites, scale)? {
                    *written += 1;
                    v["sprite"] = json!(file);
                }
                if !whole {
                    for c in n.children.iter().filter_map(|c| doc.node(*c)).filter(|c| c.visible && !c.is_mask) {
                        children.push(node_json(doc, c, b, sprites, scale, count, written, false)?);
                    }
                }
            }
        }
    }
    // A pixel layer imports sharp, and an animated one brings its sheet (G16.5).
    crate::cmd::pixel::io::unity_layer(doc, n, sprites, scale, &mut v)?;
    if !children.is_empty() {
        v["children"] = Value::Array(children);
    }
    Ok(v)
}

#[cfg(test)]
mod tests {
    use crate::Session;
    use serde_json::json;

    /// A screen with a 9-sliced panel, an image-like shape and a label exports its layout and
    /// sprites: the panel one sprite with a border, the label as text, anchors from constraints.
    #[test]
    fn a_screen_exports_its_layout_and_sprites() {
        let dir = std::env::temp_dir().join(format!("graybox-unity-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut s = Session::new();
        let screen =
            s.execute("frame.create", &json!({"x": 0, "y": 0, "width": 400, "height": 300, "name": "Main Menu"})).unwrap()["id"].as_u64().unwrap();
        let panel = s
            .execute("frame.create", &json!({"parent": screen, "x": 250, "y": 200, "width": 120, "height": 60, "props": {"fill": "#e0503a"}}))
            .unwrap()["id"]
            .as_u64()
            .unwrap();
        s.execute("game.nineSlice.set", &json!({"ids": [panel], "insets": [10, 10, 10, 10]})).unwrap();
        s.execute("text.create", &json!({"text": "Play", "parent": screen, "x": 20, "y": 20})).unwrap();
        let out = s.execute("game.unity.export", &json!({"frame": screen, "dir": dir.to_string_lossy()})).unwrap();
        let folder = std::path::PathBuf::from(out["screens"][0]["folder"].as_str().unwrap());
        assert!(folder.ends_with("Main Menu"));
        let layout: serde_json::Value = serde_json::from_slice(&std::fs::read(folder.join("layout.json")).unwrap()).unwrap();
        let kids = layout["root"]["children"].as_array().unwrap();
        let p = kids.iter().find(|k| k["id"] == json!(panel)).unwrap();
        assert_eq!(p["border"], json!([10.0, 10.0, 10.0, 10.0]));
        assert_eq!(p["anchors"], json!({"h": "right", "v": "bottom"}), "anchored by where it sits");
        assert!(folder.join(p["sprite"].as_str().unwrap()).exists());
        let t = kids.iter().find(|k| k["text"].is_object()).unwrap();
        assert_eq!(t["text"]["characters"], json!("Play"));
        let flat = layout["nodes"].as_array().unwrap();
        assert_eq!(flat.len(), 3, "the screen, the panel and the label");
        assert!(flat.iter().filter(|n| n["id"] != json!(screen)).all(|n| n["parent"] == json!(screen)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A translucent layer exports its opacity in the layout only (Unity applies it with a
    /// CanvasGroup), so its sprite is opaque; a text says how its box resizes.
    #[test]
    fn opacity_stays_out_of_the_sprite_and_text_keeps_its_resize_mode() {
        let dir = std::env::temp_dir().join(format!("graybox-unity-opacity-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut s = Session::new();
        let screen =
            s.execute("frame.create", &json!({"x": 0, "y": 0, "width": 200, "height": 100, "name": "Fade"})).unwrap()["id"].as_u64().unwrap();
        let glass = s
            .execute(
                "frame.create",
                &json!({"parent": screen, "x": 10, "y": 10, "width": 50, "height": 50, "props": {"fill": "#ff0000", "opacity": 0.5}}),
            )
            .unwrap()["id"]
            .as_u64()
            .unwrap();
        let label = s.execute("text.create", &json!({"text": "Hi", "parent": screen, "x": 80, "y": 10})).unwrap()["id"].as_u64().unwrap();
        let wrapped = s
            .execute("text.create", &json!({"text": "Wraps", "parent": screen, "x": 80, "y": 50, "width": 60, "props": {"textAutoResize": "height"}}))
            .unwrap()["id"]
            .as_u64()
            .unwrap();
        let out = s.execute("game.unity.export", &json!({"frame": screen, "dir": dir.to_string_lossy()})).unwrap();
        let folder = std::path::PathBuf::from(out["screens"][0]["folder"].as_str().unwrap());
        let layout: serde_json::Value = serde_json::from_slice(&std::fs::read(folder.join("layout.json")).unwrap()).unwrap();
        let flat = layout["nodes"].as_array().unwrap();
        let node = |id: u64| flat.iter().find(|n| n["id"] == json!(id)).unwrap();
        assert_eq!(node(glass)["opacity"], json!(0.5));
        let png = std::fs::read(folder.join(node(glass)["sprite"].as_str().unwrap())).unwrap();
        let pix = figcraft_render::images::decode(&png).unwrap();
        let alpha = pix.data().get(3 + 4 * (pix.width() as usize * (pix.height() as usize / 2) + pix.width() as usize / 2)).copied().unwrap();
        assert_eq!(alpha, 255, "the sprite is drawn at full opacity");
        assert_eq!(node(label)["text"]["autoResize"], json!("widthAndHeight"));
        assert_eq!(node(wrapped)["text"]["autoResize"], json!("height"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A tilted health bar (G17) exports Unity's X and Y rotation, and its sprite is drawn flat
    /// (Unity turns it): opaque at all four corners of its box.
    #[test]
    fn a_tilted_bar_exports_its_rotation_and_a_flat_sprite() {
        let dir = std::env::temp_dir().join(format!("graybox-unity-tilt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut s = Session::new();
        let screen = s.execute("frame.create", &json!({"x": 0, "y": 0, "width": 400, "height": 200, "name": "Hud"})).unwrap()["id"].as_u64().unwrap();
        let bar = s
            .execute("frame.create", &json!({"parent": screen, "x": 20, "y": 20, "width": 200, "height": 40, "props": {"fill": "#30c050"}}))
            .unwrap()["id"]
            .as_u64()
            .unwrap();
        s.execute("node.rotation3d.set", &json!({"ids": [bar], "y": -30, "x": 5, "perspective": 800})).unwrap();
        let out = s.execute("game.unity.export", &json!({"frame": screen, "dir": dir.to_string_lossy()})).unwrap();
        let folder = std::path::PathBuf::from(out["screens"][0]["folder"].as_str().unwrap());
        let layout: serde_json::Value = serde_json::from_slice(&std::fs::read(folder.join("layout.json")).unwrap()).unwrap();
        let node = layout["nodes"].as_array().unwrap().iter().find(|n| n["id"] == json!(bar)).unwrap().clone();
        assert_eq!(node["rotation3d"], json!({"x": 5.0, "y": -30.0, "perspective": 800.0}));
        assert_eq!((node["width"].as_f64(), node["height"].as_f64()), (Some(200.0), Some(40.0)), "its flat box");
        let png = std::fs::read(folder.join(node["sprite"].as_str().unwrap())).unwrap();
        let pix = figcraft_render::images::decode(&png).unwrap();
        let (w, h) = (pix.width() as usize, pix.height() as usize);
        for (x, y) in [(1, 1), (w - 2, 1), (1, h - 2), (w - 2, h - 2)] {
            assert_eq!(pix.data()[(y * w + x) * 4 + 3], 255, "corner ({x}, {y}) painted: drawn flat");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
