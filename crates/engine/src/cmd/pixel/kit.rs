//! Graybox pixel art (G16.3): pixel art used for game UI. A pixel layer's 9-slice snaps to whole art
//! pixels and, on a frame with auto layout, gives its cuts as the padding; Pixel text draws a text
//! layer on the art-pixel grid; the Pixel button starter makes a three-state button component; a
//! palette swap becomes a new variant.

use figcraft_doc::nine_slice::{KEY as SLICE_KEY, NineSlice};
use figcraft_doc::{Document, NodeId, NodeKind};
use figcraft_pixel::{Canvas, Rgba, Sprite, canvas, store};
use figcraft_render::pixel::TEXT_KEY;
use serde_json::{Value, json};

use super::{rgba_of, show};
use crate::automation::params::{boolean, get, num, req_id, targets};
use crate::cmd::CommandSpec;
use crate::{Result, Session, bad, spec};

pub(super) fn specs() -> Vec<CommandSpec> {
    vec![
        spec!(
            "pixel.text",
            "Pixel text",
            "{ids? (text layers; default the selection), on? (default true), scale? (art scale 1–64; default the scale of the pixel art it sits in or beside, else 4)}  draws the text with no anti-aliasing, its glyphs on the art-pixel grid → {ids}",
            true,
            text
        ),
        spec!(
            "pixel.button",
            "Pixel button",
            "{x?, y?, text? (default \"Button\"), scale? (1–16, default 4)}  makes a component set of three pixel buttons (State=Default, Hover, Pressed): a frame with auto layout, 9-sliced pixel art as its background with its cuts as the padding, and a Pixel text label → {id (the set), variants}",
            false,
            button
        ),
        spec!(
            "pixel.paletteVariant",
            "Palette swap variant",
            "{id (a component or variant), value (e.g. \"Red team\"), property? (default \"Palette\"), swap: [[from, to], …] (colours)}  duplicates it as a variant and replaces the colours in every pixel layer in it → {id, changed}",
            false,
            palette_variant
        ),
    ]
}

/// The pixel scale of layer `id`'s sprite.
fn scale_of(doc: &Document, id: NodeId) -> Option<u32> {
    doc.node(id).and_then(|n| store::load(&n.plugin_data)).map(|s| s.scale.max(1))
}

/// Turns 9-slice on or off for pixel layer `id`, or sets its cuts: they snap to whole art pixels,
/// the base is the sprite's own size, a new slice cuts a quarter of the shorter side, and a frame
/// with auto layout takes the cuts as its padding (unless the sprite says not to). True when it
/// is 9-sliced after.
pub(crate) fn slice(doc: &mut Document, id: NodeId, enabled: Option<bool>, insets: Option<[f64; 4]>) -> Result<bool> {
    let cmd = "game.nineSlice.set";
    let n = doc.get(id)?;
    let sprite = store::load(&n.plugin_data).ok_or_else(|| bad(cmd, format!("\"{}\" has a sprite that can't be read", n.name)))?;
    let current = NineSlice::of(n);
    let on = match (enabled, current) {
        (Some(false), _) => false,
        (_, Some(_)) | (Some(true), None) => true,
        (None, None) => insets.is_some(),
    };
    if !on {
        doc.get_mut(id)?.plugin_data.remove(SLICE_KEY);
        return Ok(false);
    }
    let k = f64::from(sprite.scale.max(1));
    let quarter = f64::from((sprite.width.min(sprite.height) / 4).max(1)) * k;
    let raw = insets.or(current.map(|c| c.insets)).unwrap_or([quarter; 4]);
    let snapped = raw.map(|v| (v / k).round().max(0.0) * k);
    let base = (f64::from(sprite.width) * k, f64::from(sprite.height) * k);
    put_slice(doc, id, &sprite, NineSlice { insets: snapped, base }.clamped(), Pad::Keep(1.0))?;
    Ok(true)
}

