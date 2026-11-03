//! Per-document, self-contained sound assets. Decoding is shared by import and both players.
use std::{
    collections::BTreeMap,
    io::{Cursor, ErrorKind},
    sync::Arc,
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use symphonia::core::{
    audio::SampleBuffer, codecs::DecoderOptions, errors::Error, formats::FormatOptions, io::MediaSourceStream, meta::MetadataOptions, probe::Hint,
};

pub const MAX_SOUND_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_LIBRARY_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_SOUNDS: usize = 64;
pub const MAX_SAMPLES: usize = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AudioFormat {
    Wav,
    Ogg,
    Mp3,
}
impl AudioFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Wav => "wav",
            Self::Ogg => "ogg",
            Self::Mp3 => "mp3",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Sound {
    pub name: String,
    /// Original file contents, not a path: portable with the saved document.
    pub data: Arc<str>,
    pub format: AudioFormat,
    #[serde(default = "one")]
    pub volume: f32,
    #[serde(default = "sfx")]
    pub group: String,
}
fn one() -> f32 {
    1.0
}
fn sfx() -> String {
    "SFX".into()
}
fn default_groups() -> BTreeMap<String, f32> {
    ["UI", "SFX", "music"].into_iter().map(|name| (name.into(), 1.0)).collect()
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AudioLibrary {
    pub sounds: BTreeMap<String, Sound>,
    pub master_volume: f32,
    pub groups: BTreeMap<String, f32>,
}
impl Default for AudioLibrary {
    fn default() -> Self {
        Self { sounds: BTreeMap::new(), master_volume: 1.0, groups: default_groups() }
    }
}
impl AudioLibrary {
    pub fn effective_volume(&self, id: &str, action: f32) -> Result<f32, String> {
        let sound = self.sounds.get(id).ok_or_else(|| format!("Sound `{id}` is no longer in this file"))?;
        let values = [self.master_volume, sound.volume, self.groups.get(&sound.group).copied().unwrap_or(1.0), action];
        if values.iter().any(|v| !v.is_finite() || !(0.0..=1.0).contains(v)) {
            return Err("Audio volumes must be finite numbers from 0 to 1".into());
        }
        Ok(values.into_iter().product::<f32>().clamp(0.0, 1.0))
    }
}

pub struct DecodedSound {
    pub samples: Vec<f32>,
    pub channels: u16,
    pub sample_rate: u32,
}

pub fn decode_base64(data: &str) -> Result<Vec<u8>, String> {
    if data.len() > MAX_SOUND_BYTES.div_ceil(3) * 4 {
        return Err("Sound exceeds the 16 MiB import limit".into());
    }
    let bytes = STANDARD.decode(data).map_err(|_| "Sound data is not valid base64".to_string())?;
    if bytes.len() > MAX_SOUND_BYTES {
        return Err("Sound exceeds the 16 MiB import limit".into());
    }
    Ok(bytes)
}

pub fn identify(bytes: &[u8]) -> Result<AudioFormat, String> {
    if bytes.is_empty() || bytes.len() > MAX_SOUND_BYTES {
        return Err("Sound must contain 1 byte to 16 MiB".into());
    }
    if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WAVE") {
        validate_wav(bytes)?;
        Ok(AudioFormat::Wav)
    } else if bytes.starts_with(b"OggS") {
        validate_ogg(bytes)?;
        Ok(AudioFormat::Ogg)
    } else if bytes.starts_with(b"ID3") || bytes.get(..2).is_some_and(|h| h[0] == 0xff && h[1] & 0xe0 == 0xe0) {
        validate_mp3(bytes)?;
        Ok(AudioFormat::Mp3)
    } else {
        Err("Unsupported sound: import a WAV, OGG Vorbis, or MP3 file".into())
    }
}

/// Reject structural truncation before the decoder, whose EOF is otherwise indistinguishable
/// from a short final packet. Every sample is decoded on import, not just the format header.
pub fn decode(bytes: Vec<u8>) -> Result<(AudioFormat, DecodedSound), String> {
    let kind = identify(&bytes)?;
    let mut hint = Hint::new();
    hint.with_extension(kind.extension());
    let source = MediaSourceStream::new(Box::new(Cursor::new(bytes)), Default::default());
    let probed = symphonia::default::get_probe()
        .format(&hint, source, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|e| format!("Cannot read sound: {e}"))?;
    let mut reader = probed.format;
    let track = reader.default_track().ok_or("Sound contains no audio track")?;
    let id = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions { verify: true })
        .map_err(|e| format!("Unsupported audio codec: {e}"))?;
    let mut samples = Vec::new();
    let mut channels = 0;
    let mut sample_rate = 0;
    loop {
        let packet = match reader.next_packet() {
            Ok(packet) => packet,
            Err(Error::IoError(e)) if e.kind() == ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(format!("Damaged sound packet: {e}")),
        };
        if packet.track_id() != id {
            continue;
        }
        let decoded = decoder.decode(&packet).map_err(|e| format!("Damaged audio data: {e}"))?;
        let spec = *decoded.spec();
        let count = spec.channels.count();
        if !(1..=2).contains(&count) || !(8000..=192000).contains(&spec.rate) {
            return Err("Sounds must be mono/stereo at 8–192 kHz".into());
        }
        if channels != 0 && (channels != count as u16 || sample_rate != spec.rate) {
            return Err("Audio format changes within this sound".into());
        }
        channels = count as u16;
        sample_rate = spec.rate;
        if decoded.capacity().saturating_mul(count) > MAX_SAMPLES {
            return Err("Audio packet exceeds the decoded sample limit".into());
        }
        let mut buffer = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
        buffer.copy_interleaved_ref(decoded);
        if samples.len().saturating_add(buffer.len()) > MAX_SAMPLES {
            return Err("Decoded sound exceeds the 16 million sample limit".into());
        }
        if buffer.samples().iter().any(|v| !v.is_finite()) {
            return Err("Sound contains non-finite samples".into());
        }
        samples.extend_from_slice(buffer.samples());
    }
    if samples.is_empty() {
        return Err("Sound contains no playable samples".into());
    }
    if decoder.finalize().verify_ok == Some(false) {
        return Err("Sound checksum verification failed".into());
    }
    Ok((kind, DecodedSound { samples, channels, sample_rate }))
}

