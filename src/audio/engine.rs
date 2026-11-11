use std::fs::File;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, SampleFormat, StreamConfig};
use crossbeam_channel::{Receiver, Sender, unbounded};
use parking_lot::Mutex;
use symphonia::core::codecs::CodecParameters;
use symphonia::core::codecs::audio::{AudioDecoder, AudioDecoderOptions};
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::units::Time;
use symphonia::default::{get_codecs, get_probe};

use crate::audio::ring::RingBuffer;

#[derive(Debug, Clone)]
pub enum Command {
    Load { path: String, play: bool },
    Play,
    Pause,
    Seek(f64),
    Stop,
    Volume(f32),
    Shutdown,
}

#[derive(Clone, Debug)]
pub struct TrackInfo {
    pub path: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub duration: f64,
}

pub struct EngineShared {
    pub ring: Arc<RingBuffer>,
    pub volume: AtomicU32,
    pub playing: AtomicBool,
    pub finished: AtomicBool,
    pub epoch: AtomicU64,
    pub last_seen_epoch: AtomicU64,
    pub frames_played: AtomicU64,
    pub position_base: AtomicU64,
    pub duration_frames: AtomicU64,
    pub sample_rate: AtomicU32,
    pub underrun: AtomicU64,
    pub ended: AtomicBool,
    pub output_active: AtomicBool,
    pub error: Mutex<Option<String>>,
    pub track: Mutex<Option<TrackInfo>>,
}

impl EngineShared {
    pub fn new(ring: Arc<RingBuffer>, sample_rate: u32) -> Self {
        Self {
            ring,
            volume: AtomicU32::new(1.0f32.to_bits()),
            playing: AtomicBool::new(false),
            finished: AtomicBool::new(false),
            epoch: AtomicU64::new(0),
            last_seen_epoch: AtomicU64::new(0),
            frames_played: AtomicU64::new(0),
            position_base: AtomicU64::new(0),
            duration_frames: AtomicU64::new(0),
            sample_rate: AtomicU32::new(sample_rate),
            underrun: AtomicU64::new(0),
            ended: AtomicBool::new(false),
            output_active: AtomicBool::new(true),
            error: Mutex::new(None),
            track: Mutex::new(None),
        }
    }
}

pub struct AudioEngine {
    cmd_tx: Sender<Command>,
    shared: Arc<EngineShared>,
    _stream: Option<cpal::Stream>,
    _decoder: thread::JoinHandle<()>,
}

impl AudioEngine {
    pub fn new() -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or("Aucun périphérique audio de sortie")?;
        let supported = device
            .supported_output_configs()
            .map_err(|e| e.to_string())?
            .collect::<Vec<_>>();
        // Cherche explicitement une sortie stéréo F32 : le premier format
        // annoncé par certains pilotes est U8/I16, ce qui ne doit pas empêcher
        // ARXFUND de fonctionner.
        let f32_config = supported
            .iter()
            .find(|c| c.sample_format() == SampleFormat::F32 && c.channels() >= 2)
            .cloned()
            .or_else(|| {
                supported
                    .iter()
                    .find(|c| c.sample_format() == SampleFormat::F32)
                    .cloned()
            });
        let (config, sample_format) = if let Some(cfg) = f32_config {
            let range = cfg.with_max_sample_rate();
            (range.config(), range.sample_format())
        } else {
            let def = device.default_output_config().map_err(|e| e.to_string())?;
            (def.config(), def.sample_format())
        };
        if sample_format != SampleFormat::F32 || config.channels < 2 {
            let formats = supported
                .iter()
                .map(|c| format!("{:?}/{}ch", c.sample_format(), c.channels()))
                .collect::<Vec<_>>()
                .join(", ");
            return Err(format!(
                "Aucune sortie stéréo F32 disponible (formats annoncés : {formats})."
            ));
        }
        let device_rate = config.sample_rate;
        let stream_config = StreamConfig {
            channels: 2,
            sample_rate: config.sample_rate,
            buffer_size: BufferSize::Default,
        };

        let ring = Arc::new(RingBuffer::new(1 << 19));
        let shared = Arc::new(EngineShared::new(Arc::clone(&ring), device_rate));