/// How the padding follows new cuts.
enum Pad {
    /// It keeps what it had beyond the old cuts (the Pressed button's lower label), that extra
    /// scaled by the factor.
    Keep(f64),
    /// It becomes the cuts.
    Cuts,
}

/// Stores `s` on pixel layer `id` and, with auto layout and the sprite's say-so, its cuts as the
/// padding.
fn put_slice(doc: &mut Document, id: NodeId, sprite: &Sprite, s: NineSlice, pad: Pad) -> Result<()> {
    let n = doc.get_mut(id)?;
    let old = NineSlice::of(n);
    n.plugin_data.insert(SLICE_KEY.into(), s.encode());
    if sprite.slice_padding
        && let Some(l) = n.layout.as_mut()
    {
        let mut padding = s.insets;
        if let (Pad::Keep(k), Some(old)) = (pad, old) {
            for ((p, now), (had, cut)) in padding.iter_mut().zip(s.insets).zip(l.padding.iter().zip(old.insets)) {
                let extra = (had - cut) * k;
                if extra.is_finite() {
                    *p = (now + extra).max(0.0);
                }
            }
        }
        l.padding = padding;
    }
    Ok(())
}

/// After pixel layer `id`'s scale changed from `old` to `sprite.scale`: its 9-slice keeps its cuts
/// in art pixels.
pub(crate) fn rescale(doc: &mut Document, id: NodeId, sprite: &Sprite, old: u32) -> Result<()> {
    let Some(s) = NineSlice::of(doc.get(id)?) else { return Ok(()) };
    let (old, k) = (f64::from(old.max(1)), f64::from(sprite.scale.max(1)));
    let insets = s.insets.map(|v| (v / old).round() * k);
    let base = (f64::from(sprite.width) * k, f64::from(sprite.height) * k);
    put_slice(doc, id, sprite, NineSlice { insets, base }.clamped(), Pad::Keep(k / old))
}

/// The cuts again as the padding (after the padding option was turned on).
pub(crate) fn repad(doc: &mut Document, id: NodeId, sprite: &Sprite) -> Result<()> {
    match NineSlice::of(doc.get(id)?) {
        Some(s) => put_slice(doc, id, sprite, s, Pad::Cuts),
        None => Ok(()),
    }
}

/// Moves pixel layer `layer`'s sprite onto `frame` (its background, G16.3). False when `layer`
/// isn't a pixel layer; refused when the frame already has pixel art.
pub(crate) fn to_background(doc: &mut Document, layer: NodeId, frame: NodeId) -> Result<bool> {
    let cmd = "game.image.toBackground";
    let n = doc.get(layer)?;
    if !store::is_pixel(&n.plugin_data) {
        return Ok(false);
    }
    if store::is_pixel(&doc.get(frame)?.plugin_data) {
        return Err(bad(cmd, "that frame already has pixel art as its background"));
    }
    let prefix = format!("{}.", store::KEY);
    let moved: Vec<(String, String)> = n
        .plugin_data
        .iter()
        .filter(|(k, _)| k.as_str() == store::KEY || k.starts_with(&prefix) || k.as_str() == SLICE_KEY)
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let f = doc.get_mut(frame)?;
    for (k, v) in moved {
        f.plugin_data.entry(k).or_insert(v);
    }
    // A 9-slice the frame already had is now one in art pixels of the sprite.
    if NineSlice::of(doc.get(frame)?).is_some() {
        slice(doc, frame, None, None)?;
    }
    Ok(true)
}

