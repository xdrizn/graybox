//! Documents (G13): commands that make, name, move and delete documents and edit their blocks.
//!
//! Every edit is undoable and goes through the checked store in `figcraft_doc::docs`, so a closed
//! version refuses all of them. The menus, the Actions palette, the CLI, MCP (Claude) and the control
//! channel reach documents through these ids.

use figcraft_doc::NodeId;
use figcraft_doc::docs::{Block, BlockId, BlockKind, BlockStyle, DocError, DocId, Mark, Place, Run, TableOp};
use serde_json::{Map, Value, json};

use super::CommandSpec;
use crate::{Result, Session, bad, docs_render, spec};

mod version;

pub fn specs() -> Vec<CommandSpec> {
    let mut list = vec![
        spec!(
            "doc.get",
            "Show documents",
            "{id? (a document; omit for the tree of every document), version? (a version number; the last by default)} → the tree, or the document with its blocks",
            false,
            get
        ),
        spec!(
            "doc.create",
            "New document",
            "{title?, parent? (a document id; omit or null for the top level), index? (place among its siblings), icon?} → {id}",
            true,
            create
        ),
        spec!("doc.rename", "Rename document", "{id, title}", true, |s, p| edit(s, p, rename)),
        spec!(
            "doc.move",
            "Move document",
            "{id, parent? (a document id; omit or null for the top level), index? (place among the siblings left once it is lifted out)}",
            true,
            move_doc
        ),
        spec!("doc.delete", "Delete document", "{id} (also deletes the documents inside it) → {deleted: [ids]}", true, delete),
        spec!("doc.set_icon", "Set document icon", "{id, icon (an emoji or short symbol; empty removes it)}", true, |s, p| edit(s, p, set_icon)),
        spec!(
            "doc.block.insert",
            "Add block",
            "{doc, version?, parent? (a block id), index?, kind? (paragraph, heading1-4, bulleted, numbered, checklist, toggle, callout, quote, divider… or an object like {type: embed, node}), text?, runs? ([{text, bold?, italic?, underline?, strike?, code?, color?, highlight?, link?}]), style? ({font?, size?, color?, background?})} → {id}",
            true,
            |s, p| edit(s, p, insert_block)
        ),
        spec!(
            "doc.block.set",
            "Edit block",
            "{doc, id, version?, text? (plain text; keeps the look of a single run), runs?, style? ({font?, size?, color?, background?}; null clears one), data? (the kind's own fields, like {checked: true})}",
            true,
            |s, p| edit(s, p, set_block)
        ),
        spec!(
            "doc.block.turn_into",
            "Turn block into",
            "{doc, id, version?, kind (a name like heading2, or an object like {type: callout, icon}), text? (new plain text, as when a Markdown shortcut drops its marks; else the text stays)}",
            true,
            |s, p| edit(s, p, turn_into)
        ),
        spec!(
            "doc.block.split",
            "Split block",
            "{doc, id, version?, at (the cursor, in characters)} (Enter: the words after the cursor go to a new block after it) → {id (the new block), focus (the block the keyboard goes to)}",
            true,
            |s, p| edit(s, p, split_block)
        ),
        spec!(
            "doc.block.merge",
            "Join block to the one above",
            "{doc, id, version?} (Backspace at the start: its words join the block above) → {id (the block above), at (where the cursor goes)}",
            true,
            |s, p| edit(s, p, merge_block)
        ),
        spec!("doc.block.duplicate", "Duplicate block", "{doc, id, version?} (with what is inside it, right after it) → {id}", true, |s, p| edit(
            s,
            p,
            duplicate_block
        )),
        spec!(
            "doc.block.format",
            "Format text",
            "{doc, id, version?, from, to (a range of characters), mark (bold, italic, underline, strike, code, color, highlight, link), value? (#RRGGBB or #RRGGBBAA for color and highlight, a web address for link; omit to take it off), on? (bold…code: true or false; omit to toggle)}",
            true,
            |s, p| edit(s, p, format_text)
        ),
        spec!(
            "doc.table.cell",
            "Write in table cell",
            "{doc, id (a table block), version?, row, col (from 0), text (plain; words that stay keep their looks)}",
            true,
            |s, p| edit(s, p, table_cell)
        ),
        spec!(
            "doc.table.change",
            "Change table",
            "{doc, id (a table block), version?, op (insertRow, insertColumn, deleteRow, deleteColumn, header), at (a row or column from 0; required to delete; inserting past the end adds at the end, at the start when omitted), on? (header: true or false)}",
            true,
            |s, p| edit(s, p, table_change)
        ),
        spec!(
            "doc.block.image",
            "Add image to document",
            "{doc, version?, data (base64) | path, alt? (what it shows, for screen readers), id? (an image block: it shows this picture), parent?, index?} → {id, image, width, height}",
            true,
            |s, p| edit(s, p, block_image)
        ),
        spec!(
            "doc.block.embed",
            "Embed layer in document",
            "{doc, version?, node (a frame, component, variant or any layer), live? (default true: follows the layer; false: pinned, a picture of it as it is now), id? (an embed block: change its layer or live), parent?, index?} → {id, snapshot (the pinned picture's key, or null)}",
            true,
            |s, p| edit(s, p, block_embed)
        ),
        spec!("doc.block.move", "Move block", "{doc, id, version?, parent? (a block id; omit or null for the top), index?}", true, |s, p| edit(
            s, p, move_block
        )),
        spec!("doc.block.delete", "Delete block", "{doc, id, version?} (also deletes the blocks inside it)", true, |s, p| edit(s, p, delete_block)),
        spec!(
            "doc.export",
            "Export document",
            "{doc, version? (the last by default), format? (pdf|md; default pdf), path? (else returns base64 `data` for pdf, `text` for md)} → {path, bytes, pages?} | {format, data | text}",
            false,
            export
        ),
        spec!(
            "doc.card.create",
            "Put document on the canvas",
            "{doc, version? (the last by default), x?, y?} (a card on the current page that follows the document; a closed version's card is frozen with its seal) → {id}",
            true,
            card_create
        ),
        spec!(
            "doc.card.refresh",
            "Refresh document cards",
            "{id? (a card; omit for every card in the file)} lays the card out again from its document, also repairing pages deleted or moved by hand → {refreshed}",
            true,
            card_refresh
        ),
    ];
    list.extend(version::specs());
    list
}