        // Stream callback
        let shared_cb = Arc::clone(&shared);
        let stream = device
            .build_output_stream(
                stream_config,
                move |data: &mut [f32], _| {
                    let epoch_now = shared_cb.epoch.load(Ordering::Acquire);
                    // We can't easily keep cached_epoch in FnMut without a cell; use a thread-local-ish via Atomic on shared? Use a separate atomic in shared.
                    // Simpler: store cached epoch in shared as last_seen_epoch.
                    if shared_cb.last_seen_epoch.load(Ordering::Relaxed) != epoch_now {
                        shared_cb.ring.clear_consumer();
                        shared_cb
                            .last_seen_epoch
                            .store(epoch_now, Ordering::Relaxed);
                        shared_cb.frames_played.store(shared_cb.position_base.load(Ordering::Relaxed), Ordering::Relaxed);
                    }
                    if !shared_cb.playing.load(Ordering::Relaxed) {
                        for s in data.iter_mut() {
                            *s = 0.0;
                        }
                        return;
                    }
                    let n = shared_cb.ring.read(data);
                    if n < data.len() {
                        for s in &mut data[n..] {
                            *s = 0.0;
                        }
                        shared_cb.underrun.fetch_add(1, Ordering::Relaxed);
                        if shared_cb.finished.load(Ordering::Acquire) && shared_cb.ring.is_empty() {
                            shared_cb.ended.store(true, Ordering::Release);
                        }
                    }
                    let vol = f32::from_bits(shared_cb.volume.load(Ordering::Relaxed));
                    for s in data.iter_mut() {
                        *s *= vol;
                    }
                    let frames = (n / 2) as u64;
                    shared_cb.frames_played.fetch_add(frames, Ordering::Relaxed);
                },
                |err| eprintln!("Erreur stream cpal : {}", err),
                None,
            )
            .map_err(|e| e.to_string())?;

        stream.play().map_err(|e| e.to_string())?;

        let (tx, rx) = unbounded::<Command>();
        let shared_dec = Arc::clone(&shared);
        let decoder = thread::spawn(move || decoder_loop(shared_dec, rx, device_rate));

        Ok(Self {
            cmd_tx: tx,
            shared,
            _stream: Some(stream),
            _decoder: decoder,
        })
    }

    /// Moteur de secours lorsqu'aucune sortie audio n'est disponible.
    ///
    /// L'application reste utilisable (bibliothèque, studio, export) et affiche
    /// un message explicite au lieu de planter au démarrage.
    pub fn silent(reason: impl Into<String>) -> Self {
        let device_rate = 48_000u32;
        let ring = Arc::new(RingBuffer::new(1 << 19));
        let shared = Arc::new(EngineShared::new(Arc::clone(&ring), device_rate));
        shared.output_active.store(false, Ordering::Release);
        *shared.error.lock() = Some(reason.into());
        let (tx, rx) = unbounded::<Command>();
        let shared_dec = Arc::clone(&shared);
        let decoder = thread::spawn(move || decoder_loop(shared_dec, rx, device_rate));
        Self {
            cmd_tx: tx,
            shared,
            _stream: None,
            _decoder: decoder,
        }
    }

    pub fn has_output(&self) -> bool {
        self.shared.output_active.load(Ordering::Acquire)
    }

    pub fn shared(&self) -> Arc<EngineShared> {
        Arc::clone(&self.shared)
    }

    pub fn load(&self, path: String, play: bool) {
        let _ = self.cmd_tx.send(Command::Load { path, play });
    }
    pub fn play(&self) {
        let _ = self.cmd_tx.send(Command::Play);
    }
    pub fn pause(&self) {
        let _ = self.cmd_tx.send(Command::Pause);
    }
    pub fn seek(&self, secs: f64) {
        let _ = self.cmd_tx.send(Command::Seek(secs));
    }
    pub fn stop(&self) {
        let _ = self.cmd_tx.send(Command::Stop);
    }
    pub fn set_volume(&self, v: f32) {
        self.shared
            .volume
            .store(v.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
    }
    pub fn volume(&self) -> f32 {
        f32::from_bits(self.shared.volume.load(Ordering::Relaxed))
    }
    pub fn is_playing(&self) -> bool {
        self.shared.playing.load(Ordering::Relaxed)
    }
    pub fn position_secs(&self) -> f64 {
        let rate = self.shared.sample_rate.load(Ordering::Relaxed) as f64;
        self.shared.frames_played.load(Ordering::Relaxed) as f64 / rate
    }
    pub fn duration_secs(&self) -> f64 {
        let rate = self.shared.sample_rate.load(Ordering::Relaxed) as f64;
        self.shared.duration_frames.load(Ordering::Relaxed) as f64 / rate
    }
    pub fn is_finished(&self) -> bool {
        self.shared.ended.swap(false, Ordering::AcqRel)
    }
    pub fn take_error(&self) -> Option<String> {
        self.shared.error.lock().take()
    }
    pub fn track(&self) -> Option<TrackInfo> {
        self.shared.track.lock().clone()
    }
}

// last_seen_epoch field added to EngineShared via extra field? Need to add field.
// We'll add `last_seen_epoch: AtomicU64` to EngineShared.