fn text(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "pixel.text";
    let ids = targets(s, p);
    if ids.is_empty() {
        return Err(bad(cmd, "Select a text layer"));
    }
    let on = boolean(p, "on").unwrap_or(true);
    let given = match get(p, "scale").filter(|v| !v.is_null()) {
        None => None,
        Some(v) => Some(
            v.as_f64()
                .filter(|f| f.is_finite() && *f >= 1.0 && *f <= f64::from(figcraft_pixel::sprite::MAX_SCALE))
                .ok_or_else(|| bad(cmd, "`scale` must be a whole number from 1 to 64"))?
                .round() as u32,
        ),
    };
    let doc = s.doc_mut()?;
    for id in &ids {
        let n = doc.get(*id)?;
        if !matches!(n.kind, NodeKind::Text(_)) {
            return Err(bad(cmd, format!("\"{}\" isn't a text layer", n.name)));
        }
    }
    for id in &ids {
        let scale = given.or_else(|| around(doc, *id)).unwrap_or(4);
        let n = doc.get_mut(*id)?;
        if on {
            n.plugin_data.insert(TEXT_KEY.into(), scale.to_string());
        } else {
            n.plugin_data.remove(TEXT_KEY);
        }
    }
    Ok(json!({"ids": ids.iter().map(|i| i.0).collect::<Vec<_>>()}))
}

/// The scale of the pixel art text layer `id` sits in (its nearest pixel ancestor) or beside (a
/// pixel layer next to it).
fn around(doc: &Document, id: NodeId) -> Option<u32> {
    let parent = doc.node(id)?.parent?;
    let p = doc.node(parent)?;
    // Beside it in a frame (on the page, other art is no guide).
    let beside = if p.is_page() { None } else { p.children.iter().copied().filter(|c| *c != id).find_map(|c| scale_of(doc, c)) };
    beside.or_else(|| {
        let mut at = Some(parent);
        // The nearest pixel ancestor, at most 256 levels up.
        for _ in 0..256 {
            let here = at?;
            if let Some(k) = scale_of(doc, here) {
                return Some(k);
            }
            at = doc.node(here)?.parent;
        }
        None
    })
}

/// The art's size (art pixels) and its 9-slice cut on each side.
const ART: u32 = 12;
const CUT: u32 = 4;

/// One state's colours (original): outline, face, light edge, dark edge; pressed swaps the edges
/// and drops the label one art pixel.
struct Look {
    name: &'static str,
    outline: Rgba,
    face: Rgba,
    light: Rgba,
    dark: Rgba,
    pressed: bool,
}

fn hex(s: &str) -> Rgba {
    canvas::parse_hex(s).unwrap_or([0, 0, 0, 255])
}

fn looks() -> [Look; 3] {
    let outline = hex("#15172a");
    [
        Look { name: "Default", outline, face: hex("#3a63c4"), light: hex("#6e9cf2"), dark: hex("#243d82"), pressed: false },
        Look { name: "Hover", outline, face: hex("#4d7ae0"), light: hex("#8fb6ff"), dark: hex("#2c4a9a"), pressed: false },
        Look { name: "Pressed", outline, face: hex("#2f539f"), light: hex("#6e9cf2"), dark: hex("#1c2f66"), pressed: true },
    ]
}

/// The button's art: a box with its corners cut by one pixel, an outline, a light top-left edge and
/// a dark bottom-right edge (swapped when pressed).
fn art(look: &Look) -> Option<Canvas> {
    let mut c = Canvas::new(ART, ART)?;
    let last = i64::from(ART) - 1;
    let (lit, shade) = if look.pressed { (look.dark, look.light) } else { (look.light, look.dark) };
    for y in 0..=last {
        for x in 0..=last {
            let (xe, ye) = (x == 0 || x == last, y == 0 || y == last);
            if xe && ye {
                continue;
            }
            let inner_corner = (x == 1 || x == last - 1) && (y == 1 || y == last - 1);
            let color = if xe || ye || inner_corner {
                look.outline
            } else if y == last - 1 || x == last - 1 {
                shade
            } else if y == 1 || x == 1 {
                lit
            } else {
                look.face
            };
            c.set(x, y, color);
        }
    }
    Some(c)
}

fn id_in(v: &Value) -> Option<u64> {
    v.get("id").and_then(Value::as_u64)
}

/// One undo step: the nested commands are grouped (the command itself records none).
fn button(s: &mut Session, p: &Value) -> Result<Value> {
    s.grouped("Pixel button", |s| make_button(s, p))
}