/// Run an editing command, then lay out again every canvas card whose document it changed, in the
/// same step, so one undo takes back both the words and the card.
fn edit(s: &mut Session, p: &Value, run: fn(&mut Session, &Value) -> Result<Value>) -> Result<Value> {
    let out = run(s, p)?;
    // Block commands name the document `doc`; document commands name it `id`.
    let edited = p.get("doc").or_else(|| p.get("id")).and_then(Value::as_u64).map(DocId);
    if let Ok(d) = s.doc_mut() {
        docs_render::refresh_cards(d, edited);
    }
    Ok(out)
}

fn export(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "doc.export";
    let id = doc_id(cmd, p, "doc")?;
    let want = opt_version(cmd, p)?;
    let format = text(cmd, p, "format")?.unwrap_or_else(|| "pdf".into());
    let path = text(cmd, p, "path")?;
    let d = s.doc()?;
    let doc = d.docs.get(id).ok_or_else(|| refuse(cmd, DocError::NoDoc(id)))?;
    let version = match want {
        Some(n) => doc.version(n).ok_or_else(|| refuse(cmd, DocError::NoVersion(n)))?,
        None => doc.current().ok_or_else(|| refuse(cmd, DocError::NoVersion(0)))?,
    };
    let (bytes, pages) = match format.as_str() {
        "pdf" => docs_render::pdf(doc, version).map(|(b, n)| (b, Some(n)))?,
        "md" | "markdown" => (docs_render::markdown(doc, version).into_bytes(), None),
        _ => return Err(bad(cmd, "format must be pdf or md")),
    };
    if let Some(path) = path {
        crate::file_access::write(&path, &bytes)?;
        let mut out = json!({"path": path, "bytes": bytes.len()});
        if let Some(n) = pages {
            out["pages"] = json!(n);
        }
        return Ok(out);
    }
    if format == "pdf" {
        use base64::Engine as _;
        Ok(json!({"format": "pdf", "pages": pages, "data": base64::engine::general_purpose::STANDARD.encode(&bytes)}))
    } else {
        Ok(json!({"format": "md", "text": String::from_utf8_lossy(&bytes)}))
    }
}

