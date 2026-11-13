//! Versions of a document (G13.4): close (fingerprint, read-only), a new version, "for signature",
//! signers, signing, the history, the diff between two versions and the decision log.

use figcraft_doc::docs::{Block, BlockKind, DocError, DocVersion};
use figcraft_doc::docs_version::{Mark, Signature, Signer};
use serde_json::{Value, json};

use super::{doc_id, edit, opt_uint, opt_version, refuse, text};
use crate::cmd::CommandSpec;
use crate::{Result, Session, bad, docs_page, docs_render, spec};

pub(super) fn specs() -> Vec<CommandSpec> {
    vec![
        spec!(
            "doc.version.close",
            "Close version",
            "{doc} closes the last version for good: its SHA-256 fingerprint is recorded and it can never change (a change is a new version) → {version, fingerprint, code, closedAt}",
            true,
            |s, p| edit(s, p, close)
        ),
        spec!("doc.version.new", "New version", "{doc} a draft copy of the last (closed) version to edit → {version}", true, |s, p| edit(
            s,
            p,
            new_version
        )),
        spec!(
            "doc.version.for_signature",
            "Mark document for signature",
            "{doc, version? (a draft; the last by default), on? (default true)} adds or removes the signature section at the end (the designer and the client by default)",
            true,
            |s, p| edit(s, p, for_signature)
        ),
        spec!(
            "doc.version.signers",
            "Set document signers",
            "{doc, version? (a draft; the last by default), signers: [{name?, role?}]} who signs, in slot order",
            true,
            |s, p| edit(s, p, signers)
        ),
        spec!(
            "doc.version.sign",
            "Sign version",
            "{doc, version? (a closed version; the last by default), slot? (default 0, the designer), name, role?, mark? ({type: typed, text?, style? 0-2} or {type: drawn, strokes: [[[x, y]…]…]} with x, y from 0 to 1 in a 3:1 box; default: the name typed)}",
            true,
            |s, p| edit(s, p, sign)
        ),
        spec!(
            "doc.versions",
            "Show document versions",
            "{doc} → [{number, state, closedAt?, code?, fingerprint?, intact, forSignature, signers, signatures}] (intact is false when a closed version was changed outside Graybox)",
            false,
            versions
        ),
        spec!(
            "doc.diff",
            "Compare document versions",
            "{doc, from? (default: the version before `to`), to? (default: the last)} → {from, to, added, removed, changed, blocks: [{change: same|added|removed|changed, id, kind, text, before?}]}",
            false,
            diff
        ),
        spec!(
            "doc.decisions",
            "Show decision log",
            "{} → every decision block in the file (last version of each document): [{doc, title, version, block, text, who, when, why, closed}]",
            false,
            decisions
        ),
    ]
}

