//! Graybox pixel art (G16.4): a sprite's animation. Frames (add, duplicate, delete, move, how long
//! each shows) and tags (named runs of frames such as `idle` or `hover`, with the way they play).
//! Which tag a pixel layer plays in Prototype and Play is `pixel.set {play}`; switching what plays
//! on hover or press is a variant whose sprite plays another tag.

use figcraft_doc::NodeId;
use figcraft_pixel::sprite::{DURATION_RANGE, Direction, MAX_FRAMES, MAX_REPEAT, MAX_TAGS, Tag};
use figcraft_pixel::{Sprite, store};
use serde_json::{Value, json};

use super::{show, sprite_of};
use crate::automation::params::{get, req_id};
use crate::cmd::CommandSpec;
use crate::{Result, Session, bad, spec};

pub(super) fn specs() -> Vec<CommandSpec> {
    vec![
        spec!(
            "pixel.frame.add",
            "New frame",
            "{id, index? (default after the last), copy? (a frame index whose pixels it takes: Duplicate frame), duration? (ms; default the copied frame's, else 100)}  a tag holding the frame before it takes it in → {frame (index)}",
            true,
            frame_add
        ),
        spec!(
            "pixel.frame.delete",
            "Delete frames",
            "{id, frame (index) | frames [indices]}  never every frame; one undo step → {frames}",
            true,
            frame_delete
        ),
        spec!(
            "pixel.frame.move",
            "Move frame",
            "{id, frame (index), index (where it goes; refused when it is there)}  tags keep their places → {frame}",
            true,
            frame_move
        ),
        spec!(
            "pixel.frame.set",
            "Frame duration",
            "{id, frame? (index) | frames? [indices] (default every frame), duration (ms, 10–60000)} → {frames}",
            true,
            frame_set
        ),
        spec!(
            "pixel.tag.add",
            "New tag",
            "{id, from, to (frame indices), name? (default \"Tag N\"; made unique), direction? (forward|reverse|pingPong), repeat? (0 = loop)} → {name}",
            true,
            tag_add
        ),
        spec!(
            "pixel.tag.set",
            "Tag settings",
            "{id, tag (name), name?, from?, to?, direction? (forward|reverse|pingPong), repeat? (0 = loop)} → {name}",
            true,
            tag_set
        ),
        spec!("pixel.tag.delete", "Delete tag", "{id, tag (name)} → {tags}", true, tag_delete),
    ]
}

/// Saves `sprite` in layer `id` and shows its picture again.
fn save(s: &mut Session, id: NodeId, sprite: &Sprite) -> Result<()> {
    let doc = s.doc_mut()?;
    store::save(&mut doc.get_mut(id)?.plugin_data, sprite);
    show(doc, id, sprite);
    Ok(())
}

/// A frame index param (`key`), within the sprite's frames.
fn index(cmd: &str, sprite: &Sprite, p: &Value, key: &str) -> Result<usize> {
    get(p, key)
        .and_then(Value::as_u64)
        .and_then(|i| usize::try_from(i).ok())
        .filter(|i| *i < sprite.frames.len())
        .ok_or_else(|| bad(cmd, format!("`{key}` must be a frame index (0–{})", sprite.frames.len().saturating_sub(1))))
}

/// A whole number param `key` from 0 (none when absent), capped at `max`.
fn count(cmd: &str, p: &Value, key: &str, max: f64) -> Result<Option<f64>> {
    match get(p, key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_f64()
            .filter(|f| f.is_finite() && *f >= 0.0)
            .map(|f| Some(f.min(max).round()))
            .ok_or_else(|| bad(cmd, format!("`{key}` must be a whole number from 0"))),
    }
}

/// The duration param in ms, kept in range (None when absent).
fn duration(cmd: &str, p: &Value) -> Result<Option<u32>> {
    match get(p, "duration").filter(|v| !v.is_null()) {
        None => Ok(None),
        Some(v) => v
            .as_f64()
            .filter(|d| d.is_finite())
            .map(|d| Some(d.round().clamp(f64::from(DURATION_RANGE.0), f64::from(DURATION_RANGE.1)) as u32))
            .ok_or_else(|| bad(cmd, "`duration` must be a number of ms")),
    }
}