fn card_create(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "doc.card.create";
    let id = doc_id(cmd, p, "doc")?;
    let want = opt_version(cmd, p)?;
    let num = |k: &str| p.get(k).and_then(Value::as_f64).filter(|v| v.is_finite()).unwrap_or(0.0);
    let page = s.page()?;
    let card = docs_render::create_card(s.doc_mut()?, page, id, want, (num("x"), num("y")))?;
    Ok(json!({"id": card}))
}

fn card_refresh(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "doc.card.refresh";
    let one = opt_uint(cmd, p, "id")?;
    let d = s.doc_mut()?;
    let cards = match one {
        Some(n) => vec![figcraft_doc::NodeId(n)],
        None => docs_render::cards(d),
    };
    let mut refreshed = 0;
    for c in cards {
        if docs_render::refresh_card(d, c, true)? {
            refreshed += 1;
        }
    }
    Ok(json!({"refreshed": refreshed}))
}

fn refuse(cmd: &str, e: DocError) -> crate::EngineError {
    bad(cmd, e.to_string())
}

/// A required whole-number parameter.
fn uint(cmd: &str, p: &Value, key: &str) -> Result<u64> {
    p.get(key).and_then(Value::as_u64).ok_or_else(|| bad(cmd, format!("{key} is required (a whole number)")))
}

/// An optional whole-number parameter: absent or null is `None`; any other type is refused.
fn opt_uint(cmd: &str, p: &Value, key: &str) -> Result<Option<u64>> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v.as_u64().map(Some).ok_or_else(|| bad(cmd, format!("{key} must be a whole number"))),
    }
}

fn opt_index(cmd: &str, p: &Value) -> Result<Option<usize>> {
    Ok(opt_uint(cmd, p, "index")?.map(|i| usize::try_from(i).unwrap_or(usize::MAX)))
}

fn opt_version(cmd: &str, p: &Value) -> Result<Option<u32>> {
    opt_uint(cmd, p, "version")?.map(|v| u32::try_from(v).map_err(|_| bad(cmd, "version is out of range"))).transpose()
}

fn text(cmd: &str, p: &Value, key: &str) -> Result<Option<String>> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(bad(cmd, format!("{key} must be text"))),
    }
}

fn runs(cmd: &str, p: &Value) -> Result<Option<Vec<Run>>> {
    match p.get("runs") {
        None | Some(Value::Null) => Ok(None),
        Some(v) => serde_json::from_value(v.clone()).map(Some).map_err(|e| bad(cmd, format!("runs must be a list of {{text, bold?, …}}: {e}"))),
    }
}

fn doc_id(cmd: &str, p: &Value, key: &str) -> Result<DocId> {
    uint(cmd, p, key).map(DocId)
}

fn tree_entry(s: &Session, id: DocId) -> Result<Value> {
    let d = s.doc()?;
    let doc = d.docs.get(id).ok_or_else(|| bad("doc.get", DocError::NoDoc(id).to_string()))?;
    let version = doc.current();
    Ok(json!({
        "id": id.0,
        "title": doc.title,
        "icon": doc.icon,
        "parent": doc.parent.map(|p| p.0),
        "children": doc.children.iter().map(|c| c.0).collect::<Vec<_>>(),
        "version": version.map(|v| v.number),
        "closed": version.is_some_and(|v| v.is_closed()),
        "blockCount": version.map_or(0, |v| v.block_count()),
    }))
}