fn make_button(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "pixel.button";
    let finite = |k: &str| num(p, k).filter(|f| f.is_finite()).map_or(0.0, |f| f.clamp(-1e6, 1e6).round());
    let (x, y) = (finite("x"), finite("y"));
    let label: String = get(p, "text").and_then(Value::as_str).filter(|t| !t.trim().is_empty()).unwrap_or("Button").chars().take(200).collect();
    let scale = num(p, "scale").filter(|f| f.is_finite()).map_or(4, |f| f.round().clamp(1.0, 16.0) as u32);
    let k = f64::from(scale);
    let side = f64::from(ART) * k;
    let cut = f64::from(CUT) * k;
    let mut mains = Vec::new();
    let mut top = y;
    for look in &looks() {
        let made = s.execute(
            "frame.create",
            &json!({
                "x": x, "y": top, "width": side, "height": side,
                "name": format!("State={}", look.name), "select": false, "props": {"fills": [], "clip": false},
            }),
        )?;
        let frame = NodeId(id_in(&made).ok_or_else(|| bad(cmd, "the frame wasn't made"))?);
        let sprite = Sprite::new(ART, ART, scale).ok_or_else(|| bad(cmd, "that scale can't be used"))?;
        let picture = art(look).ok_or_else(|| bad(cmd, "the art can't be drawn"))?;
        {
            let doc = s.doc_mut()?;
            let n = doc.get_mut(frame)?;
            store::save(&mut n.plugin_data, &sprite);
            if let (Some(l), Some(f)) = (sprite.layers.first(), sprite.frames.first()) {
                store::put_cel(&mut n.plugin_data, l.id, f.id, &picture);
            }
            show(doc, frame, &sprite);
        }
        let down = if look.pressed { k } else { 0.0 };
        s.execute("node.set", &json!({"ids": [frame.0], "props": {"layout": "horizontal", "mainAlign": "center", "crossAlign": "center"}}))?;
        s.execute("game.nineSlice.set", &json!({"ids": [frame.0], "insets": [cut, cut, cut, cut]}))?;
        if down > 0.0 {
            // Pressed: the label sits one art pixel lower.
            s.execute("node.set", &json!({"ids": [frame.0], "props": {"padding": [cut + down, cut, cut - down, cut]}}))?;
        }
        let t = s.execute(
            "text.create",
            &json!({"text": label, "x": 0, "y": 0, "parent": frame.0, "name": "Label", "select": false,
                "props": {"fontSize": 12.0 * k, "lineHeight": 14.0 * k, "fill": "#ffffff"}}),
        )?;
        if let Some(t) = id_in(&t) {
            s.execute("pixel.text", &json!({"ids": [t], "on": true, "scale": scale}))?;
        }
        s.execute("node.set", &json!({"ids": [frame.0], "props": {"sizingH": "hug", "sizingV": "hug"}}))?;
        // The next state goes below this one at its hugged height.
        top += s.doc()?.node(frame).map_or(side, |n| n.height) + 4.0 * k;
        let comp = s.execute("component.create", &json!({"ids": [frame.0]}))?;
        mains.push(id_in(&comp).unwrap_or(frame.0));
    }
    let set = id_in(&s.execute("component.combineVariants", &json!({"ids": mains}))?).ok_or_else(|| bad(cmd, "the variants weren't combined"))?;
    // The set doesn't clip: a state that grows with a longer label stays in view.
    s.execute("node.set", &json!({"ids": [set], "props": {"name": "Pixel button", "clip": false}}))?;
    s.set_selection(vec![NodeId(set)]);
    Ok(json!({"id": set, "variants": mains}))
}

