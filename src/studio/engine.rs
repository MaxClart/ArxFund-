//! Moteur audio du studio : stream cpal + thread de rendu du séquenceur.
//!
//! Le callback audio ne fait que vider un ring buffer SPSC lock-free. Tout le
//! séquenceur, la synthèse et le mixage sont exécutés sur le thread de rendu,
//! ce qui respecte les contraintes temps réel (aucune allocation, aucun accès
//! disque dans le callback).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, SampleFormat};
use crossbeam_channel::{Receiver, Sender, unbounded};
use parking_lot::Mutex;

use crate::audio::ring::RingBuffer;
use crate::studio::project::Project;
use crate::studio::samples::SampleBank;
use crate::studio::sequencer::Sequencer;

const BLOCK_FRAMES: usize = 512;
const RING_SAMPLES: usize = 1 << 16;

pub enum StudioCommand {
    Play,
    Pause,
    Stop,
    SeekStep(u64),
    SetProject(Box<Project>),
    SetMetronome(bool),
    SetSamples(SampleBank),
    Shutdown,
}

pub struct StudioShared {
    ring: Arc<RingBuffer>,
    playing: AtomicBool,
    frames_played: AtomicU64,
    position_base: AtomicU64,
    epoch: AtomicU64,
    last_seen_epoch: AtomicU64,
    current_step: AtomicU64,
    cpu_permille: AtomicU32,
    sample_rate: u32,
    /// Pic du dernier bloc rendu, par canal (bits f32, AtomicU32).
    channel_peaks: [AtomicU32; super::sequencer::MAX_CHANNELS],
    /// Pic master du dernier bloc rendu.
    master_peak: AtomicU32,
    error: Mutex<Option<String>>,
}

impl StudioShared {
    fn new(ring: Arc<RingBuffer>, sample_rate: u32) -> Self {
        Self {
            ring,
            playing: AtomicBool::new(false),
            frames_played: AtomicU64::new(0),
            position_base: AtomicU64::new(0),
            epoch: AtomicU64::new(0),
            last_seen_epoch: AtomicU64::new(0),
            current_step: AtomicU64::new(0),
            cpu_permille: AtomicU32::new(0),
            sample_rate,
            channel_peaks: std::array::from_fn(|_| AtomicU32::new(0)),
            master_peak: AtomicU32::new(0),
            error: Mutex::new(None),
        }
    }
}

pub struct StudioEngine {
    cmd_tx: Sender<StudioCommand>,
    shared: Arc<StudioShared>,
    _stream: cpal::Stream,
    thread: Option<JoinHandle<()>>,
    buffer_frames: u32,
}

impl StudioEngine {
    pub fn new() -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or("Aucun périphérique audio de sortie")?;
        let supported = device
            .supported_output_configs()
            .map_err(|e| e.to_string())?
            .collect::<Vec<_>>();

        let range = supported
            .iter()
            .filter(|c| c.sample_format() == SampleFormat::F32 && c.channels() == 2)
            .max_by_key(|c| {
                (
                    (c.min_sample_rate()..=c.max_sample_rate()).contains(&48_000),
                    (c.min_sample_rate()..=c.max_sample_rate()).contains(&44_100),
                    c.max_sample_rate(),
                )
            })
            .cloned()
            .ok_or("Aucune sortie stéréo f32 disponible")?;
        let min = range.min_sample_rate();
        let max = range.max_sample_rate();
        let rate = if (min..=max).contains(&48_000) {
            48_000
        } else if (min..=max).contains(&44_100) {
            44_100
        } else {
            max
        };
        let chosen = range.with_sample_rate(rate);
        let sample_rate = chosen.sample_rate();
        let mut config = chosen.config();
        config.channels = 2;
        config.buffer_size = BufferSize::Fixed(1024);

        let ring = Arc::new(RingBuffer::new(RING_SAMPLES));
        let shared = Arc::new(StudioShared::new(Arc::clone(&ring), sample_rate));

