//! Bounded real audio playback and the per-document sound library editor.
use crate::FigApp;
use figcraft_doc::audio::{self, AudioLibrary, DecodedSound, Sound};
use serde_json::json;
use std::{cell::RefCell, collections::VecDeque, sync::Arc};

const MAX_VOICES: usize = 16;
const MAX_CACHED_SOUNDS: usize = 8;
const ERROR_KEY: &str = "game.audio.error";

#[derive(Default)]
struct Cache {
    entries: VecDeque<(String, Arc<DecodedSound>)>,
    samples: usize,
}
impl Cache {
    fn get(&mut self, sound: &Sound) -> Result<Arc<DecodedSound>, String> {
        let key = audio::content_id(sound.data.as_bytes());
        if let Some((_, data)) = self.entries.iter().find(|(id, _)| id == &key) {
            return Ok(Arc::clone(data));
        }
        let bytes = audio::decode_base64(&sound.data)?;
        let (format, decoded) = audio::decode(bytes)?;
        if format != sound.format {
            return Err("Saved sound format does not match its contents".into());
        }
        while self.entries.len() >= MAX_CACHED_SOUNDS || self.samples.saturating_add(decoded.samples.len()) > audio::MAX_SAMPLES {
            let Some((_, oldest)) = self.entries.pop_front() else { break };
            self.samples = self.samples.saturating_sub(oldest.samples.len());
        }
        self.samples += decoded.samples.len();
        let decoded = Arc::new(decoded);
        self.entries.push_back((key, Arc::clone(&decoded)));
        Ok(decoded)
    }
}

/// Browser resume is asynchronous. Hosts drain this once per frame and show it as a toast.
pub fn take_error(ctx: &egui::Context) -> Option<String> {
    ctx.data_mut(|data| data.remove_temp::<String>(egui::Id::unique(ERROR_KEY)))
}

pub fn unlock(ctx: &egui::Context) -> Result<(), String> {
    backend::unlock(ctx)
}
pub fn play(ctx: &egui::Context, library: &AudioLibrary, id: &str, volume: f32) -> Result<(), String> {
    let gain = library.effective_volume(id, volume)?;
    let sound = library.sounds.get(id).ok_or_else(|| format!("Sound `{id}` is missing"))?;
    // Muted actions do not open an output device or allocate a decoder.
    if gain == 0.0 {
        return Ok(());
    }
    backend::play(ctx, sound, gain)
}

