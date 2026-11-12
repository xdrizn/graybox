//! Graybox pixel art (G16.6): `pixel.paint`, a sprite drawn from rows of characters, each one a
//! palette key. It is how Claude (chat, MCP, the CLI) draws a whole sprite in one step.

use std::collections::HashMap;

use figcraft_pixel::{Canvas, Rgba, canvas};
use serde_json::{Value, json};

use super::{MAX_COORD, cel_of, edit_cel, sprite_of};
use crate::automation::params::{boolean, get, num, req_id};
use crate::cmd::CommandSpec;
use crate::{Result, Session, bad, spec};

pub(super) fn specs() -> Vec<CommandSpec> {
    vec![spec!(
        "pixel.paint",
        "Paint pixel rows",
        "{id, rows: [\"..aa..\", …] (one character a pixel, top to bottom), palette: {\"a\": \"#c83447\", …} (one character → colour; `.` and space leave a pixel as it is), x?, y? (where the first row's first pixel goes, default 0, 0), clear? (empty the cel first), layer?, frame?}  draws a whole sprite at once, one undo step → {changed}",
        true,
        paint
    )]
}

/// `pixel.paint`: rows of characters, each a palette key, drawn at once (how Claude draws a sprite).
fn paint(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "pixel.paint";
    let id = req_id(cmd, p, "id")?;
    let sprite = sprite_of(cmd, s.doc()?, id)?;
    let palette: HashMap<char, Rgba> = match get(p, "palette") {
        None | Some(Value::Null) => HashMap::new(),
        Some(Value::Object(o)) => o
            .iter()
            .take(256)
            .map(|(k, v)| {
                let mut chars = k.chars();
                match (chars.next(), chars.next(), v.as_str().and_then(canvas::parse_hex)) {
                    (Some(c), None, Some(rgba)) => Ok((c, rgba)),
                    _ => Err(bad(cmd, "`palette` maps one character to a colour, e.g. {\"a\": \"#c83447\"}")),
                }
            })
            .collect::<Result<_>>()?,
        Some(_) => return Err(bad(cmd, "`palette` maps one character to a colour, e.g. {\"a\": \"#c83447\"}")),
    };
    let rows: Vec<&str> = match get(p, "rows") {
        Some(Value::Array(a)) if !a.is_empty() => a
            .iter()
            .take(Canvas::MAX_SIDE as usize)
            .map(Value::as_str)
            .collect::<Option<_>>()
            .ok_or_else(|| bad(cmd, "`rows` must be strings, one character a pixel"))?,
        _ => return Err(bad(cmd, "`rows` must be strings, one character a pixel")),
    };
    let at = |key: &str| num(p, key).filter(|f| f.is_finite()).map_or(0, |f| f.round().clamp(-MAX_COORD, MAX_COORD) as i64);
    let (x0, y0) = (at("x"), at("y"));
    let clear = boolean(p, "clear").unwrap_or(false);
    // Check every character first, so a typo changes nothing.
    for row in &rows {
        if let Some(c) = row.chars().find(|c| !matches!(c, '.' | ' ') && !palette.contains_key(c)) {
            return Err(bad(cmd, format!("`{c}` isn't in the palette (`.` and space leave a pixel as it is)")));
        }
    }
    let cel = cel_of(cmd, &sprite, p)?;
    let changed = edit_cel(s, id, &sprite, cel, |c| {
        let mut n = 0;
        if clear && let Some(empty) = Canvas::new(c.width(), c.height()) {
            n += c.to_bytes().chunks_exact(4).filter(|px| px[3] > 0).count();
            *c = empty;
        }
        for (dy, row) in rows.iter().enumerate() {
            for (dx, ch) in row.chars().take(Canvas::MAX_SIDE as usize).enumerate() {
                let Some(rgba) = palette.get(&ch) else { continue };
                let (x, y) = (x0 + dx as i64, y0 + dy as i64);
                if c.get(x, y) != Some(*rgba) && c.get(x, y).is_some() {
                    c.set(x, y, *rgba);
                    n += 1;
                }
            }
        }
        n
    })?;
    Ok(json!({"changed": changed}))
}