fn to_stereo(input: &[f32], channels: usize) -> Vec<f32> {
    if channels == 2 {
        return input.to_vec();
    }
    let frames = input.len() / channels;
    let mut out = vec![0.0f32; frames * 2];
    for f in 0..frames {
        let l = input[f * channels];
        let r = if channels > 1 {
            input[f * channels + 1]
        } else {
            l
        };
        out[f * 2] = l;
        out[f * 2 + 1] = r;
    }
    out
}

struct LinearResampler {
    ratio: f64,
}

impl LinearResampler {
    fn new(in_rate: u32, out_rate: u32) -> Self {
        Self {
            ratio: out_rate as f64 / in_rate as f64,
        }
    }
    fn process(&self, input: &[f32]) -> Vec<f32> {
        if (self.ratio - 1.0).abs() < 1e-9 {
            return input.to_vec();
        }
        let channels = 2usize;
        let in_frames = input.len() / channels;
        if in_frames == 0 {
            return Vec::new();
        }
        let out_frames = ((in_frames as f64) * self.ratio).round().max(1.0) as usize;
        let mut out = vec![0.0f32; out_frames * channels];
        for of in 0..out_frames {
            let pos = of as f64 / self.ratio;
            let i0 = pos.floor() as usize;
            let frac = (pos - i0 as f64) as f32;
            let i1 = (i0 + 1).min(in_frames - 1);
            for c in 0..channels {
                let a = input[i0 * channels + c];
                let b = input[i1 * channels + c];
                out[of * channels + c] = a + (b - a) * frac;
            }
        }
        out
    }
}