fn palette_variant(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "pixel.paletteVariant";
    let id = req_id(cmd, p, "id")?;
    let value =
        get(p, "value").and_then(Value::as_str).map(str::trim).filter(|v| !v.is_empty()).ok_or_else(|| bad(cmd, "give the variant a `value`"))?;
    let property = get(p, "property").and_then(Value::as_str).map(str::trim).filter(|v| !v.is_empty()).unwrap_or("Palette");
    let pairs: Vec<(Rgba, Rgba)> = get(p, "swap")
        .and_then(Value::as_array)
        .map(|a| a.iter().take(256).filter_map(|pair| Some((rgba_of(pair.get(0)?)?, rgba_of(pair.get(1)?)?))).collect())
        .unwrap_or_default();
    if pairs.is_empty() {
        return Err(bad(cmd, "`swap` must list [from, to] colour pairs"));
    }
    if !matches!(s.doc()?.get(id)?.kind, NodeKind::Component { .. }) {
        return Err(bad(cmd, "pick a component or a variant"));
    }
    let (value, property) = (value.chars().take(100).collect::<String>(), property.chars().take(100).collect::<String>());
    s.grouped("Palette swap variant", |s| swap_variant(s, id, &property, &value, &pairs))
}

fn swap_variant(s: &mut Session, id: NodeId, property: &str, value: &str, pairs: &[(Rgba, Rgba)]) -> Result<Value> {
    let cmd = "pixel.paletteVariant";
    // A main component on its own becomes a set of one first.
    let in_set = |s: &Session| -> Result<Option<NodeId>> {
        let doc = s.doc()?;
        Ok(doc.get(id)?.parent.filter(|p| doc.node(*p).is_some_and(|n| matches!(n.kind, NodeKind::ComponentSet { .. }))))
    };
    let set = match in_set(s)? {
        Some(set) => set,
        None => NodeId(id_in(&s.execute("component.combineVariants", &json!({"ids": [id.0]}))?).ok_or_else(|| bad(cmd, "the set wasn't made"))?),
    };
    // A name no variant of the set has yet ("Red", then "Red 2"…).
    let value = {
        let doc = s.doc()?;
        let taken = |v: &str| {
            let name = format!("{property}={v}");
            doc.get(set)
                .map(|n| n.children.iter().any(|c| doc.node(*c).is_some_and(|m| m.name.split(", ").any(|part| part == name))))
                .unwrap_or(false)
        };
        (1..100).map(|i| if i == 1 { value.to_string() } else { format!("{value} {i}") }).find(|v| !taken(v)).unwrap_or_else(|| value.to_string())
    };
    let add = json!({"id": id.0, "property": property, "value": value});
    let dup = match s.execute("component.addVariant", &add) {
        Ok(v) => v,
        // The set has no such property yet: add it, then the variant.
        Err(_) => {
            s.execute("component.addProperty", &json!({"id": set.0, "name": property, "type": "variant"}))?;
            s.execute("component.addVariant", &add)?
        }
    };
    let dup = NodeId(id_in(&dup).ok_or_else(|| bad(cmd, "the variant wasn't made"))?);
    let layers: Vec<NodeId> = {
        let doc = s.doc()?;
        doc.subtree(dup).into_iter().filter(|i| doc.node(*i).is_some_and(|n| store::is_pixel(&n.plugin_data))).take(4096).collect()
    };
    let doc = s.doc_mut()?;
    let mut changed = 0;
    for layer in layers {
        let Some(sprite) = doc.node(layer).and_then(|n| store::load(&n.plugin_data)) else { continue };
        let mut here = 0;
        for l in &sprite.layers {
            for f in &sprite.frames {
                let data = &doc.get(layer)?.plugin_data;
                if !data.contains_key(&store::cel_key(l.id, f.id)) {
                    continue;
                }
                let Some(mut cel) = store::cel(data, &sprite, l.id, f.id) else { continue };
                let n = figcraft_pixel::transform::swap_colors(&mut cel, pairs);
                if n > 0 {
                    store::put_cel(&mut doc.get_mut(layer)?.plugin_data, l.id, f.id, &cel);
                    here += n;
                }
            }
        }
        if here > 0 {
            show(doc, layer, &sprite);
        }
        changed += here;
    }
    s.set_selection(vec![dup]);
    Ok(json!({"id": dup.0, "changed": changed}))
}
