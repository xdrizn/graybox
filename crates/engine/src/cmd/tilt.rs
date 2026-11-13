//! Graybox: 3D rotation of layers (G17), the `node.rotation3d.set` command. The tilt (X and Y
//! rotation, perspective) is stored in the layer's plugin data ([`figcraft_doc::tilt`]); the
//! renderer warps the layer by it and clicks go back through it.

use figcraft_doc::NodeKind;
use figcraft_doc::tilt::{KEY, Tilt};
use serde_json::{Value, json};

use super::CommandSpec;
use crate::{Result, Session, bad, spec};

pub fn specs() -> Vec<CommandSpec> {
    vec![spec!(
        "node.rotation3d.set",
        "Set 3D rotation",
        "{ids? (default: the selection), x? (degrees, -80..80: + tips the top away), y? (degrees, -80..80: + brings the right side near), perspective? (eye distance in px, 50..100000, default 1000), enabled? (false removes it; true adds a flat one to edit)} → {ids, rotation3d: {x, y, perspective} | null}",
        true,
        set
    )]
}

/// A number parameter, `None` when absent.
fn number(cmd: &str, p: &Value, name: &str) -> Result<Option<f64>> {
    match p.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v.as_f64().filter(|f| f.is_finite()).map(Some).ok_or_else(|| bad(cmd, format!("{name} must be a number"))),
    }
}

fn set(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "node.rotation3d.set";
    let ids = crate::automation::params::targets(s, p);
    if ids.is_empty() {
        return Err(bad(cmd, "Select a layer"));
    }
    let enabled = match p.get("enabled") {
        None | Some(Value::Null) => None,
        Some(v) => Some(v.as_bool().ok_or_else(|| bad(cmd, "enabled must be true or false"))?),
    };
    let (x, y, d) = (number(cmd, p, "x")?, number(cmd, p, "y")?, number(cmd, p, "perspective")?);
    let doc = s.doc_mut()?;
    let mut last = None;
    for id in &ids {
        let n = doc.get(*id)?;
        if n.is_page() || matches!(n.kind, NodeKind::Section { .. }) {
            return Err(bad(cmd, format!("\"{}\" can't turn in 3D: pick a frame, a component, an instance or a layer", n.name)));
        }
        let current = n.plugin_data.get(KEY).and_then(|s| Tilt::decode(s));
        let next = match enabled {
            Some(false) => None,
            _ if enabled == Some(true) || x.is_some() || y.is_some() || d.is_some() || current.is_some() => {
                let c = current.unwrap_or_default();
                Some(Tilt::new(x.unwrap_or(c.x), y.unwrap_or(c.y), d.unwrap_or(c.perspective)))
            }
            _ => None,
        };
        let node = doc.get_mut(*id)?;
        match next {
            Some(t) => {
                node.plugin_data.insert(KEY.into(), t.encode());
            }
            None => {
                node.plugin_data.remove(KEY);
            }
        }
        last = next;
    }
    let rotation = last.map(|t| json!({"x": t.x, "y": t.y, "perspective": t.perspective}));
    Ok(json!({"ids": ids, "rotation3d": rotation}))
}

#[cfg(test)]
mod tests {
    use crate::Session;
    use figcraft_doc::NodeId;
    use figcraft_doc::tilt::{KEY, Tilt};
    use serde_json::json;

    fn frame(s: &mut Session) -> NodeId {
        NodeId(s.execute("frame.create", &json!({"x": 0, "y": 0, "width": 200, "height": 40})).unwrap()["id"].as_u64().unwrap())
    }

    fn tilt(s: &Session, id: NodeId) -> Option<Tilt> {
        s.doc().unwrap().node(id).unwrap().plugin_data.get(KEY).and_then(|v| Tilt::decode(v))
    }

