//! Versions of a document (G13.4): closing, a new version, signers and signatures.
//!
//! Closing freezes a version: its blocks, whether it is "for signature" and its signers are covered by
//! a fingerprint (SHA-256 of [`canonical`], computed by the engine) and can never change again. A
//! change is a new version, a copy of the last one to edit, whose blocks keep their ids so the two can
//! be compared block by block.
//!
//! A signature is given on a closed version, in a signer's slot: a drawn mark (strokes) or a typed name
//! in a handwriting style. It records who, when and the fingerprint it agreed to. The data has room
//! for signing through a link later (G15): an email, the server's time, how it was given.

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::docs::{DocError, DocId, Documents, MAX_VERSIONS, VersionState};

/// Most signers (slots) of one version.
pub const MAX_SIGNERS: usize = 20;
/// Longest name, role or email of a signer, in characters.
pub const MAX_SIGNER_TEXT: usize = 120;
/// Most strokes of a drawn signature.
pub const MAX_STROKES: usize = 200;
/// Most points of a drawn signature, all strokes together.
pub const MAX_POINTS: usize = 20_000;
/// The handwriting styles a typed signature can use.
pub const HAND_STYLES: u8 = 3;

/// Someone who signs a version, in a slot of its signature section.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Signer {
    pub name: String,
    /// What they sign as ("Designer", "Client", "Producer"…).
    pub role: String,
    /// For signing through a link (G15); empty here.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub email: String,
}

/// The mark of a signature.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Mark {
    /// The name typed, drawn in a handwriting style (0, 1 or 2; the font is picked when drawing).
    Typed {
        #[serde(default)]
        text: String,
        #[serde(default)]
        style: u8,
    },
    /// Strokes drawn with a brush: points in a 3:1 box, x and y from 0 to 1.
    Drawn {
        #[serde(default)]
        strokes: Vec<Vec<[f32; 2]>>,
    },
}

impl Default for Mark {
    fn default() -> Self {
        Self::Typed { text: String::new(), style: 0 }
    }
}

/// A signature given on a closed version.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Signature {
    /// The slot it fills (an index into the version's signers).
    pub slot: usize,
    pub name: String,
    pub role: String,
    pub mark: Mark,
    /// When it was given, seconds since 1970 (this computer's clock).
    pub signed_at: u64,
    /// The fingerprint of the version it agrees to.
    pub fingerprint: String,
    /// How it was given: "graybox" (here) or, later, "link" (G15).
    pub via: String,
    /// The server's time, for a signature given through a link (G15).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_time: Option<u64>,
}

fn clean(s: &str, max: usize) -> String {
    s.chars().filter(|c| !c.is_control()).take(max).collect::<String>().trim().to_string()
}

impl Signer {
    fn sanitize(&mut self) {
        self.name = clean(&self.name, MAX_SIGNER_TEXT);
        self.role = clean(&self.role, MAX_SIGNER_TEXT);
        self.email = clean(&self.email, MAX_SIGNER_TEXT);
    }
}

impl Mark {
    /// Whether there is something to see: a typed name, or a drawn stroke.
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Typed { text, .. } => text.trim().is_empty(),
            Self::Drawn { strokes } => strokes.iter().all(|s| s.is_empty()),
        }
    }

    /// Keep it within the caps: a known style, points finite and inside the box.
    pub fn sanitize(&mut self) {
        match self {
            Self::Typed { text, style } => {
                *text = clean(text, MAX_SIGNER_TEXT);
                *style = (*style).min(HAND_STYLES - 1);
            }
            Self::Drawn { strokes } => {
                strokes.truncate(MAX_STROKES);
                let mut budget = MAX_POINTS;
                for s in strokes.iter_mut() {
                    s.retain(|p| p[0].is_finite() && p[1].is_finite());
                    s.truncate(budget);
                    budget -= s.len();
                    for p in s.iter_mut() {
                        p[0] = p[0].clamp(0.0, 1.0);
                        p[1] = p[1].clamp(0.0, 1.0);
                    }
                }
                strokes.retain(|s| !s.is_empty());
            }
        }
    }
}

impl Signature {
    fn sanitize(&mut self) {
        self.name = clean(&self.name, MAX_SIGNER_TEXT);
        self.role = clean(&self.role, MAX_SIGNER_TEXT);
        self.fingerprint = clean(&self.fingerprint, 128);
        self.via = clean(&self.via, 16);
        self.mark.sanitize();
    }
}

/// What a version's fingerprint covers, as text: its blocks, whether it is for signature and its
/// signers. Signatures, the state and the times are left out (they are what closing and signing add).
///
/// Its shape must never change: a field added to blocks or signers is skipped when it has its default
/// value, or every version closed by an older Graybox would read as changed outside Graybox (a test
/// pins it).
pub fn canonical(v: &crate::docs::DocVersion) -> String {
    json!({"blocks": v.blocks, "forSignature": v.for_signature, "signers": v.signers}).to_string()
}

/// The signers a version starts with when it is marked for signature: the designer and the client.
pub fn default_signers() -> Vec<Signer> {
    vec![Signer { role: "Designer".into(), ..Signer::default() }, Signer { role: "Client".into(), ..Signer::default() }]
}