fn decoder_loop(shared: Arc<EngineShared>, rx: Receiver<Command>, device_rate: u32) {
    let mut format: Option<Box<dyn FormatReader>> = None;
    let mut decoder: Option<Box<dyn AudioDecoder>> = None;
    let mut track_id: Option<u32> = None;
    let mut resampler: Option<LinearResampler> = None;
    let mut resample_needed = false;
    let mut pending = Vec::new();
    let mut pending_pos = 0;

    loop {
        // Handle commands
        while let Ok(cmd) = rx.try_recv() {
            match cmd {
                Command::Shutdown => return,
                Command::Volume(v) => {
                    shared
                        .volume
                        .store(v.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
                }
                Command::Play => shared.playing.store(true, Ordering::Relaxed),
                Command::Pause => shared.playing.store(false, Ordering::Relaxed),
                Command::Stop => {
                    shared.playing.store(false, Ordering::Relaxed);
                    shared.finished.store(false, Ordering::Relaxed);
                    shared.ended.store(false, Ordering::Relaxed);
                }
                Command::Load { path, play } => {
                    // reset state
                    pending.clear();
                    pending_pos = 0;
                    shared.position_base.store(0, Ordering::Relaxed);
                    *shared.track.lock() = None;
                    shared.playing.store(false, Ordering::Relaxed);
                    shared.finished.store(false, Ordering::Relaxed);
                    shared.ended.store(false, Ordering::Relaxed);
                    shared.frames_played.store(0, Ordering::Relaxed);
                    shared.duration_frames.store(0, Ordering::Relaxed);
                    shared.epoch.fetch_add(1, Ordering::AcqRel);
                    format = None;
                    decoder = None;
                    track_id = None;
                    resampler = None;
                    resample_needed = false;

                    // open file
                    let file = match File::open(&path) {
                        Ok(f) => f,
                        Err(e) => {
                            *shared.error.lock() = Some(format!("Ouverture impossible : {}", e));
                            continue;
                        }
                    };
                    let mss = MediaSourceStream::new(Box::new(file), Default::default());
                    let probe = get_probe();
                    let hint = Hint::new();
                    match probe.probe(
                        &hint,
                        mss,
                        FormatOptions::default(),
                        MetadataOptions::default(),
                    ) {
                        Ok(fmt) => {
                            let t = fmt.default_track(symphonia::core::formats::TrackType::Audio);
                            let t = match t {
                                Some(t) => t,
                                None => {
                                    *shared.error.lock() = Some("Aucune piste audio".to_string());
                                    continue;
                                }
                            };
                            track_id = Some(t.id);
                            let cp = match t.codec_params.clone() {
                                Some(CodecParameters::Audio(a)) => a,
                                _ => {
                                    *shared.error.lock() =
                                        Some("Paramètres codec indisponibles".to_string());
                                    continue;
                                }
                            };
                            let sr_file = cp.sample_rate.unwrap_or(device_rate);
                            let channels =
                                cp.channels.as_ref().map(|c| c.count() as u16).unwrap_or(2);
                            let n_frames = t.num_frames.unwrap_or(0);
                            let duration = if let Some(tb) = t.time_base {
                                n_frames as f64 * tb.numer.get() as f64 / tb.denom.get() as f64
                            } else {
                                0.0
                            };
                            let reg = get_codecs();
                            let registered = match reg.get_audio_decoder(cp.codec) {
                                Some(r) => r,
                                None => {
                                    *shared.error.lock() = Some("Codec inconnu".to_string());
                                    continue;
                                }
                            };
                            let dec =
                                match (registered.factory)(&cp, &AudioDecoderOptions::default()) {
                                    Ok(d) => d,
                                    Err(e) => {
                                        *shared.error.lock() =
                                            Some(format!("Décodeur indisponible : {}", e));
                                        continue;
                                    }
                                };
                            if sr_file != device_rate {
                                resampler = Some(LinearResampler::new(sr_file, device_rate));
                                resample_needed = true;
                            }
                            shared.sample_rate.store(device_rate, Ordering::Relaxed);
                            shared.duration_frames.store(
                                (duration * device_rate as f64).round() as u64,
                                Ordering::Relaxed,
                            );
                            *shared.track.lock() = Some(TrackInfo {
                                path: path.clone(),
                                sample_rate: sr_file,
                                channels,
                                duration,
                            });
                            format = Some(fmt);
                            decoder = Some(dec);
                            shared.playing.store(play, Ordering::Relaxed);
                        }
                        Err(e) => {
                            *shared.error.lock() = Some(format!("Format non reconnu : {}", e));
                        }
                    }
                }
                Command::Seek(secs) => {
                    if let Some(fmt) = format.as_mut() {
                        let to = SeekTo::Time {
                            time: Time::try_from_secs_f64(secs).unwrap_or_default(),
                            track_id,
                        };
                        match fmt.seek(SeekMode::Coarse, to) {
                            Ok(result) => {
                                if let Some(dec) = decoder.as_mut() { dec.reset(); }
                                let actual = fmt.tracks().iter().find(|t| t.id == result.track_id)
                                    .and_then(|t| t.time_base)
                                    .map(|tb| result.actual_ts.get() as f64 * tb.numer.get() as f64 / tb.denom.get() as f64)
                                    .unwrap_or(secs.max(0.0));
                                pending.clear();
                                pending_pos = 0;
                                shared.position_base.store((actual * device_rate as f64) as u64, Ordering::Relaxed);
                                shared.finished.store(false, Ordering::Relaxed);
                                shared.ended.store(false, Ordering::Relaxed);
                                shared.epoch.fetch_add(1, Ordering::AcqRel);
                            }
                            Err(e) => *shared.error.lock() = Some(format!("Déplacement impossible : {e}")),
                        }
                    }
                }
            }
        }

        if !shared.output_active.load(Ordering::Acquire) {
            thread::sleep(Duration::from_millis(50));
            continue;
        }

        if !shared.playing.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(10));
            continue;
        }
        // The consumer acknowledges the flush before new audio is queued.
        if shared.last_seen_epoch.load(Ordering::Acquire) != shared.epoch.load(Ordering::Acquire) {
            thread::sleep(Duration::from_millis(2));
            continue;
        }
        if pending_pos < pending.len() {
            pending_pos += shared.ring.write(&pending[pending_pos..]);
            if pending_pos < pending.len() { thread::sleep(Duration::from_millis(2)); }
            continue;
        }
        let Some(fmt) = format.as_mut() else {
            thread::sleep(Duration::from_millis(10));
            continue;
        };
        let Some(dec) = decoder.as_mut() else {
            thread::sleep(Duration::from_millis(10));
            continue;
        };

        match fmt.next_packet() {
            Ok(Some(packet)) => {
                if Some(packet.track_id) != track_id {
                    continue;
                }
                match dec.decode_ref(&packet.as_packet_ref()) {
                    Ok(decoded) => {
                        let spec = decoded.spec();
                        let frames = decoded.frames();
                        let ch = spec.channels().count() as usize;
                        let mut samples = vec![0.0f32; frames * ch];
                        decoded.copy_to_slice_interleaved::<f32, _>(&mut samples);
                        let stereo = to_stereo(&samples, ch);
                        let out_samples = if resample_needed {
                            resampler.as_ref().unwrap().process(&stereo)
                        } else {
                            stereo
                        };
                        pending = out_samples;
                        pending_pos = 0;
                    }
                    Err(_) => {}
                }
            }
            Ok(None) => {
                shared.finished.store(true, Ordering::Release);
                thread::sleep(Duration::from_millis(50));
            }
            Err(_) => {
                thread::sleep(Duration::from_millis(10));
            }
        }
    }
}