#[cfg(not(target_arch = "wasm32"))]
mod backend {
    use super::{Arc, Cache, DecodedSound, ERROR_KEY, MAX_VOICES, RefCell, Sound};
    use cpal::traits::HostTrait as _;
    use rodio::{OutputStream, OutputStreamBuilder, Sink, Source};
    use std::{
        sync::atomic::{AtomicBool, Ordering},
        time::Duration,
    };
    struct Runtime {
        stream: OutputStream,
        voices: Vec<Sink>,
        cache: Cache,
        failed: Arc<AtomicBool>,
    }
    thread_local! { static RUNTIME: RefCell<Option<Runtime>> = const { RefCell::new(None) }; }
    fn open_output(ctx: &egui::Context, failed: &Arc<AtomicBool>) -> Result<OutputStream, String> {
        let open = |builder: OutputStreamBuilder| {
            let ctx = ctx.clone();
            let failed = Arc::clone(failed);
            builder
                .with_error_callback(move |why| {
                    failed.store(true, Ordering::Relaxed);
                    ctx.data_mut(|data| {
                        data.insert_temp(
                            egui::Id::unique(ERROR_KEY),
                            format!("Audio output stopped: {why}. Check the system output device, then click Preview or Play again."),
                        )
                    });
                    ctx.request_repaint();
                })
                .open_stream_or_fallback()
        };
        let initial = match OutputStreamBuilder::from_default_device().and_then(open) {
            Ok(stream) => return Ok(stream),
            Err(why) => why,
        };
        // Prefer the default output; a missing/busy default is not evidence that every
        // ordinary output device is unusable. No virtual/mock sink is substituted.
        let devices = cpal::default_host().output_devices().map_err(|why| format!("Cannot enumerate audio outputs: {why}"))?;
        for device in devices {
            if let Ok(stream) = OutputStreamBuilder::from_device(device).and_then(open) {
                return Ok(stream);
            }
        }
        Err(format!("Cannot open an audio output device: {initial}. Check your system output device and try Preview again."))
    }
    fn with_runtime<T>(ctx: &egui::Context, f: impl FnOnce(&mut Runtime) -> Result<T, String>) -> Result<T, String> {
        RUNTIME.with(|cell| {
            let mut slot = cell.try_borrow_mut().map_err(|_| "Audio output is busy".to_string())?;
            if slot.as_ref().is_some_and(|runtime| runtime.failed.load(Ordering::Relaxed)) {
                *slot = None;
            }
            if slot.is_none() {
                let failed = Arc::new(AtomicBool::new(false));
                let stream = open_output(ctx, &failed)?;
                *slot = Some(Runtime { stream, voices: Vec::new(), cache: Cache::default(), failed });
            }
            match slot.as_mut() {
                Some(runtime) => f(runtime),
                None => Err("Audio output is unavailable".into()),
            }
        })
    }
    pub(super) fn unlock(ctx: &egui::Context) -> Result<(), String> {
        with_runtime(ctx, |_| Ok(()))
    }
    struct Pcm {
        data: Arc<DecodedSound>,
        at: usize,
    }
    impl Iterator for Pcm {
        type Item = f32;
        fn next(&mut self) -> Option<f32> {
            let value = self.data.samples.get(self.at).copied();
            self.at += usize::from(value.is_some());
            value
        }
        fn size_hint(&self) -> (usize, Option<usize>) {
            let left = self.data.samples.len().saturating_sub(self.at);
            (left, Some(left))
        }
    }
    impl Source for Pcm {
        fn current_span_len(&self) -> Option<usize> {
            Some(self.data.samples.len().saturating_sub(self.at))
        }
        fn channels(&self) -> u16 {
            self.data.channels
        }
        fn sample_rate(&self) -> u32 {
            self.data.sample_rate
        }
        fn total_duration(&self) -> Option<Duration> {
            Some(Duration::from_secs_f64(self.data.samples.len() as f64 / f64::from(self.data.channels) / f64::from(self.data.sample_rate)))
        }
    }
    pub(super) fn play(ctx: &egui::Context, sound: &Sound, gain: f32) -> Result<(), String> {
        with_runtime(ctx, |runtime| {
            runtime.voices.retain(|voice| !voice.empty());
            if runtime.voices.len() >= MAX_VOICES {
                return Err("Audio voice limit reached (16 simultaneous sounds)".into());
            }
            let data = runtime.cache.get(sound)?;
            let sink = Sink::connect_new(runtime.stream.mixer());
            sink.set_volume(gain);
            sink.append(Pcm { data, at: 0 });
            runtime.voices.push(sink);
            Ok(())
        })
    }
}