fn get(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "doc.get";
    let Some(id) = opt_uint(cmd, p, "id")? else {
        let ids = s.doc()?.docs.walk();
        let docs = ids.into_iter().map(|i| tree_entry(s, i)).collect::<Result<Vec<_>>>()?;
        return Ok(json!({"documents": docs}));
    };
    let id = DocId(id);
    let want = opt_version(cmd, p)?;
    let d = s.doc()?;
    let doc = d.docs.get(id).ok_or_else(|| refuse(cmd, DocError::NoDoc(id)))?;
    let version = match want {
        Some(n) => doc.version(n).ok_or_else(|| refuse(cmd, DocError::NoVersion(n)))?,
        None => doc.current().ok_or_else(|| refuse(cmd, DocError::NoVersion(0)))?,
    };
    let mut out = tree_entry(s, id)?;
    if let Some(o) = out.as_object_mut() {
        o.insert("blocks".into(), serde_json::to_value(&version.blocks).map_err(|e| bad(cmd, e.to_string()))?);
        o.insert("viewing".into(), json!(version.number));
        o.insert("versions".into(), json!(doc.versions.iter().map(|v| v.number).collect::<Vec<_>>()));
    }
    Ok(out)
}

fn create(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "doc.create";
    let title = text(cmd, p, "title")?.unwrap_or_default();
    let icon = text(cmd, p, "icon")?;
    let parent = opt_uint(cmd, p, "parent")?.map(DocId);
    let index = opt_index(cmd, p)?;
    let docs = &mut s.doc_mut()?.docs;
    let id = docs.create(&title, parent, index).map_err(|e| refuse(cmd, e))?;
    if let Some(icon) = icon {
        docs.set_icon(id, &icon).map_err(|e| refuse(cmd, e))?;
    }
    Ok(json!({"id": id.0}))
}

fn rename(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "doc.rename";
    let id = doc_id(cmd, p, "id")?;
    let title = text(cmd, p, "title")?.ok_or_else(|| bad(cmd, "title is required"))?;
    s.doc_mut()?.docs.rename(id, &title).map_err(|e| refuse(cmd, e))?;
    Ok(json!({"ok": true}))
}

fn move_doc(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "doc.move";
    let id = doc_id(cmd, p, "id")?;
    let parent = opt_uint(cmd, p, "parent")?.map(DocId);
    let index = opt_index(cmd, p)?;
    s.doc_mut()?.docs.move_to(id, parent, index).map_err(|e| refuse(cmd, e))?;
    Ok(json!({"ok": true}))
}

fn delete(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "doc.delete";
    let id = doc_id(cmd, p, "id")?;
    let gone = s.doc_mut()?.docs.delete(id).map_err(|e| refuse(cmd, e))?;
    Ok(json!({"deleted": gone.iter().map(|g| g.0).collect::<Vec<_>>()}))
}

fn set_icon(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "doc.set_icon";
    let id = doc_id(cmd, p, "id")?;
    let icon = text(cmd, p, "icon")?.ok_or_else(|| bad(cmd, "icon is required (empty removes it)"))?;
    s.doc_mut()?.docs.set_icon(id, &icon).map_err(|e| refuse(cmd, e))?;
    Ok(json!({"ok": true}))
}

/// Read a `style` object into `style`: a key set to a value changes it, set to null clears it.
fn merge_style(cmd: &str, style: &mut BlockStyle, v: &Value) -> Result<()> {
    let o = v.as_object().ok_or_else(|| bad(cmd, "style must be an object like {font?, size?, color?, background?}"))?;
    if let Some(unknown) = o.keys().find(|k| !matches!(k.as_str(), "font" | "size" | "color" | "background")) {
        return Err(bad(cmd, format!("style has no {unknown}")));
    }
    let string = |k: &str| -> Result<Option<Option<String>>> {
        match o.get(k) {
            None => Ok(None),
            Some(Value::Null) => Ok(Some(None)),
            Some(Value::String(s)) => Ok(Some(Some(s.clone()))),
            Some(_) => Err(bad(cmd, format!("style.{k} must be text"))),
        }
    };
    if let Some(f) = string("font")? {
        style.font = f;
    }
    if let Some(c) = string("color")? {
        style.color = c;
    }
    if let Some(b) = string("background")? {
        style.background = b;
    }
    match o.get("size") {
        None => {}
        Some(Value::Null) => style.size = None,
        Some(n) => {
            let size = n.as_f64().ok_or_else(|| bad(cmd, "style.size must be a number"))?;
            style.size = Some(size as f32);
        }
    }
    if style.valid() { Ok(()) } else { Err(bad(cmd, "style has a value out of range (size 6-200, colours like #RRGGBB)")) }
}

