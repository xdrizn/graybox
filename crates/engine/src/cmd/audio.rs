//! Undoable per-file sound library commands, shared by UI, CLI and control clients.
use super::CommandSpec;
use crate::{
    Result, Session, bad,
    params::{get, req_string, string},
    spec,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use figcraft_doc::audio::{self, AudioFormat, MAX_LIBRARY_BYTES, MAX_SOUNDS, Sound};
use serde_json::{Value, json};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        spec!(
            "audio.import",
            "Import sound",
            "{path | data (base64), name?, mime?, group? (default SFX), volume? (0..1)} → {sound, name, format}; WAV, OGG Vorbis or MP3; max 16 MiB/file, 64 sounds, 64 MiB/library",
            true,
            import
        ),
        spec!(
            "audio.remove",
            "Remove sound",
            "{sound}; removes the asset; existing Play sound actions report a missing sound instead of crashing",
            true,
            remove
        ),
        spec!(
            "audio.volume",
            "Sound volume",
            "{sound? | group?, volume (0..1)}; no selector sets master volume; playback multiplies master × group × sound × action",
            true,
            volume
        ),
    ]
}
fn level(cmd: &str, p: &Value, default: Option<f32>) -> Result<f32> {
    let value = match get(p, "volume") {
        None => return default.ok_or_else(|| bad(cmd, "volume is required")),
        Some(value) => value.as_f64().ok_or_else(|| bad(cmd, "volume must be a number from 0 to 1"))?,
    };
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(bad(cmd, "volume must be a finite number from 0 to 1"));
    }
    Ok(value as f32)
}
fn label(cmd: &str, value: String, field: &str) -> Result<String> {
    if value.trim().is_empty() || value.len() > 120 || value.chars().any(char::is_control) {
        return Err(bad(cmd, format!("{field} must contain 1–120 printable bytes")));
    }
    Ok(value)
}
fn import(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "audio.import";
    if get(p, "path").is_some() && get(p, "data").is_some() {
        return Err(bad(cmd, "pass either path or data, not both"));
    }
    let data = get(p, "data").and_then(Value::as_str);
    let data_mime = data.and_then(|data| data.strip_prefix("data:")).and_then(|url| url.split_once(',')).map(|(header, _)| header);
    let bytes = if let Some(data) = data {
        let raw = if data.starts_with("data:") { data.split_once(',').ok_or_else(|| bad(cmd, "invalid data URL"))?.1 } else { data };
        audio::decode_base64(raw).map_err(|e| bad(cmd, e))?
    } else if let Some(path) = string(p, "path") {
        #[cfg(not(target_arch = "wasm32"))]
        {
            crate::file_access::read_capped(&path, audio::MAX_SOUND_BYTES as u64)?
        }
        #[cfg(target_arch = "wasm32")]
        {
            return Err(bad(cmd, format!("{path}: browser imports require picked file data")));
        }
    } else {
        return Err(bad(cmd, "pass data (base64) or path"));
    };
    let kind = audio::identify(&bytes).map_err(|e| bad(cmd, e))?;
    if let Some(mime) = get(p, "mime").and_then(Value::as_str).or(data_mime) {
        let mime = mime.split(';').next().unwrap_or_default().trim().to_ascii_lowercase();
        let expected = match kind {
            AudioFormat::Wav => matches!(mime.as_str(), "audio/wav" | "audio/wave" | "audio/x-wav" | "audio/vnd.wave"),
            AudioFormat::Ogg => matches!(mime.as_str(), "audio/ogg" | "application/ogg"),
            AudioFormat::Mp3 => matches!(mime.as_str(), "audio/mpeg" | "audio/mp3"),
        };
        if !expected && mime != "application/octet-stream" && !mime.is_empty() {
            return Err(bad(cmd, "MIME type does not match the WAV/OGG/MP3 contents"));
        }
    }
    let name = string(p, "name")
        .or_else(|| string(p, "path").and_then(|path| std::path::Path::new(&path).file_name().map(|s| s.to_string_lossy().into_owned())))
        .unwrap_or_else(|| format!("Sound.{}", kind.extension()));
    if let Some(ext) = std::path::Path::new(&name).extension().and_then(|e| e.to_str())
        && !["wav", "ogg", "mp3"].iter().any(|known| ext.eq_ignore_ascii_case(known))
    {
        return Err(bad(cmd, "unsupported sound extension; use WAV, OGG or MP3"));
    }
    let name = label(cmd, name, "name")?;
    let group = label(cmd, string(p, "group").unwrap_or_else(|| "SFX".into()), "group")?;
    let volume = level(cmd, p, Some(1.0))?;
    let id = audio::content_id(&bytes);
    let library = &s.doc()?.audio;
    if library.sounds.len() >= MAX_SOUNDS && !library.sounds.contains_key(&id) {
        return Err(bad(cmd, "A file can contain at most 64 sounds"));
    }
    if library.groups.len() >= 64 && !library.groups.contains_key(&group) {
        return Err(bad(cmd, "At most 64 sound groups are supported"));
    }
    let encoded = STANDARD.encode(&bytes);
    let total = library
        .sounds
        .iter()
        .filter(|(key, _)| *key != &id)
        .try_fold(bytes.len(), |total, (_, sound)| {
            let padding = sound.data.as_bytes().iter().rev().take_while(|byte| **byte == b'=').take(2).count();
            let size = sound.data.len().saturating_mul(3).div_ceil(4).saturating_sub(padding);
            total.checked_add(size)
        })
        .ok_or_else(|| bad(cmd, "Sound library exceeds 64 MiB"))?;
    if total > MAX_LIBRARY_BYTES {
        return Err(bad(cmd, "Sound library exceeds 64 MiB"));
    }
    audio::decode(bytes).map_err(|e| bad(cmd, e))?;
    let library = &mut s.doc_mut()?.audio;
    library.groups.entry(group.clone()).or_insert(1.0);
    library.sounds.insert(id.clone(), Sound { name: name.clone(), data: encoded.into(), format: kind, volume, group });
    Ok(json!({"sound": id, "name": name, "format": kind}))
}
fn remove(s: &mut Session, p: &Value) -> Result<Value> {
    let id = req_string("audio.remove", p, "sound")?;
    if s.doc_mut()?.audio.sounds.remove(&id).is_none() {
        return Err(bad("audio.remove", format!("No sound `{id}`")));
    }
    Ok(json!({"removed": id}))
}
fn volume(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "audio.volume";
    let level = level(cmd, p, None)?;
    let sound = string(p, "sound");
    let group = string(p, "group");
    if sound.is_some() && group.is_some() {
        return Err(bad(cmd, "choose sound or group, not both"));
    }
    // Resolve every fallible selector before taking the mutable document.
    if let Some(id) = &sound
        && !s.doc()?.audio.sounds.contains_key(id)
    {
        return Err(bad(cmd, format!("No sound `{id}`")));
    }
    let group = group.map(|g| label(cmd, g, "group")).transpose()?;
    let library = &mut s.doc_mut()?.audio;
    if let Some(id) = sound {
        if let Some(asset) = library.sounds.get_mut(&id) {
            asset.volume = level;
        }
    } else if let Some(group) = group {
        if library.groups.len() >= 64 && !library.groups.contains_key(&group) {
            return Err(bad(cmd, "At most 64 sound groups are supported"));
        }
        library.groups.insert(group, level);
    } else {
        library.master_volume = level;
    }
    Ok(json!({"volume": level}))
}