fn u32_le(b: &[u8]) -> usize {
    u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize
}
fn validate_wav(b: &[u8]) -> Result<(), String> {
    if b.len() < 44 || u32_le(&b[4..8]).checked_add(8) != Some(b.len()) {
        return Err("Truncated or invalid WAV length".into());
    }
    let mut at = 12;
    let mut format = false;
    let mut data = false;
    while at < b.len() {
        if at + 8 > b.len() {
            return Err("Truncated WAV chunk".into());
        }
        let size = u32_le(&b[at + 4..at + 8]);
        let end = at.checked_add(8).and_then(|v| v.checked_add(size)).ok_or("Invalid WAV chunk size")?;
        if end > b.len() {
            return Err("Truncated WAV data".into());
        }
        if &b[at..at + 4] == b"fmt " {
            if size < 16 {
                return Err("Invalid WAV format".into());
            }
            format = true;
        }
        if &b[at..at + 4] == b"data" {
            data |= size > 0;
        }
        at = end + (size & 1);
    }
    if at != b.len() || !format || !data {
        return Err("WAV is missing format or sample data".into());
    }
    Ok(())
}
fn validate_ogg(b: &[u8]) -> Result<(), String> {
    let mut at = 0;
    let mut eos = false;
    while at < b.len() {
        if at + 27 > b.len() || &b[at..at + 4] != b"OggS" || b[at + 4] != 0 {
            return Err("Invalid or truncated OGG page".into());
        }
        let segments = b[at + 26] as usize;
        if at + 27 + segments > b.len() {
            return Err("Truncated OGG segment table".into());
        }
        let payload: usize = b[at + 27..at + 27 + segments].iter().map(|v| *v as usize).sum();
        let end = at + 27 + segments + payload;
        if end > b.len() {
            return Err("Truncated OGG payload".into());
        }
        let mut crc = 0u32;
        for (i, byte) in b[at..end].iter().enumerate() {
            crc ^= u32::from(if (22..26).contains(&i) { 0 } else { *byte }) << 24;
            for _ in 0..8 {
                crc = (crc << 1) ^ if crc & 0x8000_0000 != 0 { 0x04c1_1db7 } else { 0 };
            }
        }
        if crc as usize != u32_le(&b[at + 22..at + 26]) {
            return Err("OGG page checksum mismatch".into());
        }
        eos = b[at + 5] & 4 != 0;
        at = end;
    }
    if !eos {
        return Err("OGG stream has no end-of-stream page".into());
    }
    Ok(())
}
fn validate_mp3(b: &[u8]) -> Result<(), String> {
    let mut at = 0;
    if b.starts_with(b"ID3") {
        if b.len() < 10 || !(2..=4).contains(&b[3]) || b[6..10].iter().any(|x| x & 0x80 != 0) {
            return Err("Invalid MP3 ID3 header".into());
        }
        let size = b[6..10].iter().fold(0usize, |n, v| (n << 7) | *v as usize);
        at = 10 + size + if b[3] == 4 && b[5] & 0x10 != 0 { 10 } else { 0 };
    }
    let end = if b.len() >= 128 && &b[b.len() - 128..b.len() - 125] == b"TAG" { b.len() - 128 } else { b.len() };
    let mut frames = 0;
    while at < end {
        let header = b.get(at..at + 4).filter(|_| at + 4 <= end).ok_or("Truncated MP3 frame header")?;
        let size = mp3_frame_size(header)?;
        if at + size > end {
            return Err("Truncated MP3 frame".into());
        }
        at += size;
        frames += 1;
    }
    if frames == 0 || at != end {
        return Err("MP3 contains no complete audio frames".into());
    }
    Ok(())
}
fn mp3_frame_size(header: &[u8]) -> Result<usize, String> {
    if header.len() != 4 || header[0] != 0xff || header[1] & 0xe0 != 0xe0 {
        return Err("Invalid MP3 frame signature".into());
    }
    let version = (header[1] >> 3) & 3;
    let layer = (header[1] >> 1) & 3;
    let rate_index = (header[2] >> 2) & 3;
    let bitrate_index = (header[2] >> 4) as usize;
    if version == 1 || layer != 1 || rate_index == 3 || bitrate_index == 0 || bitrate_index == 15 {
        return Err("Unsupported or invalid MP3 frame header".into());
    }
    let mpeg1 = version == 3;
    let rates = [44100usize, 48000, 32000];
    let divisor = match version {
        3 => 1,
        2 => 2,
        _ => 4,
    };
    let rate = rates[rate_index as usize] / divisor;
    let bitrates = if mpeg1 {
        [0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320]
    } else {
        [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160]
    };
    Ok((if mpeg1 { 144 } else { 72 }) * bitrates[bitrate_index] * 1000 / rate + ((header[2] >> 1) & 1) as usize)
}

/// Deterministic content key; no product assets or external URLs enter a library.
pub fn content_id(bytes: &[u8]) -> String {
    let hash = bytes.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ u64::from(*b)).wrapping_mul(0x100_0000_01b3));
    format!("sound-{hash:016x}")
}