fn insert_block(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "doc.block.insert";
    let doc = doc_id(cmd, p, "doc")?;
    let version = opt_version(cmd, p)?;
    let kind = match p.get("kind") {
        None | Some(Value::Null) => BlockKind::Paragraph,
        Some(v) => BlockKind::from_value(v).ok_or_else(|| bad(cmd, "kind is not a block kind (see the doc for names)"))?,
    };
    let place = Place { parent: opt_uint(cmd, p, "parent")?.map(BlockId), index: opt_index(cmd, p)? };
    let body = match (runs(cmd, p)?, text(cmd, p, "text")?) {
        (Some(_), Some(_)) => return Err(bad(cmd, "give text or runs, not both")),
        (Some(r), None) => r,
        (None, Some(t)) if !t.is_empty() => vec![Run::plain(t)],
        _ => Vec::new(),
    };
    let mut style = BlockStyle::default();
    if let Some(v) = p.get("style").filter(|v| !v.is_null()) {
        merge_style(cmd, &mut style, v)?;
    }
    let docs = &mut s.doc_mut()?.docs;
    let id = docs.insert_block(doc, version, place, kind, body).map_err(|e| refuse(cmd, e))?;
    if style != BlockStyle::default() {
        docs.update_block(doc, version, id, |b| {
            b.style = style;
            Ok(())
        })
        .map_err(|e| refuse(cmd, e))?;
    }
    Ok(json!({"id": id.0}))
}

/// The block's kind with `data`'s fields written over its own (`{"checked": true}`).
fn merge_kind(cmd: &str, kind: &BlockKind, data: &Value) -> Result<BlockKind> {
    let fields = data.as_object().ok_or_else(|| bad(cmd, "data must be an object with the kind's own fields"))?;
    let mut base: Map<String, Value> = match serde_json::to_value(kind) {
        Ok(Value::Object(o)) => o,
        _ => return Err(bad(cmd, "this block's data cannot be edited")),
    };
    for (k, v) in fields {
        if k == "type" {
            return Err(bad(cmd, "use doc.block.turn_into to change the kind"));
        }
        base.insert(k.clone(), v.clone());
    }
    serde_json::from_value::<BlockKind>(Value::Object(base)).map_err(|e| bad(cmd, format!("data does not fit a {} block: {e}", kind.name())))
}

fn set_block(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "doc.block.set";
    let doc = doc_id(cmd, p, "doc")?;
    let id = BlockId(uint(cmd, p, "id")?);
    let version = opt_version(cmd, p)?;
    let (body, plain) = (runs(cmd, p)?, text(cmd, p, "text")?);
    if body.is_some() && plain.is_some() {
        return Err(bad(cmd, "give text or runs, not both"));
    }
    let style = p.get("style").filter(|v| !v.is_null());
    let data = p.get("data").filter(|v| !v.is_null());
    if body.is_none() && plain.is_none() && style.is_none() && data.is_none() {
        return Err(bad(cmd, "nothing to change: give text, runs, style or data"));
    }
    // Why the style or data was refused, kept to tell the caller (the store only sees "invalid").
    let mut why = None;
    let result = s.doc_mut()?.docs.update_block(doc, version, id, |b: &mut Block| {
        if let Some(r) = body {
            b.text = r;
        }
        if let Some(t) = &plain {
            b.set_plain_text(t);
        }
        if let Some(v) = style
            && let Err(e) = merge_style(cmd, &mut b.style, v)
        {
            why = Some(e);
            return Err(DocError::Invalid("style is not valid"));
        }
        if let Some(d) = data {
            match merge_kind(cmd, &b.kind, d) {
                Ok(kind) => b.kind = kind,
                Err(e) => {
                    why = Some(e);
                    return Err(DocError::Invalid("data does not fit this block"));
                }
            }
        }
        Ok(())
    });
    if let Some(e) = why {
        return Err(e);
    }
    result.map_err(|e| refuse(cmd, e))?;
    Ok(json!({"ok": true}))
}

