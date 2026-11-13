//! Pixel mode's settings and helpers kept outside the drawing itself (G16.6 follow-ups): the
//! timeline toggle the app remembers, and colours taken from the file's colour styles and
//! variables (a sprite records where each colour came from, so it can follow them), with palette
//! files made into colour styles.

use figcraft_doc::{Document, NodeId, StyleKind, VariableValue};
use figcraft_pixel::{Rgba, canvas, store};
use serde_json::{Value, json};

use crate::automation::params::{boolean, get};
use crate::cmd::CommandSpec;
use crate::{Result, Session, bad, spec};

/// The preference (a View toggle the app keeps) that shows the timeline over Pixel mode's pill.
pub const TIMELINE_PREF: &str = "graybox.pixelTimeline";
/// The most colours a palette file makes styles of.
const MAX_IMPORT: usize = 256;

pub(super) fn specs() -> Vec<CommandSpec> {
    vec![
        spec!(
            "pixel.timeline",
            "Show pixel timeline",
            "{on? (default: the other way)}  the animation timeline over Pixel mode's toolbar, kept for the app → {on}",
            false,
            timeline
        ),
        spec!(
            "pixel.styles.follow",
            "Update pixel colours from styles",
            "{ids? (pixel layers; default every one in the file)}  pixels painted from a colour style or variable that has changed since take its colour now, in every layer and frame → {changed, layers}",
            true,
            follow
        ),
        spec!(
            "pixel.palette.import",
            "Import a palette as colour styles",
            "{text (a .gpl or .hex palette), name? (the styles' folder, default the palette's Name or \"Palette\")}  one colour style per colour → {styles: [ids]}",
            true,
            import
        ),
    ]
}

/// Whether Pixel mode shows its timeline (off until turned on).
pub fn timeline_shown(s: &Session) -> bool {
    s.prefs.view.get(TIMELINE_PREF).copied().unwrap_or(false)
}

fn timeline(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "pixel.timeline";
    let on = match get(p, "on").filter(|v| !v.is_null()) {
        None => !timeline_shown(s),
        Some(_) => boolean(p, "on").ok_or_else(|| bad(cmd, "`on` must be true or false"))?,
    };
    s.prefs.view.insert(TIMELINE_PREF.to_string(), on);
    s.prefs_unsaved = true;
    Ok(json!({"on": on}))
}

/// The colour link `link` stands for now: a colour style's first solid paint, or a colour
/// variable's value in its collection's default mode. None when it is gone or not a colour.
pub fn link_color(doc: &Document, link: &str) -> Option<Rgba> {
    if let Some(id) = link.strip_prefix("style:") {
        return match &doc.style(id)?.kind {
            StyleKind::Paint { paints } => paints.iter().find_map(|p| p.solid_color()).map(|c| c.to_rgba8()),
            _ => None,
        };
    }
    let id = link.strip_prefix("var:")?;
    match doc.variables.resolve(id, &std::collections::BTreeMap::new())? {
        VariableValue::Color(c) => Some(c.to_rgba8()),
        _ => None,
    }
}

/// The `link` param (`style:<id>` or `var:<id>` naming a colour of this file), when given.
pub(super) fn link_param(cmd: &str, doc: &Document, p: &Value) -> Result<Option<String>> {
    match get(p, "link") {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_str()
            .filter(|l| link_color(doc, l).is_some())
            .map(|l| Some(l.to_string()))
            .ok_or_else(|| bad(cmd, "`link` must be style:<id> of a colour style or var:<id> of a colour variable in this file")),
    }
}

/// Records on pixel layer `id` that `color` came from `link`, when the link stands for exactly
/// that colour (another one would recolour the wrong pixels when it is followed).
pub(super) fn record(doc: &mut Document, id: NodeId, color: Rgba, link: Option<&str>) {
    let Some(link) = link else { return };
    if link_color(doc, link) != Some(color) {
        return;
    }
    let Some(n) = doc.node_mut(id) else { return };
    let Some(mut sprite) = store::load(&n.plugin_data) else { return };
    let key = canvas::to_hex(color);
    if sprite.links.get(&key).map(String::as_str) == Some(link) {
        return;
    }
    sprite.links.insert(key, link.to_string());
    store::save(&mut n.plugin_data, &sprite);
}

