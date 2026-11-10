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
    let layout = json!({
        "graybox": 1,
        "name": root.name,
        "id": frame.0,
        "width": root.width,
        "height": root.height,
        "scale": scale,
        "root": tree,
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
    if !with_layers && let Some(n) = d.node_mut(id) {
        n.children.clear();
    }
    let Some(pix) = figcraft_render::render_area(&d, &[id], b, scale) else { return Ok(None) };
    let png = figcraft_render::encode_png(&pix).map_err(EngineError::Other)?;
    let file = format!("{}.png", id.0);
    file_access::write(sprites.join(&file), &png)?;
    Ok(Some(format!("sprites/{file}")))
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
                "color": color,
                "align": format!("{:?}", t.align_h).to_lowercase(),
                "alignVertical": format!("{:?}", t.align_v).to_lowercase(),
            });
        }
        _ => {
            if let Some((slice, _)) = figcraft_doc::nine_slice::effective(doc, n.id) {
                // One sprite at the base size, layers included, its cuts as the border.
                let (bw, bh) = slice.base;
                let mut base_doc = doc.clone();
                if let Some(m) = base_doc.node_mut(n.id) {
                    m.width = bw;
                    m.height = bh;
                    m.plugin_data.remove(figcraft_doc::nine_slice::KEY);
                }
                let base_box = Rect::new(b.x0, b.y0, b.x0 + bw, b.y0 + bh);
                if let Some(file) = sprite(&base_doc, n.id, base_box, true, sprites, scale)? {
                    *written += 1;
                    let [t, r, btm, l] = slice.insets;
                    v["sprite"] = json!(file);
                    v["border"] = json!([l * scale, btm * scale, r * scale, t * scale]);
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
        let _ = std::fs::remove_dir_all(&dir);
    }
}