fn turn_into(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "doc.block.turn_into";
    let doc = doc_id(cmd, p, "doc")?;
    let id = BlockId(uint(cmd, p, "id")?);
    let version = opt_version(cmd, p)?;
    let kind = p
        .get("kind")
        .and_then(BlockKind::from_value)
        .ok_or_else(|| bad(cmd, "kind is required: a name like heading2, bulleted, checklist, or an object like {type: callout}"))?;
    let words = text(cmd, p, "text")?;
    s.doc_mut()?
        .docs
        .update_block(doc, version, id, |b| {
            b.kind = kind;
            if let Some(t) = &words {
                b.set_plain_text(t);
            }
            Ok(())
        })
        .map_err(|e| refuse(cmd, e))?;
    Ok(json!({"ok": true}))
}

fn split_block(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "doc.block.split";
    let doc = doc_id(cmd, p, "doc")?;
    let id = BlockId(uint(cmd, p, "id")?);
    let version = opt_version(cmd, p)?;
    let at = usize::try_from(uint(cmd, p, "at")?).unwrap_or(usize::MAX);
    let split = s.doc_mut()?.docs.split_block(doc, version, id, at).map_err(|e| refuse(cmd, e))?;
    Ok(json!({"id": split.new.0, "focus": split.focus.0}))
}

fn merge_block(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "doc.block.merge";
    let doc = doc_id(cmd, p, "doc")?;
    let id = BlockId(uint(cmd, p, "id")?);
    let version = opt_version(cmd, p)?;
    let merge = s.doc_mut()?.docs.merge_block(doc, version, id).map_err(|e| refuse(cmd, e))?;
    Ok(json!({"id": merge.into.0, "at": merge.at}))
}

fn duplicate_block(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "doc.block.duplicate";
    let doc = doc_id(cmd, p, "doc")?;
    let id = BlockId(uint(cmd, p, "id")?);
    let version = opt_version(cmd, p)?;
    let copy = s.doc_mut()?.docs.duplicate_block(doc, version, id).map_err(|e| refuse(cmd, e))?;
    Ok(json!({"id": copy.0}))
}

fn format_text(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "doc.block.format";
    let doc = doc_id(cmd, p, "doc")?;
    let id = BlockId(uint(cmd, p, "id")?);
    let version = opt_version(cmd, p)?;
    let at = |key| uint(cmd, p, key).map(|n| usize::try_from(n).unwrap_or(usize::MAX));
    let (from, to) = (at("from")?, at("to")?);
    let name = text(cmd, p, "mark")?.ok_or_else(|| bad(cmd, "mark is required: bold, italic, underline, strike, code, color, highlight or link"))?;
    let value = text(cmd, p, "value")?;
    let mark = Mark::from_name(&name, value.as_deref()).ok_or_else(|| bad(cmd, format!("unknown mark {name:?}")))?;
    let on = match p.get("on") {
        None | Some(Value::Null) => None,
        Some(Value::Bool(b)) => Some(*b),
        Some(_) => return Err(bad(cmd, "on must be true or false")),
    };
    s.doc_mut()?.docs.format_text(doc, version, id, from, to, &mark, on).map_err(|e| refuse(cmd, e))?;
    Ok(json!({"ok": true}))
}

fn table_cell(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "doc.table.cell";
    let doc = doc_id(cmd, p, "doc")?;
    let id = BlockId(uint(cmd, p, "id")?);
    let version = opt_version(cmd, p)?;
    let at = |key| uint(cmd, p, key).map(|n| usize::try_from(n).unwrap_or(usize::MAX));
    let (row, col) = (at("row")?, at("col")?);
    let words = text(cmd, p, "text")?.ok_or_else(|| bad(cmd, "text is required"))?;
    s.doc_mut()?.docs.set_cell(doc, version, id, row, col, &words).map_err(|e| refuse(cmd, e))?;
    Ok(json!({"ok": true}))
}

