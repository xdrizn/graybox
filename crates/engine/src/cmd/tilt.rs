//! Graybox: 3D rotation of layers (G17), the `node.rotation3d.set` command. The tilt (X and Y
//! rotation, perspective) is stored in the layer's plugin data ([`figcraft_doc::tilt`]); the
//! renderer warps the layer by it and clicks go back through it.

use std::cell::Cell;

use figcraft_doc::tilt::{self, Homography, KEY, Resize, Tilt};
use figcraft_doc::{Document, NodeId, NodeKind};
use figcraft_geom::{Affine, Point};
use serde_json::{Value, json};

use super::CommandSpec;
use crate::tools::{PointerEvent, PointerKind};
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

thread_local! {
    /// The handle of a tilted layer being dragged.
    static THROUGH: Cell<Option<Drag>> = const { Cell::new(None) };
    /// [`pointer`] is running the tools itself (they don't come back to it).
    static INSIDE: Cell<bool> = const { Cell::new(false) };
}

/// A handle of a tilted layer being dragged, as the press found it.
#[derive(Clone, Copy, Debug)]
struct Drag {
    /// The open document it belongs to (`DocState::uid`) and the layer.
    uid: u64,
    id: NodeId,
    /// Its warp taken back (drawn page point → flat page point) when the press came.
    back: Homography,
    /// A corner or side: the resize, its tilt, and the moving corner found last (start space).
    resize: Option<(Resize, Tilt, Point)>,
}

/// Where page point `p` reaches the handles of selection `sel`: the one selected tilted layer's
/// handles sit on its drawn corners, so `p` is taken back through its tilt (unless it shows flat).
/// `None` where nothing of it is drawn.
pub(crate) fn handle_point(doc: &Document, sel: &[NodeId], p: Point) -> Option<Point> {
    let [id] = sel else { return Some(p) };
    let Some(n) = doc.node(*id) else { return Some(p) };
    tilt::flat_point(doc, *id, n, p)
}

/// Whether the one selected layer is tilted: its corner radius and arc handles would sit on its
/// flat box, so it has none (the Design panel still sets them).
pub(crate) fn selected_tilted(s: &Session) -> bool {
    s.ds().is_ok_and(|ds| matches!(ds.selection[..], [id] if ds.doc.node(id).and_then(tilt::local).is_some()))
}

/// The pointer on a handle of the one selected tilted layer (and the rest of that drag): the tools
/// get a flat point, with the layer flat, so resizing and rotating work in its own flat frame. A
/// resize gets the point that leaves the dragged corner (or side) drawn under the pointer. `None`
/// (the tools take the event as it is) for everything else.
pub(crate) fn pointer(s: &mut Session, ev: &PointerEvent) -> Option<Result<()>> {
    if INSIDE.get() {
        return None;
    }
    // A drag that ended some other way (Escape, a command) holds nothing any more.
    let uid = s.ds().ok()?.uid;
    let held = THROUGH.get().filter(|d| d.uid == uid && !matches!(s.tool.gesture, crate::tools::Gesture::None));
    THROUGH.set(held);
    let mut drag = match held {
        Some(d) => d,
        None if ev.kind == PointerKind::Down => press(s, ev)?,
        None => return None,
    };
    let q = match drag.resize {
        // Where no corner puts it under the pointer, the last one found stays (no jump).
        Some((r, t, last)) => {
            let c = r.solve(&t, ev.p(), last).unwrap_or(last);
            drag.resize = Some((r, t, c));
            Some(r.parent * r.start * c)
        }
        None => None,
    };
    let q = q.or_else(|| drag.back.apply(ev.p())).unwrap_or(ev.p());
    let taken = PointerEvent { x: q.x, y: q.y, ..ev.clone() };
    let out = {
        let _inside = Inside::enter();
        tilt::with_flat(&[drag.id], || s.pointer(&taken))
    };
    let going = ev.kind != PointerKind::Up && !matches!(s.tool.gesture, crate::tools::Gesture::None);
    THROUGH.set(going.then_some(drag));
    Some(out)
}

/// [`INSIDE`] while it lives, cleared however the tools return (a panic the frame guard catches).
struct Inside;

impl Inside {
    fn enter() -> Self {
        INSIDE.set(true);
        Inside
    }
}

impl Drop for Inside {
    fn drop(&mut self) {
        INSIDE.set(false);
    }
}