fn now() -> u64 {
    web_time::SystemTime::now().duration_since(web_time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn close(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "doc.version.close";
    let id = doc_id(cmd, p, "doc")?;
    let docs = &mut s.doc_mut()?.docs;
    let last = docs.get(id).and_then(|d| d.current()).ok_or_else(|| refuse(cmd, DocError::NoDoc(id)))?;
    let hash = docs_render::fingerprint(last);
    let at = docs_page::when(now());
    let number = docs.close_version(id, &hash, &at).map_err(|e| refuse(cmd, e))?;
    let code = figcraft_doc::docs_version::short_code(&hash);
    Ok(json!({"version": number, "fingerprint": hash, "code": code, "closedAt": at}))
}

fn new_version(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "doc.version.new";
    let id = doc_id(cmd, p, "doc")?;
    let n = s.doc_mut()?.docs.new_version(id).map_err(|e| refuse(cmd, e))?;
    Ok(json!({"version": n}))
}

fn for_signature(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "doc.version.for_signature";
    let id = doc_id(cmd, p, "doc")?;
    let version = opt_version(cmd, p)?;
    let on = match p.get("on") {
        None | Some(Value::Null) => true,
        Some(Value::Bool(b)) => *b,
        Some(_) => return Err(bad(cmd, "on must be true or false")),
    };
    s.doc_mut()?.docs.set_for_signature(id, version, on).map_err(|e| refuse(cmd, e))?;
    Ok(json!({"forSignature": on}))
}

fn signers(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "doc.version.signers";
    let id = doc_id(cmd, p, "doc")?;
    let version = opt_version(cmd, p)?;
    let list: Vec<Signer> = match p.get("signers") {
        Some(v @ Value::Array(_)) => {
            serde_json::from_value(v.clone()).map_err(|e| bad(cmd, format!("signers must be a list of {{name?, role?}}: {e}")))?
        }
        _ => return Err(bad(cmd, "signers is required (a list of {name?, role?})")),
    };
    s.doc_mut()?.docs.set_signers(id, version, list).map_err(|e| refuse(cmd, e))?;
    Ok(json!({}))
}

fn sign(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "doc.version.sign";
    let id = doc_id(cmd, p, "doc")?;
    let version = opt_version(cmd, p)?;
    let slot = usize::try_from(opt_uint(cmd, p, "slot")?.unwrap_or(0)).unwrap_or(usize::MAX);
    let name = text(cmd, p, "name")?.ok_or_else(|| bad(cmd, "name is required"))?;
    let role = text(cmd, p, "role")?;
    let mark = match p.get("mark") {
        None | Some(Value::Null) => Mark::Typed { text: name.clone(), style: 0 },
        Some(v) => serde_json::from_value(v.clone())
            .map_err(|e| bad(cmd, format!("mark must be {{type: typed, text?, style?}} or {{type: drawn, strokes}}: {e}")))?,
    };
    // A typed mark with no text of its own writes the name.
    let mark = match mark {
        Mark::Typed { text, style } if text.trim().is_empty() => Mark::Typed { text: name.clone(), style },
        m => m,
    };
    let docs = &mut s.doc_mut()?.docs;
    let doc = docs.get(id).ok_or_else(|| refuse(cmd, DocError::NoDoc(id)))?;
    let v = match version {
        Some(n) => doc.version(n).ok_or_else(|| refuse(cmd, DocError::NoVersion(n)))?,
        None => doc.current().ok_or_else(|| refuse(cmd, DocError::NoVersion(0)))?,
    };
    if !docs_render::intact(v) {
        return Err(bad(cmd, "this version was changed outside Graybox: its fingerprint no longer matches, so it cannot be signed"));
    }
    let role = role.unwrap_or_else(|| v.signers.get(slot).map(|r| r.role.clone()).unwrap_or_default());
    let sig = Signature { slot, name, role, mark, signed_at: now(), via: "graybox".into(), ..Signature::default() };
    docs.sign(id, version, sig).map_err(|e| refuse(cmd, e))?;
    Ok(json!({}))
}

fn version_json(v: &DocVersion) -> Value {
    let signatures: Vec<Value> = v
        .signatures
        .iter()
        .map(|g| json!({"slot": g.slot, "name": g.name, "role": g.role, "signedAt": docs_page::when(g.signed_at), "via": g.via}))
        .collect();
    json!({
        "number": v.number,
        "state": if v.is_closed() { "closed" } else { "draft" },
        "closedAt": v.closed_at,
        "code": docs_render::code_of(v),
        "fingerprint": v.fingerprint,
        "intact": docs_render::intact(v),
        "forSignature": v.for_signature,
        "signers": v.signers,
        "signatures": signatures,
    })
}

fn versions(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "doc.versions";
    let id = doc_id(cmd, p, "doc")?;
    let doc = s.doc()?.docs.get(id).ok_or_else(|| refuse(cmd, DocError::NoDoc(id)))?;
    Ok(Value::Array(doc.versions.iter().map(version_json).collect()))
}

/// Every block of a version in reading order, with its id, kind, text and whole content.
fn flat(blocks: &[Block], out: &mut Vec<(u64, String, String, Value)>) {
    for b in blocks {
        let mut own = b.clone();
        own.children.clear();
        out.push((b.id.0, b.kind.name(), b.plain_text(), serde_json::to_value(&own).unwrap_or(Value::Null)));
        flat(&b.children, out);
    }
}

fn diff(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "doc.diff";
    let id = doc_id(cmd, p, "doc")?;
    let doc = s.doc()?.docs.get(id).ok_or_else(|| refuse(cmd, DocError::NoDoc(id)))?;
    let number = |key: &str| -> Result<Option<u32>> { Ok(opt_uint(cmd, p, key)?.map(|n| u32::try_from(n).unwrap_or(u32::MAX))) };
    let to_n = number("to")?.or_else(|| doc.current().map(|v| v.number)).unwrap_or(1);
    let from_n = number("from")?.unwrap_or(to_n.saturating_sub(1).max(1));
    let to = doc.version(to_n).ok_or_else(|| refuse(cmd, DocError::NoVersion(to_n)))?;
    let from = doc.version(from_n).ok_or_else(|| refuse(cmd, DocError::NoVersion(from_n)))?;
    let (mut a, mut b) = (Vec::new(), Vec::new());
    flat(&from.blocks, &mut a);
    flat(&to.blocks, &mut b);
    let d = compare(&a, &b);
    Ok(json!({"from": from_n, "to": to_n, "added": d.added, "removed": d.removed, "changed": d.changed, "blocks": d.blocks}))
}

/// A flattened block: id, kind, text, and its whole content without children.
type Flat = (u64, String, String, Value);

#[derive(Default)]
struct Compared {
    added: usize,
    removed: usize,
    changed: usize,
    blocks: Vec<Value>,
}

impl Compared {
    /// The blocks of `a` before `limit` that `b` no longer has, from `*next` on.
    fn removed_before(&mut self, a: &[Flat], b: &[Flat], next: &mut usize, limit: usize) {
        while *next < limit {
            if let Some((bid, kind, text, _)) = a.get(*next)
                && !b.iter().any(|x| x.0 == *bid)
            {
                self.blocks.push(json!({"change": "removed", "id": bid, "kind": kind, "text": text}));
                self.removed += 1;
            }
            *next += 1;
        }
    }
}

/// Version `b` against version `a`, block by block (a new version keeps its blocks' ids), in `b`'s
/// order with the removed blocks where they were.
fn compare(a: &[Flat], b: &[Flat]) -> Compared {
    let mut d = Compared::default();
    let mut next = 0usize;
    for (k, (bid, kind, text, whole)) in b.iter().enumerate() {
        match a.iter().position(|x| x.0 == *bid) {
            None => {
                // What was removed here comes first, as in a written diff: up to the next kept block.
                let kept = b.iter().skip(k + 1).find_map(|y| a.iter().position(|x| x.0 == y.0)).unwrap_or(a.len());
                d.removed_before(a, b, &mut next, kept);
                d.blocks.push(json!({"change": "added", "id": bid, "kind": kind, "text": text}));
                d.added += 1;
            }
            Some(i) => {
                d.removed_before(a, b, &mut next, i);
                next = next.max(i + 1);
                match a.get(i) {
                    Some((_, _, _, old)) if old == whole => d.blocks.push(json!({"change": "same", "id": bid, "kind": kind, "text": text})),
                    Some((_, old_kind, old_text, _)) => {
                        d.blocks
                            .push(json!({"change": "changed", "id": bid, "kind": kind, "text": text, "before": old_text, "kindBefore": old_kind}));
                        d.changed += 1;
                    }
                    None => {}
                }
            }
        }
    }
    d.removed_before(a, b, &mut next, a.len());
    d
}

fn decisions(s: &mut Session, _p: &Value) -> Result<Value> {
    let docs = &s.doc()?.docs;
    let mut out = Vec::new();
    for id in docs.walk() {
        let Some(doc) = docs.get(id) else { continue };
        let Some(v) = doc.current() else { continue };
        let mut stack: Vec<&Block> = v.blocks.iter().rev().collect();
        while let Some(b) = stack.pop() {
            if let BlockKind::Decision { who, when, why } = &b.kind {
                out.push(json!({"doc": id.0, "title": doc.title, "version": v.number, "block": b.id.0, "text": b.plain_text(),
                    "who": who, "when": when, "why": why, "closed": v.is_closed()}));
            }
            stack.extend(b.children.iter().rev());
        }
    }
    Ok(Value::Array(out))
}