fn table_change(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "doc.table.change";
    let doc = doc_id(cmd, p, "doc")?;
    let id = BlockId(uint(cmd, p, "id")?);
    let version = opt_version(cmd, p)?;
    let name = text(cmd, p, "op")?.ok_or_else(|| bad(cmd, "op is required: insertRow, insertColumn, deleteRow, deleteColumn or header"))?;
    let at = opt_uint(cmd, p, "at")?.map(|n| usize::try_from(n).unwrap_or(usize::MAX));
    if at.is_none() && name.starts_with("delete") {
        return Err(bad(cmd, "at is required to delete a row or a column"));
    }
    let at = at.unwrap_or(0);
    let on = match p.get("on") {
        None | Some(Value::Null) => true,
        Some(Value::Bool(b)) => *b,
        Some(_) => return Err(bad(cmd, "on must be true or false")),
    };
    let op = TableOp::from_name(&name, at, on).ok_or_else(|| bad(cmd, format!("unknown op {name:?}")))?;
    s.doc_mut()?.docs.change_table(doc, version, id, op).map_err(|e| refuse(cmd, e))?;
    Ok(json!({"ok": true}))
}

/// Put a picture in a document: a new image block, or the picture of image block `id`. The bytes go
/// with the file's images; a refused block leaves none behind.
fn block_image(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "doc.block.image";
    let doc = doc_id(cmd, p, "doc")?;
    let version = opt_version(cmd, p)?;
    let alt = text(cmd, p, "alt")?;
    let target = opt_uint(cmd, p, "id")?.map(BlockId);
    let place = Place { parent: opt_uint(cmd, p, "parent")?.map(BlockId), index: opt_index(cmd, p)? };
    let bytes = super::create::image_bytes(cmd, p)?;
    let file = s.doc_mut()?;
    let known = file.images.contains_key(&figcraft_format::image_hash(&bytes));
    let (hash, w, h) = super::create::add_readable_image(file, bytes).map_err(|e| bad(cmd, e))?;
    let image = hash.clone();
    let placed = match target {
        Some(id) => file
            .docs
            .update_block(doc, version, id, |b| match &mut b.kind {
                BlockKind::Image { image: shown, alt: said } => {
                    *shown = image;
                    if let Some(a) = alt {
                        *said = a;
                    }
                    Ok(())
                }
                _ => Err(DocError::Invalid("that block is not an image")),
            })
            .map(|()| id),
        None => file.docs.insert_block(doc, version, place, BlockKind::Image { image, alt: alt.unwrap_or_default() }, Vec::new()),
    };
    match placed {
        Ok(id) => Ok(json!({"id": id.0, "image": hash, "width": w, "height": h})),
        Err(e) => {
            if !known {
                file.images.remove(&hash);
            }
            Err(refuse(cmd, e))
        }
    }
}

/// The longest side, in pixels, of a pinned picture of a layer.
const SNAPSHOT_SIDE: f64 = 2048.0;

/// A picture of layer `node` as it is now (at twice its size, at most [`SNAPSHOT_SIDE`] pixels
/// long), added to the file's images: its key, and whether the file had it already.
pub(crate) fn snapshot(file: &mut figcraft_doc::Document, node: NodeId) -> std::result::Result<(String, bool), String> {
    let b = figcraft_render::export_bounds(file, node).ok_or("the layer has nothing to draw")?;
    let side = b.width().max(b.height());
    if !(side.is_finite() && side > 0.0) {
        return Err("the layer has nothing to draw".into());
    }
    let scale = (SNAPSHOT_SIDE / side).min(2.0);
    let pix = figcraft_render::render_node(file, node, scale).ok_or("the layer could not be drawn")?;
    let png = figcraft_render::encode_png(&pix).map_err(|e| e.to_string())?;
    let known = file.images.contains_key(&figcraft_format::image_hash(&png));
    let (key, _, _) = figcraft_format::add_image(file, png).map_err(|e| e.to_string())?;
    Ok((key, known))
}