#[cfg(target_arch = "wasm32")]
mod backend {
    use super::{Cache, ERROR_KEY, MAX_CACHED_SOUNDS, MAX_VOICES, RefCell, Sound, VecDeque, audio};
    use wasm_bindgen::JsValue;
    use web_sys::{AudioBuffer, AudioBufferSourceNode, AudioContext, AudioContextState, GainNode};
    struct Voice {
        source: AudioBufferSourceNode,
        gain: GainNode,
        ends: f64,
    }
    struct Runtime {
        context: AudioContext,
        voices: Vec<Voice>,
        cache: Cache,
        buffers: VecDeque<(String, AudioBuffer, usize)>,
        buffer_samples: usize,
    }
    thread_local! { static RUNTIME: RefCell<Option<Runtime>> = const { RefCell::new(None) }; }
    fn error(value: JsValue) -> String {
        value.as_string().unwrap_or_else(|| format!("{value:?}"))
    }
    fn with_runtime<T>(f: impl FnOnce(&mut Runtime) -> Result<T, String>) -> Result<T, String> {
        RUNTIME.with(|cell| {
            let mut slot = cell.try_borrow_mut().map_err(|_| "Audio context is busy".to_string())?;
            if slot.as_ref().is_some_and(|runtime| runtime.context.state() == AudioContextState::Closed) {
                *slot = None;
            }
            if slot.is_none() {
                let context = AudioContext::new().map_err(|e| format!("Web Audio is unavailable: {}", error(e)))?;
                *slot = Some(Runtime { context, voices: Vec::new(), cache: Cache::default(), buffers: VecDeque::new(), buffer_samples: 0 });
            }
            match slot.as_mut() {
                Some(runtime) => f(runtime),
                None => Err("Web Audio context is unavailable".into()),
            }
        })
    }
    pub(super) fn unlock(ctx: &egui::Context) -> Result<(), String> {
        with_runtime(|runtime| {
            if runtime.context.state() != AudioContextState::Running {
                let promise =
                    runtime.context.resume().map_err(|e| format!("Cannot resume Web Audio: {}. Click Play or Preview to allow sound.", error(e)))?;
                let ctx = ctx.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    if let Err(why) = wasm_bindgen_futures::JsFuture::from(promise).await {
                        let message = format!("Browser blocked sound: {}. Allow audio for this site, then click Play or Preview.", error(why));
                        ctx.data_mut(|data| data.insert_temp(egui::Id::unique(ERROR_KEY), message));
                        ctx.request_repaint();
                    }
                });
            }
            Ok(())
        })
    }
    pub(super) fn play(ctx: &egui::Context, sound: &Sound, gain: f32) -> Result<(), String> {
        unlock(ctx)?;
        with_runtime(|runtime| {
            let now = runtime.context.current_time();
            runtime.voices.retain(|voice| {
                if voice.ends > now {
                    true
                } else {
                    let _ = voice.source.disconnect();
                    let _ = voice.gain.disconnect();
                    false
                }
            });
            if runtime.voices.len() >= MAX_VOICES {
                return Err("Audio voice limit reached (16 simultaneous sounds)".into());
            }
            let key = audio::content_id(sound.data.as_bytes());
            let buffer = if let Some((_, buffer, _)) = runtime.buffers.iter().find(|(id, _, _)| id == &key) {
                buffer.clone()
            } else {
                let decoded = runtime.cache.get(sound)?;
                let frames = decoded.samples.len() / usize::from(decoded.channels);
                let buffer = runtime
                    .context
                    .create_buffer(u32::from(decoded.channels), frames as u32, decoded.sample_rate as f32)
                    .map_err(|e| format!("Cannot allocate Web Audio buffer: {}", error(e)))?;
                if decoded.channels == 1 {
                    buffer.copy_to_channel(&decoded.samples, 0).map_err(|e| format!("Cannot fill Web Audio buffer: {}", error(e)))?;
                } else {
                    let mut data = Vec::with_capacity(frames);
                    for channel in 0..decoded.channels {
                        data.clear();
                        data.extend(decoded.samples.iter().skip(usize::from(channel)).step_by(usize::from(decoded.channels)).copied());
                        buffer.copy_to_channel(&data, i32::from(channel)).map_err(|e| format!("Cannot fill Web Audio buffer: {}", error(e)))?;
                    }
                }
                while runtime.buffers.len() >= MAX_CACHED_SOUNDS || runtime.buffer_samples.saturating_add(decoded.samples.len()) > audio::MAX_SAMPLES
                {
                    let Some((_, _, count)) = runtime.buffers.pop_front() else { break };
                    runtime.buffer_samples = runtime.buffer_samples.saturating_sub(count);
                }
                runtime.buffer_samples += decoded.samples.len();
                runtime.buffers.push_back((key, buffer.clone(), decoded.samples.len()));
                buffer
            };
            let source = runtime.context.create_buffer_source().map_err(|e| format!("Cannot create audio source: {}", error(e)))?;
            source.set_buffer(Some(&buffer));
            let node = runtime.context.create_gain().map_err(|e| format!("Cannot create audio gain: {}", error(e)))?;
            node.gain().set_value(gain);
            source.connect_with_audio_node(&node).map_err(|e| format!("Cannot connect sound: {}", error(e)))?;
            node.connect_with_audio_node(&runtime.context.destination()).map_err(|e| format!("Cannot connect audio output: {}", error(e)))?;
            source.start().map_err(|e| format!("Cannot start sound: {}", error(e)))?;
            runtime.voices.push(Voice { source, gain: node, ends: now + buffer.duration() });
            Ok(())
        })
    }
}