    /// The owner's health bar: left side near is Y negative. Fields set one at a time keep the
    /// others; off removes it; undo brings it back.
    #[test]
    fn a_frame_turns_in_3d_one_field_at_a_time() {
        let mut s = Session::new();
        let f = frame(&mut s);
        let r = s.execute("node.rotation3d.set", &json!({"ids": [f], "y": -30})).unwrap();
        assert_eq!(r["rotation3d"], json!({"x": 0.0, "y": -30.0, "perspective": 1000.0}));
        s.execute("node.rotation3d.set", &json!({"ids": [f], "x": 10, "perspective": 600})).unwrap();
        assert_eq!(tilt(&s, f), Some(Tilt::new(10.0, -30.0, 600.0)));
        s.execute("node.rotation3d.set", &json!({"ids": [f], "x": 500})).unwrap();
        assert_eq!(tilt(&s, f).map(|t| t.x), Some(80.0), "clamped to 80°");
        s.execute("node.rotation3d.set", &json!({"ids": [f], "enabled": false})).unwrap();
        assert_eq!(tilt(&s, f), None);
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(tilt(&s, f), Some(Tilt::new(80.0, -30.0, 600.0)));
    }

    /// A 200 × 40 bar at (100, 80), turned Y −45° at perspective 400 (left side near): drawn from
    /// x 114 to 260, 48.6 tall at the left, 34 at the right. A click hits what is drawn there; with
    /// a layer inside selected the bar shows flat and is hit flat.
    #[test]
    fn clicks_hit_what_the_tilt_draws_and_flat_while_editing_inside() {
        use figcraft_geom::Point;
        let mut s = Session::new();
        let f = NodeId(s.execute("frame.create", &json!({"x": 100, "y": 80, "width": 200, "height": 40})).unwrap()["id"].as_u64().unwrap());
        let left =
            NodeId(s.execute("shape.rectangle", &json!({"parent": f, "x": 0, "y": 0, "width": 100, "height": 40})).unwrap()["id"].as_u64().unwrap());
        s.execute("node.rotation3d.set", &json!({"ids": [f], "y": -45, "perspective": 400})).unwrap();
        s.execute("select.none", &json!({})).unwrap();
        let page = s.page().unwrap();
        let doc = s.doc().unwrap().clone();
        let hit = |s: &Session, x: f64, y: f64| s.hit(&doc, page, Point::new(x, y), 0.0);
        assert_eq!(hit(&s, 118.0, 78.0), Some(left), "above the flat box, inside the near end");
        assert_eq!(hit(&s, 290.0, 100.0), None, "inside the flat box, past the far end");
        s.execute("select.set", &json!({"ids": [left]})).unwrap();
        assert_eq!(hit(&s, 118.0, 78.0), None, "shown flat while a layer inside is selected");
        assert!(hit(&s, 290.0, 100.0).is_some());
        // "Select layer" (the context menu's list) goes by what is shown too.
        let at =
            |s: &mut Session, x: f64, y: f64| s.execute("select.layersAt", &json!({"x": x, "y": y})).unwrap()["layers"].as_array().unwrap().len();
        assert_eq!(at(&mut s, 118.0, 78.0), 0);
        assert_eq!(at(&mut s, 290.0, 100.0), 1);
        s.execute("select.none", &json!({})).unwrap();
        assert_eq!(at(&mut s, 118.0, 78.0), 2, "the bar and its left half, through the tilt");
    }

    #[test]
    fn enabling_adds_a_flat_tilt_to_edit_and_junk_is_refused() {
        let mut s = Session::new();
        let f = frame(&mut s);
        s.execute("node.rotation3d.set", &json!({"ids": [f], "enabled": true})).unwrap();
        assert_eq!(tilt(&s, f), Some(Tilt::default()));
        for junk in [json!({"ids": [f], "x": "ten"}), json!({"ids": [f], "enabled": 1}), json!({"ids": [f], "perspective": [1]})] {
            assert!(s.execute("node.rotation3d.set", &junk).is_err(), "{junk}");
        }
        let page = s.page().unwrap();
        assert!(s.execute("node.rotation3d.set", &json!({"ids": [page], "y": 10})).is_err());
        assert!(s.execute("node.rotation3d.set", &json!({"ids": [], "y": 10})).is_err());
    }
}