/// Repair what a file says about a version's signing (loading).
pub(crate) fn repair(v: &mut crate::docs::DocVersion) {
    v.signers.truncate(MAX_SIGNERS);
    v.signers.iter_mut().for_each(Signer::sanitize);
    v.signatures.truncate(MAX_SIGNERS);
    v.signatures.iter_mut().for_each(Signature::sanitize);
    // One signature per slot, in a slot that exists, on a closed version only.
    let slots = v.signers.len().max(1);
    let mut seen = std::collections::BTreeSet::new();
    v.signatures.retain(|s| s.slot < slots && seen.insert(s.slot));
    if v.state != VersionState::Closed {
        v.signatures.clear();
    }
}

impl Documents {
    /// Close the last version of `doc` with its `fingerprint` (see [`canonical`]) at `closed_at`;
    /// returns its number. Refused when it is closed already.
    pub fn close_version(&mut self, doc: DocId, fingerprint: &str, closed_at: &str) -> Result<u32, DocError> {
        let v = self.draft_mut(doc, None)?;
        v.state = VersionState::Closed;
        v.fingerprint = Some(clean(fingerprint, 128));
        v.closed_at = Some(clean(closed_at, 64));
        Ok(v.number)
    }

    /// Start a new version of `doc`: a draft copy of the last one (closed), whose blocks keep their ids.
    pub fn new_version(&mut self, doc: DocId) -> Result<u32, DocError> {
        let d = self.doc_mut(doc)?;
        let last = d.versions.last().ok_or(DocError::NoVersion(0))?;
        if !last.is_closed() {
            return Err(DocError::Invalid("the last version is still a draft: close it before making a new one"));
        }
        if d.versions.len() >= MAX_VERSIONS {
            return Err(DocError::Limit("versions"));
        }
        let number = last.number.saturating_add(1);
        let mut v = crate::docs::DocVersion::draft(number);
        v.blocks = last.blocks.clone();
        v.for_signature = last.for_signature;
        v.signers = last.signers.clone();
        d.versions.push(v);
        Ok(number)
    }

    /// Mark the draft `version` (the last by default) for signature or not. Turning it on gives it
    /// the default signers when it has none.
    pub fn set_for_signature(&mut self, doc: DocId, version: Option<u32>, on: bool) -> Result<(), DocError> {
        let v = self.draft_mut(doc, version)?;
        v.for_signature = on;
        if on && v.signers.is_empty() {
            v.signers = default_signers();
        }
        Ok(())
    }

    /// Set the signers of the draft `version` (the last by default).
    pub fn set_signers(&mut self, doc: DocId, version: Option<u32>, signers: Vec<Signer>) -> Result<(), DocError> {
        if signers.len() > MAX_SIGNERS {
            return Err(DocError::Limit("signers"));
        }
        let v = self.draft_mut(doc, version)?;
        v.signers = signers;
        v.signers.iter_mut().for_each(Signer::sanitize);
        Ok(())
    }

    /// Sign the closed `version` (the last by default) in `sig.slot`. Refused on a draft, in a slot
    /// that does not exist or is signed already, or with an empty mark.
    pub fn sign(&mut self, doc: DocId, version: Option<u32>, mut sig: Signature) -> Result<(), DocError> {
        sig.sanitize();
        if sig.mark.is_empty() {
            return Err(DocError::Invalid("a signature needs a drawn mark or a typed name"));
        }
        if sig.name.is_empty() {
            return Err(DocError::Invalid("a signature needs the signer's name"));
        }
        let d = self.doc_mut(doc)?;
        let v = match version {
            Some(n) => d.versions.iter_mut().find(|v| v.number == n).ok_or(DocError::NoVersion(n))?,
            None => d.versions.last_mut().ok_or(DocError::NoVersion(0))?,
        };
        if !v.is_closed() {
            return Err(DocError::Invalid("only a closed version can be signed: close it first"));
        }
        if sig.slot >= v.signers.len().max(1) {
            return Err(DocError::Invalid("that signature slot does not exist"));
        }
        if v.signatures.iter().any(|s| s.slot == sig.slot) {
            return Err(DocError::Invalid("that slot is signed already"));
        }
        sig.fingerprint = v.fingerprint.clone().unwrap_or_default();
        v.signatures.push(sig);
        v.signatures.sort_by_key(|s| s.slot);
        Ok(())
    }
}

/// The short code of a fingerprint (hex SHA-256), as on the document and every PDF footer:
/// `GBX-XXXX-XXXX`, the first 40 bits in Crockford base 32 (no I, L, O or U to misread).
pub fn short_code(fingerprint: &str) -> Option<String> {
    const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
    let hex = fingerprint.get(..10)?;
    let bits = u64::from_str_radix(hex, 16).ok()?;
    let chars: String = (0..8).rev().filter_map(|i| ALPHABET.get(((bits >> (i * 5)) & 31) as usize).map(|c| *c as char)).collect();
    Some(format!("GBX-{}-{}", chars.get(..4)?, chars.get(4..)?))
}