pub fn show(app: &mut FigApp, ui: &mut egui::Ui) {
    if let Some(message) = take_error(ui.ctx()) {
        app.toast(message);
    }
    ui.heading("Sound library");
    ui.label("WAV, OGG Vorbis and MP3 sounds travel with this file.");
    let group_key = egui::Id::unique("game.audio.import-group");
    let mut import_group = ui.ctx().data_mut(|d| d.get_temp::<String>(group_key)).unwrap_or_else(|| "SFX".into());
    ui.horizontal_wrapped(|ui| {
        ui.label("Import into");
        craft_ui::ComboBox::from_id_salt("audio-import-group").selected_text(&import_group).show_ui(ui, |ui| {
            if let Ok(doc) = app.session.doc() {
                for group in doc.audio.groups.keys() {
                    ui.selectable_value(&mut import_group, group.clone(), group);
                }
            }
        });
        let full = app.session.doc().map_or(true, |doc| doc.audio.sounds.len() >= audio::MAX_SOUNDS);
        if ui.add_enabled(!full, egui::Button::new("Import sound…")).on_hover_text("Up to 16 MiB each; 64 sounds and 64 MiB per file").clicked() {
            let group = import_group.clone();
            app.pick_file("Import sound", &["wav", "ogg", "mp3"], true, move |app, mut file| {
                if let Some(object) = file.as_object_mut() {
                    object.insert("group".into(), json!(group));
                }
                app.run_user("audio.import", file);
            });
        }
    });
    ui.ctx().data_mut(|d| d.insert_temp(group_key, import_group));
    ui.add_space(8.0);
    // Copy only editor metadata, never the base64 audio payload or whole document.
    let Ok(doc) = app.session.doc() else {
        ui.label("Open a file to manage sounds.");
        return;
    };
    let mut master = doc.audio.master_volume;
    let groups: Vec<_> = doc.audio.groups.iter().map(|(name, volume)| (name.clone(), *volume)).collect();
    let sounds: Vec<_> =
        doc.audio.sounds.iter().map(|(id, sound)| (id.clone(), sound.name.clone(), sound.group.clone(), sound.volume, sound.format)).collect();
    if ui.add(egui::Slider::new(&mut master, 0.0..=1.0).text("Master volume")).changed() {
        app.run_user("audio.volume", json!({"volume": master}));
    }
    ui.collapsing("Group volumes", |ui| {
        for (group, mut volume) in groups {
            if ui.add(egui::Slider::new(&mut volume, 0.0..=1.0).text(&group)).changed() {
                app.run_user("audio.volume", json!({"group": group, "volume": volume}));
            }
        }
    });
    ui.separator();
    if sounds.is_empty() {
        ui.label("No sounds yet. Import an original or licensed recording, then attach it to a Play sound action.");
    }
    egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
        for (id, name, group, mut volume, format) in sounds {
            ui.push_id(&id, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.strong(&name);
                    ui.label(format!("{} · {group}", format.extension().to_uppercase()));
                    if ui.button("Preview").on_hover_text("Play with master, group and sound volumes").clicked() {
                        let result = unlock(ui.ctx())
                            .and_then(|()| app.session.doc().map_err(|e| e.to_string()).and_then(|doc| play(ui.ctx(), &doc.audio, &id, 1.0)));
                        if let Err(error) = result {
                            app.toast(error);
                        }
                    }
                    if ui.button("Remove").on_hover_text("Remove from this file; actions referring to it will report a missing sound").clicked() {
                        app.run_user("audio.remove", json!({"sound": id}));
                    }
                });
                if ui.add(egui::Slider::new(&mut volume, 0.0..=1.0).text("Sound volume")).changed() {
                    app.run_user("audio.volume", json!({"sound": id, "volume": volume}));
                }
                ui.add_space(8.0);
            });
        }
    });
}