fn direction(cmd: &str, p: &Value) -> Result<Option<Direction>> {
    match get(p, "direction").filter(|v| !v.is_null()) {
        None => Ok(None),
        Some(v) => match v.as_str() {
            Some("forward") => Ok(Some(Direction::Forward)),
            Some("reverse") => Ok(Some(Direction::Reverse)),
            Some("pingPong" | "pingpong" | "ping-pong") => Ok(Some(Direction::PingPong)),
            _ => Err(bad(cmd, "`direction` must be forward, reverse or pingPong")),
        },
    }
}

fn frame_add(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "pixel.frame.add";
    let id = req_id(cmd, p, "id")?;
    let mut sprite = sprite_of(cmd, s.doc()?, id)?;
    let copy = match get(p, "copy").filter(|v| !v.is_null()) {
        None => None,
        Some(_) => Some(index(cmd, &sprite, p, "copy")?),
    };
    let at = count(cmd, p, "index", MAX_FRAMES as f64)?.map_or(sprite.frames.len(), |f| f as usize);
    let length = duration(cmd, p)?.or_else(|| copy.and_then(|c| sprite.frames.get(c)).map(|f| f.duration)).unwrap_or(100);
    let source = copy.and_then(|c| sprite.frames.get(c)).map(|f| f.id);
    let new = sprite.insert_frame(at, length).ok_or_else(|| bad(cmd, format!("a sprite has at most {MAX_FRAMES} frames")))?;
    let doc = s.doc_mut()?;
    let data = &mut doc.get_mut(id)?.plugin_data;
    if let Some(from) = source {
        // A duplicate takes the cels as they are stored (no decoding).
        for l in &sprite.layers {
            if let Some(text) = data.get(&store::cel_key(l.id, from)).cloned() {
                data.insert(store::cel_key(l.id, new), text);
            }
        }
    }
    save(s, id, &sprite)?;
    Ok(json!({"frame": sprite.frame_index(new)}))
}

/// The frames `frame` (one index) or `frames` (indices) name, in order and once each; every
/// frame when neither is given and `all` is true.
fn picked(cmd: &str, sprite: &Sprite, p: &Value, all: bool) -> Result<Vec<usize>> {
    let mut which: Vec<usize> = match (get(p, "frame").filter(|v| !v.is_null()), get(p, "frames").filter(|v| !v.is_null())) {
        (Some(_), _) => vec![index(cmd, sprite, p, "frame")?],
        (None, Some(Value::Array(a))) if !a.is_empty() => a
            .iter()
            .take(MAX_FRAMES)
            .map(|v| v.as_u64().and_then(|i| usize::try_from(i).ok()).filter(|i| *i < sprite.frames.len()))
            .collect::<Option<_>>()
            .ok_or_else(|| bad(cmd, "`frames` must be frame indices"))?,
        (None, None) if all => (0..sprite.frames.len()).collect(),
        _ => return Err(bad(cmd, "`frame` (an index) or `frames` (indices) must say which frames")),
    };
    which.sort_unstable();
    which.dedup();
    Ok(which)
}

fn frame_delete(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "pixel.frame.delete";
    let id = req_id(cmd, p, "id")?;
    let mut sprite = sprite_of(cmd, s.doc()?, id)?;
    let which = picked(cmd, &sprite, p, false)?;
    if which.len() >= sprite.frames.len() {
        return Err(bad(cmd, "a sprite keeps at least one frame"));
    }
    // Last first, so the indices still name the frames asked for.
    for i in which.iter().rev() {
        sprite.remove_frame(*i);
    }
    save(s, id, &sprite)?;
    Ok(json!({"frames": sprite.frames.len()}))
}

fn frame_move(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "pixel.frame.move";
    let id = req_id(cmd, p, "id")?;
    let mut sprite = sprite_of(cmd, s.doc()?, id)?;
    let i = index(cmd, &sprite, p, "frame")?;
    let to = count(cmd, p, "index", MAX_FRAMES as f64)?.ok_or_else(|| bad(cmd, "`index` must be where the frame goes"))? as usize;
    let to = to.min(sprite.frames.len().saturating_sub(1));
    if !sprite.move_frame(i, to) {
        return Err(bad(cmd, "the frame is already there"));
    }
    save(s, id, &sprite)?;
    Ok(json!({"frame": to}))
}