        let error_slot = Arc::clone(&shared);
        let shared_cb = Arc::clone(&shared);
        let data_cb = move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
            let epoch_now = shared_cb.epoch.load(Ordering::Acquire);
            if shared_cb.last_seen_epoch.load(Ordering::Relaxed) != epoch_now {
                shared_cb.ring.clear_consumer();
                shared_cb
                    .last_seen_epoch
                    .store(epoch_now, Ordering::Relaxed);
                shared_cb.frames_played.store(
                    shared_cb.position_base.load(Ordering::Relaxed),
                    Ordering::Relaxed,
                );
            }
            if !shared_cb.playing.load(Ordering::Relaxed) {
                data.fill(0.0);
                return;
            }
            let n = shared_cb.ring.read(data);
            if n < data.len() {
                data[n..].fill(0.0);
            }
            let frames = (data.len() / 2) as u64;
            shared_cb.frames_played.fetch_add(frames, Ordering::Relaxed);
        };
        let err_cb = move |err: cpal::Error| {
            eprintln!("Erreur stream studio : {err}");
            if let Some(mut slot) = error_slot.error.try_lock() {
                *slot = Some(format!("Erreur audio : {err}"));
            }
        };

        let (stream, buffer_frames) =
            match device.build_output_stream(config.clone(), data_cb, err_cb, None) {
                Ok(stream) => (stream, 1024),
                Err(_) => {
                    config.buffer_size = BufferSize::Default;
                    let shared_cb = Arc::clone(&shared);
                    let error_slot = Arc::clone(&shared);
                    let data_cb = move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                        let epoch_now = shared_cb.epoch.load(Ordering::Acquire);
                        if shared_cb.last_seen_epoch.load(Ordering::Relaxed) != epoch_now {
                            shared_cb.ring.clear_consumer();
                            shared_cb
                                .last_seen_epoch
                                .store(epoch_now, Ordering::Relaxed);
                            shared_cb.frames_played.store(
                                shared_cb.position_base.load(Ordering::Relaxed),
                                Ordering::Relaxed,
                            );
                        }
                        if !shared_cb.playing.load(Ordering::Relaxed) {
                            data.fill(0.0);
                            return;
                        }
                        let n = shared_cb.ring.read(data);
                        if n < data.len() {
                            data[n..].fill(0.0);
                        }
                        let frames = (data.len() / 2) as u64;
                        shared_cb.frames_played.fetch_add(frames, Ordering::Relaxed);
                    };
                    let err_cb = move |err: cpal::Error| {
                        eprintln!("Erreur stream studio : {err}");
                        if let Some(mut slot) = error_slot.error.try_lock() {
                            *slot = Some(format!("Erreur audio : {err}"));
                        }
                    };
                    let stream = device
                        .build_output_stream(config, data_cb, err_cb, None)
                        .map_err(|e| format!("Impossible d'ouvrir la sortie audio : {e}"))?;
                    (stream, 512)
                }
            };
        stream
            .play()
            .map_err(|e| format!("Démarrage du stream : {e}"))?;

        let (tx, rx) = unbounded::<StudioCommand>();
        let shared_thread = Arc::clone(&shared);
        let project = Project::new_default("Session");
        let sample_rate_f = sample_rate as f32;
        let handle = thread::spawn(move || {
            render_loop(shared_thread, rx, project, SampleBank::new(), sample_rate_f);
        });

        Ok(Self {
            cmd_tx: tx,
            shared,
            _stream: stream,
            thread: Some(handle),
            buffer_frames,
        })
    }

    pub fn play(&self) {
        let _ = self.cmd_tx.send(StudioCommand::Play);
    }
    pub fn pause(&self) {
        let _ = self.cmd_tx.send(StudioCommand::Pause);
    }
    pub fn stop(&self) {
        let _ = self.cmd_tx.send(StudioCommand::Stop);
    }
    pub fn seek_step(&self, step: u64) {
        let _ = self.cmd_tx.send(StudioCommand::SeekStep(step));
    }
    pub fn set_project(&self, project: &Project) {
        let _ = self
            .cmd_tx
            .send(StudioCommand::SetProject(Box::new(project.clone())));
    }
    pub fn set_metronome(&self, on: bool) {
        let _ = self.cmd_tx.send(StudioCommand::SetMetronome(on));
    }
    pub fn set_samples(&self, bank: SampleBank) {
        let _ = self.cmd_tx.send(StudioCommand::SetSamples(bank));
    }

    pub fn is_playing(&self) -> bool {
        self.shared.playing.load(Ordering::Relaxed)
    }
    pub fn current_step(&self) -> usize {
        self.shared.current_step.load(Ordering::Relaxed) as usize
    }
    pub fn position_secs(&self) -> f64 {
        self.shared.frames_played.load(Ordering::Relaxed) as f64
            / self.shared.sample_rate.max(1) as f64
    }
    pub fn sample_rate(&self) -> u32 {
        self.shared.sample_rate
    }
    pub fn buffer_frames(&self) -> u32 {
        self.buffer_frames
    }
    pub fn latency_ms(&self) -> f32 {
        self.buffer_frames as f32 * 1000.0 / self.shared.sample_rate.max(1) as f32
    }
    pub fn cpu_load(&self) -> f32 {
        self.shared.cpu_permille.load(Ordering::Relaxed) as f32 / 1000.0
    }
    pub fn take_error(&self) -> Option<String> {
        self.shared.error.lock().take()
    }

    /// Niveau du VU-meter pour un canal donné (valeur entre 0.0 et 1.0+).
    /// Mesuré sur le dernier bloc rendu par le thread de synthèse (pas dans le callback).
    pub fn channel_level(&self, channel_index: usize) -> f32 {
        let bits = self
            .shared
            .channel_peaks
            .get(channel_index)
            .map(|slot| slot.load(Ordering::Relaxed))
            .unwrap_or(0);
        let v = f32::from_bits(bits);
        if v.is_finite() && v >= 0.0 { v } else { 0.0 }
    }

    /// Niveau VU-meter du master (0.0 à 1.0+).
    pub fn master_level(&self) -> f32 {
        let v = f32::from_bits(self.shared.master_peak.load(Ordering::Relaxed));
        if v.is_finite() && v >= 0.0 { v } else { 0.0 }
    }
}

