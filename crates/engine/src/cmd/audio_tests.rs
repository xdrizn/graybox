//! Real original fixtures: 880 Hz, 30 ms generated with ffmpeg's sine source; no product recordings.
use crate::Session;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use figcraft_doc::{
    Document,
    audio::{self, AudioFormat, AudioLibrary, MAX_SOUND_BYTES, Sound},
};
use serde_json::json;

fn wav() -> Vec<u8> {
    let samples: Vec<i16> = (0..240).map(|i| ((i as f64 * std::f64::consts::TAU * 880.0 / 8000.0).sin() * 4000.0) as i16).collect();
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + samples.len() as u32 * 2).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&8000u32.to_le_bytes());
    bytes.extend_from_slice(&16000u32.to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&(samples.len() as u32 * 2).to_le_bytes());
    for value in samples {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}
fn import(session: &mut Session, bytes: &[u8], name: &str) -> String {
    session.execute("audio.import", &json!({"data": STANDARD.encode(bytes), "name":name})).unwrap()["sound"].as_str().unwrap().to_string()
}
#[test]
fn real_wav_ogg_and_mp3_decode_and_import() {
    for (bytes, format, name) in [
        (wav(), AudioFormat::Wav, "Original.wav"),
        (include_bytes!("audio-test.ogg").to_vec(), AudioFormat::Ogg, "Original.ogg"),
        (include_bytes!("audio-test.mp3").to_vec(), AudioFormat::Mp3, "Original.mp3"),
    ] {
        let (kind, decoded) = audio::decode(bytes.clone()).unwrap();
        assert_eq!(kind, format);
        assert!(!decoded.samples.is_empty());
        assert!(decoded.samples.iter().any(|value| value.abs() > 0.001));
        let mut session = Session::new();
        let id = import(&mut session, &bytes, name);
        assert_eq!(session.doc().unwrap().audio.sounds[&id].format, format);
        assert_eq!(audio::decode_base64(&session.doc().unwrap().audio.sounds[&id].data).unwrap(), bytes);
    }
}
#[test]
fn damaged_truncated_and_unsupported_data_are_not_assets() {
    let mut session = Session::new();
    for bytes in [b"not a sound".to_vec(), b"RIFF1234WAVE".to_vec(), b"OggS".to_vec(), b"ID3".to_vec(), b"fLaC12345678".to_vec()] {
        assert!(session.execute("audio.import", &json!({"data": STANDARD.encode(bytes)})).is_err());
    }
    for mut bytes in [wav(), include_bytes!("audio-test.ogg").to_vec(), include_bytes!("audio-test.mp3").to_vec()] {
        bytes.pop();
        assert!(audio::decode(bytes).is_err());
    }
    let mut ogg = include_bytes!("audio-test.ogg").to_vec();
    let last = ogg.len() - 1;
    ogg[last] ^= 1;
    assert!(audio::decode(ogg).is_err());
    assert!(session.execute("audio.import", &json!({"data": STANDARD.encode(wav()), "name":"not-a-sound.svg"})).is_err());
    assert!(session.execute("audio.import", &json!({"data": STANDARD.encode(wav()), "mime":"audio/mpeg"})).is_err());
    assert!(session.doc().unwrap().audio.sounds.is_empty());
}
#[test]
fn size_and_library_count_are_bounded() {
    assert!(audio::identify(&vec![0; MAX_SOUND_BYTES + 1]).is_err());
    assert!(audio::decode_base64(&"A".repeat(MAX_SOUND_BYTES.div_ceil(3) * 4 + 4)).is_err());
    let mut session = Session::new();
    let original = wav();
    let asset =
        Sound { name: "Original".into(), data: STANDARD.encode(&original).into(), format: AudioFormat::Wav, volume: 1.0, group: "SFX".into() };
    for i in 0..audio::MAX_SOUNDS {
        session.doc_mut().unwrap().audio.sounds.insert(format!("existing-{i}"), asset.clone());
    }
    assert!(session.execute("audio.import", &json!({"data":STANDARD.encode(original)})).is_err());
}
#[test]
fn volumes_multiply_and_reject_invalid_or_ambiguous_values() {
    let mut session = Session::new();
    let id = import(&mut session, &wav(), "Original.wav");
    session.execute("audio.volume", &json!({"volume":0.5})).unwrap();
    session.execute("audio.volume", &json!({"group":"SFX", "volume":0.25})).unwrap();
    session.execute("audio.volume", &json!({"sound":id, "volume":0.8})).unwrap();
    assert!((session.doc().unwrap().audio.effective_volume(&id, 0.5).unwrap() - 0.05).abs() < 0.00001);
    assert!(session.doc().unwrap().audio.effective_volume(&id, f32::NAN).is_err());
    for volume in [json!(-0.1), json!(1.1), json!("loud"), json!(null)] {
        assert!(session.execute("audio.volume", &json!({"volume":volume})).is_err());
    }
    assert!(session.execute("audio.volume", &json!({"sound":id,"group":"SFX","volume":0.5})).is_err());
    session.execute("audio.volume", &json!({"volume":0.0})).unwrap();
    assert_eq!(session.doc().unwrap().audio.effective_volume(&id, 1.0).unwrap(), 0.0);
}
#[test]
fn figcraft_archive_roundtrip_preserves_original_bytes_and_mix() {
    let mut session = Session::new();
    let bytes = wav();
    let id = import(&mut session, &bytes, "Original.wav");
    session.execute("audio.volume", &json!({"volume":0.35})).unwrap();
    let saved = figcraft_format::save_bytes(session.doc().unwrap(), None).unwrap();
    let (restored, _) = figcraft_format::load_bytes(&saved).unwrap();
    assert_eq!(restored.audio, session.doc().unwrap().audio);
    assert_eq!(audio::decode_base64(&restored.audio.sounds[&id].data).unwrap(), bytes);
    let mut old = serde_json::to_value(Document::new("Before audio")).unwrap();
    old.as_object_mut().unwrap().remove("audio");
    let restored: Document = serde_json::from_value(old).unwrap();
    assert_eq!(restored.audio, AudioLibrary::default());
}
#[test]
fn content_ids_deduplicate_and_removed_action_targets_fail_safely() {
    let mut session = Session::new();
    let id = import(&mut session, &wav(), "Original.wav");
    assert_eq!(id, import(&mut session, &wav(), "Renamed.wav"));
    assert_eq!(session.doc().unwrap().audio.sounds.len(), 1);
    session.execute("audio.remove", &json!({"sound":id})).unwrap();
    assert!(session.doc().unwrap().audio.effective_volume(&id, 1.0).is_err());
    assert!(session.execute("audio.remove", &json!({"sound":id})).is_err());
    session.execute("edit.undo", &json!({})).unwrap();
    assert!(session.doc().unwrap().audio.sounds.contains_key(&id));
}