fn frame_set(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "pixel.frame.set";
    let id = req_id(cmd, p, "id")?;
    let mut sprite = sprite_of(cmd, s.doc()?, id)?;
    let length = duration(cmd, p)?.ok_or_else(|| bad(cmd, "`duration` must be a number of ms"))?;
    let which = picked(cmd, &sprite, p, true)?;
    for i in &which {
        if let Some(f) = sprite.frames.get_mut(*i) {
            f.duration = length;
        }
    }
    save(s, id, &sprite)?;
    Ok(json!({"frames": which.len()}))
}

/// `name` made unique among the sprite's tags (`idle`, `idle 2`, …), `except` the tag being renamed.
fn unique(sprite: &Sprite, name: &str, except: Option<&str>) -> String {
    let base: String = name.trim().chars().take(200).collect();
    let base = if base.is_empty() { "Tag".to_string() } else { base };
    let taken = |n: &str| sprite.tags.iter().any(|t| t.name == n && Some(t.name.as_str()) != except);
    if !taken(&base) {
        return base;
    }
    (2..).map(|k| format!("{base} {k}")).find(|n| !taken(n)).unwrap_or(base)
}

/// The tag named by `tag`.
fn tag_index(cmd: &str, sprite: &Sprite, p: &Value) -> Result<usize> {
    let name = get(p, "tag").and_then(Value::as_str).ok_or_else(|| bad(cmd, "`tag` must be a tag's name"))?;
    sprite.tags.iter().position(|t| t.name == name).ok_or_else(|| bad(cmd, format!("no tag called \"{name}\"")))
}

fn tag_add(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "pixel.tag.add";
    let id = req_id(cmd, p, "id")?;
    let mut sprite = sprite_of(cmd, s.doc()?, id)?;
    if sprite.tags.len() >= MAX_TAGS {
        return Err(bad(cmd, format!("a sprite has at most {MAX_TAGS} tags")));
    }
    let (a, b) = (index(cmd, &sprite, p, "from")?, index(cmd, &sprite, p, "to")?);
    let fallback = format!("Tag {}", sprite.tags.len() + 1);
    let name = unique(&sprite, get(p, "name").and_then(Value::as_str).unwrap_or(&fallback), None);
    let direction = direction(cmd, p)?.unwrap_or_default();
    let repeat = count(cmd, p, "repeat", f64::from(MAX_REPEAT))?.map_or(0, |r| r as u32);
    sprite.tags.push(Tag { name: name.clone(), from: a.min(b), to: a.max(b), direction, repeat });
    save(s, id, &sprite)?;
    Ok(json!({"name": name}))
}

fn tag_set(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "pixel.tag.set";
    let id = req_id(cmd, p, "id")?;
    let mut sprite = sprite_of(cmd, s.doc()?, id)?;
    let i = tag_index(cmd, &sprite, p)?;
    let old = sprite.tags.get(i).cloned().ok_or_else(|| bad(cmd, "no such tag"))?;
    let from = match get(p, "from").filter(|v| !v.is_null()) {
        Some(_) => index(cmd, &sprite, p, "from")?,
        None => old.from,
    };
    let to = match get(p, "to").filter(|v| !v.is_null()) {
        Some(_) => index(cmd, &sprite, p, "to")?,
        None => old.to,
    };
    let name = match get(p, "name").and_then(Value::as_str) {
        Some(n) => unique(&sprite, n, Some(&old.name)),
        None => old.name.clone(),
    };
    let direction = direction(cmd, p)?.unwrap_or(old.direction);
    let repeat = count(cmd, p, "repeat", f64::from(MAX_REPEAT))?.map_or(old.repeat, |r| r as u32);
    if sprite.play.as_deref() == Some(old.name.as_str()) {
        sprite.play = Some(name.clone());
    }
    if let Some(t) = sprite.tags.get_mut(i) {
        *t = Tag { name: name.clone(), from: from.min(to), to: from.max(to), direction, repeat };
    }
    save(s, id, &sprite)?;
    Ok(json!({"name": name}))
}

fn tag_delete(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "pixel.tag.delete";
    let id = req_id(cmd, p, "id")?;
    let mut sprite = sprite_of(cmd, s.doc()?, id)?;
    let i = tag_index(cmd, &sprite, p)?;
    let gone = sprite.tags.remove(i);
    if sprite.play.as_deref() == Some(gone.name.as_str()) {
        sprite.play = None;
    }
    save(s, id, &sprite)?;
    Ok(json!({"tags": sprite.tags.len()}))
}