/// After `from` became `to` everywhere in pixel layer `id`: `from`'s link goes, and `to` takes
/// `link` when given.
pub(super) fn relink(doc: &mut Document, id: NodeId, from: Rgba, to: Rgba, link: Option<&str>) {
    if let Some(n) = doc.node_mut(id)
        && let Some(mut sprite) = store::load(&n.plugin_data)
        && sprite.links.remove(&canvas::to_hex(from)).is_some()
    {
        store::save(&mut n.plugin_data, &sprite);
    }
    record(doc, id, to, link);
}

/// The pixel layers `ids` names, or every one in the file (an instance shows its main's pixels).
fn pixel_layers(cmd: &str, doc: &Document, p: &Value) -> Result<Vec<NodeId>> {
    if let Some(list) = get(p, "ids").filter(|v| !v.is_null()) {
        let ids: Option<Vec<NodeId>> = list.as_array().and_then(|a| a.iter().take(4096).map(|v| v.as_u64().map(NodeId)).collect());
        let ids = ids.ok_or_else(|| bad(cmd, "`ids` must be a list of layer ids"))?;
        for id in &ids {
            super::sprite_of(cmd, doc, *id)?;
        }
        return Ok(ids);
    }
    Ok(doc.nodes().filter(|n| store::is_pixel(&n.plugin_data) && !matches!(n.kind, figcraft_doc::NodeKind::Instance { .. })).map(|n| n.id).collect())
}

fn follow(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "pixel.styles.follow";
    let ids = pixel_layers(cmd, s.doc()?, p)?;
    let (mut changed, mut layers) = (0, 0);
    for id in ids {
        let sprite = super::sprite_of(cmd, s.doc()?, id)?;
        // Each link's colour then and now; a link to something gone is dropped.
        let mut moves = Vec::new();
        let mut gone = Vec::new();
        for (hex, link) in &sprite.links {
            match (canvas::parse_hex(hex), link_color(s.doc()?, link)) {
                (Some(from), Some(to)) if from != to => moves.push((from, to, link.clone())),
                (Some(_), Some(_)) => {}
                _ => gone.push(hex.clone()),
            }
        }
        if moves.is_empty() && gone.is_empty() {
            continue;
        }
        let mut n = 0;
        for (from, to, _) in &moves {
            let now = super::sprite_of(cmd, s.doc()?, id)?;
            n += super::edit::replace_everywhere(s, id, &now, *from, *to, 0)?;
        }
        let doc = s.doc_mut()?;
        if let Some(node) = doc.node_mut(id)
            && let Some(mut now) = store::load(&node.plugin_data)
        {
            for hex in &gone {
                now.links.remove(hex);
            }
            for (from, to, link) in &moves {
                now.links.remove(&canvas::to_hex(*from));
                now.links.insert(canvas::to_hex(*to), link.clone());
            }
            store::save(&mut node.plugin_data, &now);
        }
        changed += n;
        layers += usize::from(n > 0);
    }
    Ok(json!({"changed": changed, "layers": layers}))
}

fn import(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "pixel.palette.import";
    let text = get(p, "text").and_then(Value::as_str).ok_or_else(|| bad(cmd, "`text` must be the palette file's text"))?;
    let named = figcraft_pixel::palette::parse_named(text);
    if named.is_empty() {
        return Err(bad(cmd, "the palette has no colours in it (.gpl or .hex)"));
    }
    let folder = get(p, "name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(str::to_string)
        .or_else(|| figcraft_pixel::palette::name(text))
        .unwrap_or_else(|| "Palette".to_string());
    let folder: String = folder.chars().filter(|c| *c != '/').take(64).collect();
    let doc = s.doc_mut()?;
    let mut ids = Vec::new();
    for (i, (color, name)) in named.into_iter().take(MAX_IMPORT).enumerate() {
        let name: String = name.map(|n| n.replace('/', "-")).filter(|n| !n.is_empty()).unwrap_or_else(|| (i + 1).to_string());
        let [r, g, b, a] = color;
        let paint = figcraft_color::Paint::solid(figcraft_color::Color::from_rgba8(r, g, b, a));
        let id = doc.new_style_id();
        doc.styles.push(figcraft_doc::Style {
            id: id.clone(),
            name: format!("{folder}/{name}"),
            description: String::new(),
            kind: StyleKind::Paint { paints: vec![paint] },
            bound: std::collections::BTreeMap::new(),
        });
        ids.push(id);
    }
    Ok(json!({"styles": ids}))
}