/// A press on a handle of the one selected tilted layer: what dragging it needs.
fn press(s: &Session, ev: &PointerEvent) -> Option<Drag> {
    let ds = s.ds().ok()?;
    let [id] = ds.selection[..] else { return None };
    let back = tilt::page_warp(&ds.doc, id, &tilt::editing_flat(&ds.doc, &ds.selection))?.inverse()?;
    let q = back.apply(ev.p())?;
    let handle = tilt::with_flat(&[id], || crate::tools::handle_at(&ds.doc, &ds.selection, q, ev.zoom))?;
    let n = ds.doc.node(id)?;
    let t = tilt::of(n)?;
    let parent = n.parent.map_or(Affine::IDENTITY, |p| ds.doc.world_transform(p));
    let (w, h) = (n.width, n.height);
    let corners = [Point::new(0.0, 0.0), Point::new(w, 0.0), Point::new(w, h), Point::new(0.0, h)];
    let at = |i: u8| corners.get(usize::from(i % 4)).copied();
    // The corner or side that stays, and which coordinates the handle moves.
    let (anchor, moves) = match handle {
        crate::tools::Handle::Corner(i) => (at(i.wrapping_add(2))?, (true, true)),
        crate::tools::Handle::Edge(i) => (at(i.wrapping_add(2))?, (i % 2 == 1, i % 2 == 0)),
        crate::tools::Handle::Rotate(_) => return Some(Drag { uid: ds.uid, id, back, resize: None }),
    };
    let start = (parent * n.transform).inverse() * q;
    let r = Resize { parent, start: n.transform, size: (w, h), anchor, moves };
    Some(Drag { uid: ds.uid, id, back, resize: Some((r, t, start)) })
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

    /// The selected tilted bar's handles sit on its drawn corners (none where its flat corner
    /// would be), and dragging one resizes the bar in its own flat frame: the drawn top-right
    /// corner dragged right widens it, its left side stays put. One undo step.
    #[test]
    fn handles_sit_on_the_tilted_corners_and_resize_from_there() {
        use crate::tools::{Handle, PointerEvent, PointerKind, handle_at};
        use figcraft_geom::Point;
        let mut s = Session::new();
        let f = NodeId(s.execute("frame.create", &json!({"x": 100, "y": 80, "width": 200, "height": 40})).unwrap()["id"].as_u64().unwrap());
        s.execute("select.set", &json!({"ids": [f]})).unwrap();
        s.execute("view.zoomSelection", &json!({})).unwrap();
        assert!(!s.shape_handles().is_empty(), "flat: corner radius handles");
        s.execute("node.rotation3d.set", &json!({"ids": [f], "y": -45, "perspective": 400})).unwrap();
        assert!(s.shape_handles().is_empty(), "tilted: none (they'd sit on the flat box)");
        let doc = s.doc().unwrap().clone();
        let h = figcraft_doc::tilt::page_warp(&doc, f, &[]).unwrap();
        let tr = h.apply(Point::new(300.0, 80.0)).unwrap();
        assert!(tr.x < 265.0, "the far end is drawn short: {tr:?}");
        assert_eq!(handle_at(&doc, &[f], tr, 1.0), Some(Handle::Corner(1)));
        assert_eq!(handle_at(&doc, &[f], Point::new(300.0, 80.0), 1.0), None, "nothing drawn at the flat corner");
        let undo = s.ds().unwrap().history.undo.len();
        // Held still at the end, the canvas sends the same point each frame.
        let steps =
            [(PointerKind::Down, 0.0), (PointerKind::Move, 5.0)].into_iter().chain([(PointerKind::Move, 10.0); 6]).chain([(PointerKind::Up, 10.0)]);
        for (kind, dx) in steps {
            s.pointer(&PointerEvent::at(kind, tr.x + dx, tr.y, 1.0)).unwrap();
        }
        let n = s.doc().unwrap().node(f).unwrap().clone();
        let now = figcraft_doc::tilt::page_warp(s.doc().unwrap(), f, &[]).unwrap();
        let corner = now.apply(s.doc().unwrap().world_transform(f) * Point::new(n.width, 0.0)).unwrap();
        assert!((corner - Point::new(tr.x + 10.0, tr.y)).hypot() < 1.5, "the drawn corner follows the pointer: {corner:?}");
        assert!(n.width > 205.0, "widened: {}", n.width);
        assert!((figcraft_geom::translation(n.transform).x - 100.0).abs() < 0.5, "left side kept: {:?}", n.transform);
        assert_eq!(s.ds().unwrap().history.undo.len(), undo + 1);
        // The next press, away from the handles, isn't taken back through the tilt.
        s.pointer(&PointerEvent::at(PointerKind::Down, 600.0, 400.0, 1.0)).unwrap();
        s.pointer(&PointerEvent::at(PointerKind::Up, 600.0, 400.0, 1.0)).unwrap();
        assert!(s.ds().unwrap().selection.is_empty(), "a click on empty canvas deselects");
    }

    /// SVG and PDF can't warp: a tilted layer goes in as the renderer's picture of it, as tall as
    /// its near end (48.6 for this bar, 40 flat), and a flat one stays vectors.
    #[test]
    fn svg_and_pdf_embed_a_tilted_layer_as_its_picture() {
        let mut s = Session::new();
        let f = NodeId(s.execute("frame.create", &json!({"x": 100, "y": 80, "width": 200, "height": 40})).unwrap()["id"].as_u64().unwrap());
        s.execute("shape.rectangle", &json!({"parent": f, "x": 0, "y": 0, "width": 100, "height": 40})).unwrap();
        let count = |b: &[u8], what: &str| String::from_utf8_lossy(b).matches(what).count();
        let flat_svg = figcraft_svg::export_svg(s.doc().unwrap(), &[f], &Default::default()).unwrap();
        assert!(!flat_svg.contains("<image") && flat_svg.contains("<path"), "{flat_svg}");
        let flat_pdf = figcraft_pdf::export_nodes_pdf(s.doc().unwrap(), &[f]).unwrap();
        assert_eq!(count(&flat_pdf, "/Subtype/Image"), 0);
        s.execute("node.rotation3d.set", &json!({"ids": [f], "y": -45, "perspective": 400})).unwrap();
        let svg = figcraft_svg::export_svg(s.doc().unwrap(), &[f], &Default::default()).unwrap();
        assert!(svg.contains("<image") && !svg.contains("<path"), "{svg}");
        let height: f64 = svg.split("height=\"").nth(1).and_then(|r| r.split('"').next()).unwrap().parse().unwrap();
        assert!(height > 48.0, "the near end's height: {height}");
        let pdf = figcraft_pdf::export_nodes_pdf(s.doc().unwrap(), &[f]).unwrap();
        assert!(count(&pdf, "/Subtype/Image") >= 1);
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