/// Pin every live layer shown in the last version of `doc` (a closing version keeps what it showed).
/// A layer that is gone or draws nothing stays as it is.
pub(crate) fn pin_live_embeds(file: &mut figcraft_doc::Document, doc: DocId) {
    fn live(blocks: &[Block], out: &mut Vec<(BlockId, NodeId)>) {
        for b in blocks {
            if let BlockKind::Embed { node, live: true, .. } = b.kind {
                out.push((b.id, node));
            }
            live(&b.children, out);
        }
    }
    let mut found = Vec::new();
    if let Some(v) = file.docs.get(doc).and_then(|d| d.current()) {
        live(&v.blocks, &mut found);
    }
    for (id, node) in found {
        let Ok((key, known)) = snapshot(file, node) else { continue };
        let pinned = file.docs.update_block(doc, None, id, |b| {
            b.kind = BlockKind::Embed { node, live: false, snapshot: Some(key.clone()) };
            Ok(())
        });
        if pinned.is_err() && !known {
            file.images.remove(&key);
        }
    }
}

/// Show a layer of the file in a document: a new embed block, or block `id` changed (its layer, or
/// live or pinned).
fn block_embed(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "doc.block.embed";
    let doc = doc_id(cmd, p, "doc")?;
    let version = opt_version(cmd, p)?;
    let target = opt_uint(cmd, p, "id")?.map(BlockId);
    let asked = opt_uint(cmd, p, "node")?.map(NodeId);
    let live = match p.get("live") {
        None | Some(Value::Null) => None,
        Some(Value::Bool(b)) => Some(*b),
        Some(_) => return Err(bad(cmd, "live must be true or false")),
    };
    let place = Place { parent: opt_uint(cmd, p, "parent")?.map(BlockId), index: opt_index(cmd, p)? };
    let file = s.doc_mut()?;
    // What block `id` shows now.
    let had = match target {
        Some(id) => {
            let d = file.docs.get(doc).ok_or_else(|| refuse(cmd, DocError::NoDoc(doc)))?;
            let v = match version {
                Some(n) => d.version(n),
                None => d.current(),
            };
            match v.and_then(|v| v.block(id)).map(|b| &b.kind) {
                Some(BlockKind::Embed { node, live, .. }) => Some((*node, *live)),
                Some(_) => return Err(bad(cmd, "that block is not an embed")),
                None => return Err(refuse(cmd, DocError::NoBlock(id))),
            }
        }
        None => None,
    };
    let node = asked.or(had.map(|h| h.0)).ok_or_else(|| bad(cmd, "node is required: the layer to show"))?;
    match file.node(node) {
        Some(n) if n.parent.is_some() && !matches!(n.kind, figcraft_doc::NodeKind::Page { .. }) => {}
        Some(_) => return Err(bad(cmd, "pick a layer on a page, not a page")),
        None => return Err(crate::EngineError::NoNode(node)),
    }
    let live = live.or(had.map(|h| h.1)).unwrap_or(true);
    let pinned = if live { None } else { Some(snapshot(file, node).map_err(|e| bad(cmd, e))?) };
    let shot = pinned.as_ref().map(|(k, _)| k.clone());
    let kind = BlockKind::Embed { node, live, snapshot: shot.clone() };
    let placed = match target {
        Some(id) => file
            .docs
            .update_block(doc, version, id, |b| {
                b.kind = kind;
                Ok(())
            })
            .map(|()| id),
        None => file.docs.insert_block(doc, version, place, kind, Vec::new()),
    };
    match placed {
        Ok(id) => Ok(json!({"id": id.0, "snapshot": shot})),
        Err(e) => {
            if let Some((key, false)) = pinned {
                file.images.remove(&key);
            }
            Err(refuse(cmd, e))
        }
    }
}

fn move_block(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "doc.block.move";
    let doc = doc_id(cmd, p, "doc")?;
    let id = BlockId(uint(cmd, p, "id")?);
    let version = opt_version(cmd, p)?;
    let place = Place { parent: opt_uint(cmd, p, "parent")?.map(BlockId), index: opt_index(cmd, p)? };
    s.doc_mut()?.docs.move_block(doc, version, id, place).map_err(|e| refuse(cmd, e))?;
    Ok(json!({"ok": true}))
}

fn delete_block(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "doc.block.delete";
    let doc = doc_id(cmd, p, "doc")?;
    let id = BlockId(uint(cmd, p, "id")?);
    let version = opt_version(cmd, p)?;
    s.doc_mut()?.docs.delete_block(doc, version, id).map_err(|e| refuse(cmd, e))?;
    Ok(json!({"ok": true}))
}