impl Drop for StudioEngine {
    fn drop(&mut self) {
        let _ = self.cmd_tx.send(StudioCommand::Shutdown);
        if let Some(h) = self.thread.take() {
            let _ = h.join();
        }
    }
}

fn write_block(ring: &RingBuffer, buf: &[f32], _rx: &Receiver<StudioCommand>) -> bool {
    ring.write(buf) == buf.len()
}

fn seconds_to_step(secs: f64, project: &Project, seq: &Sequencer) -> u64 {
    let sps = seq.samples_per_step(project).max(1.0);
    ((secs * seq.sample_rate as f64) / sps).round().max(0.0) as u64
}

fn render_loop(
    shared: Arc<StudioShared>,
    rx: Receiver<StudioCommand>,
    mut project: Project,
    mut bank: SampleBank,
    sample_rate: f32,
) {
    let mut seq = Sequencer::new(sample_rate);
    let mut metronome = false;
    let mut playing = false;
    let mut buf = vec![0.0f32; BLOCK_FRAMES * 2];
    let prefill_target = (sample_rate as usize / 20).max(BLOCK_FRAMES) * 2; // ~50 ms

    loop {
        while let Ok(cmd) = rx.try_recv() {
            match cmd {
                StudioCommand::Shutdown => return,
                StudioCommand::SetProject(p) => project = *p,
                StudioCommand::SetSamples(b) => bank = b,
                StudioCommand::SetMetronome(on) => metronome = on,
                StudioCommand::Play => {
                    if !playing {
                        wait_epoch_ack(&shared);
                        let mut guard = 0;
                        while shared.ring.len() < prefill_target && guard < 200 {
                            seq.render_block(&project, &bank, metronome, &mut buf);
                            shared
                                .current_step
                                .store(seq.last_local_step as u64, Ordering::Relaxed);
                            if !write_block(&shared.ring, &buf, &rx) {
                                return;
                            }
                            guard += 1;
                        }
                        shared.playing.store(true, Ordering::Release);
                        playing = true;
                    }
                }
                StudioCommand::Pause => {
                    if playing {
                        let audible_frames = shared.frames_played.load(Ordering::Relaxed);
                        let secs = audible_frames as f64 / sample_rate as f64;
                        let step = seconds_to_step(secs, &project, &seq);
                        seq.seek_step(&project, step);
                        shared
                            .position_base
                            .store(audible_frames, Ordering::Relaxed);
                        shared
                            .current_step
                            .store(seq.last_local_step as u64, Ordering::Relaxed);
                        shared.epoch.fetch_add(1, Ordering::AcqRel);
                        shared.playing.store(false, Ordering::Release);
                        playing = false;
                    }
                }
                StudioCommand::Stop => {
                    playing = false;
                    shared.playing.store(false, Ordering::Release);
                    seq.reset();
                    shared.position_base.store(0, Ordering::Relaxed);
                    shared.current_step.store(0, Ordering::Relaxed);
                    shared.cpu_permille.store(0, Ordering::Relaxed);
                    shared.epoch.fetch_add(1, Ordering::AcqRel);
                }
                StudioCommand::SeekStep(step) => {
                    seq.seek_step(&project, step);
                    let sps = seq.samples_per_step(&project);
                    shared
                        .position_base
                        .store((step as f64 * sps) as u64, Ordering::Relaxed);
                    shared
                        .current_step
                        .store(seq.last_local_step as u64, Ordering::Relaxed);
                    shared.epoch.fetch_add(1, Ordering::AcqRel);
                    if playing {
                        shared.playing.store(false, Ordering::Release);
                        wait_epoch_ack(&shared);
                        let mut guard = 0;
                        while shared.ring.len() < prefill_target && guard < 200 {
                            seq.render_block(&project, &bank, metronome, &mut buf);
                            if !write_block(&shared.ring, &buf, &rx) {
                                return;
                            }
                            guard += 1;
                        }
                        shared.playing.store(true, Ordering::Release);
                    }
                }
            }
        }

        if !playing {
            shared.cpu_permille.store(0, Ordering::Relaxed);
            shared.master_peak.store(0, Ordering::Relaxed);
            for peak in &shared.channel_peaks {
                peak.store(0, Ordering::Relaxed);
            }
            thread::sleep(Duration::from_millis(5));
            continue;
        }

        if shared.ring.len() >= prefill_target || shared.ring.free() < buf.len() {
            thread::sleep(Duration::from_millis(2));
            continue;
        }
        let t0 = Instant::now();
        seq.render_block(&project, &bank, metronome, &mut buf);
        for (ci, &peak) in seq.channel_peaks.iter().enumerate() {
            shared
                .channel_peaks
                .get(ci)
                .map(|slot| slot.store(peak.to_bits(), Ordering::Relaxed));
        }
        shared
            .master_peak
            .store(seq.master_peak.to_bits(), Ordering::Relaxed);
        let elapsed = t0.elapsed().as_secs_f64();
        let block_secs = BLOCK_FRAMES as f64 / sample_rate as f64;
        let cpu = (elapsed / block_secs * 1000.0).clamp(0.0, 9999.0) as u32;
        shared.cpu_permille.store(cpu, Ordering::Relaxed);
        shared
            .current_step
            .store(seq.last_local_step as u64, Ordering::Relaxed);
        if !write_block(&shared.ring, &buf, &rx) {
            return;
        }
    }
}

/// Attend que le callback audio ait pris en compte le dernier changement
/// d'époque (flush du ring). Évite de réécrire par-dessus des données périmées.
fn wait_epoch_ack(shared: &StudioShared) {
    let target = shared.epoch.load(Ordering::Acquire);
    let deadline = Instant::now() + Duration::from_millis(80);
    while shared.last_seen_epoch.load(Ordering::Relaxed) != target {
        if Instant::now() >= deadline {
            break;
        }
        thread::sleep(Duration::from_millis(1));
    }
}
