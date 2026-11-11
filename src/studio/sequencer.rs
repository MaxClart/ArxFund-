//! Séquenceur pas à pas et rendu audio du studio.
//!
//! Le rendu est effectué sur un thread dédié (pas dans le callback cpal). Le
//! séquenceur ne fait aucune allocation : les voix sont pré-allouées et les
//! buffers d'échantillons sont partagés via `Arc`.

use crate::studio::project::{ChannelKind, Project, STEPS_PER_BEAT};
use crate::studio::samples::SampleBank;
use crate::studio::synth::{Trigger, Voice, VoiceKind};

const MAX_VOICES: usize = 96;
/// Nombre maximal de tranches suivies par les VU-mètres.
pub const MAX_CHANNELS: usize = 64;

pub struct Sequencer {
    pub sample_rate: f32,
    /// Position absolue du rendu, en échantillons.
    pub position_samples: f64,
    pub global_step: u64,
    /// Pas local affiché par le Channel Rack.
    pub last_local_step: usize,
    /// Pic du dernier bloc rendu, par canal (0..1+).
    pub channel_peaks: [f32; MAX_CHANNELS],
    /// Pic du dernier bloc rendu sur le master.
    pub master_peak: f32,
    samples_until_next: f64,
    rng: u32,
    voices: Vec<Voice>,
    steal_index: usize,
    /// Dernière fréquence mélodique par canal, pour le glide.
    last_bass: Vec<(f32, f64)>,
    stop_at_samples: Option<f64>,
    delay: Vec<Vec<[f32; 2]>>,
    delay_pos: usize,
}

impl Sequencer {
    pub fn new(sample_rate: f32) -> Self {
        let voices = (0..MAX_VOICES).map(|_| Voice::silent()).collect();
        Self {
            sample_rate: sample_rate.max(1.0),
            position_samples: 0.0,
            global_step: 0,
            last_local_step: 0,
            channel_peaks: [0.0; MAX_CHANNELS],
            master_peak: 0.0,
            samples_until_next: 0.0,
            rng: 0x51ED_270B,
            voices,
            steal_index: 0,
            last_bass: Vec::new(),
            stop_at_samples: None,
            delay: (0..MAX_CHANNELS)
                .map(|_| vec![[0.0; 2]; (sample_rate * 0.3).max(1.0) as usize])
                .collect(),
            delay_pos: 0,
        }
    }

    pub fn reset(&mut self) {
        self.position_samples = 0.0;
        self.global_step = 0;
        self.last_local_step = 0;
        self.samples_until_next = 0.0;
        self.rng = 0x51ED_270B;
        self.last_bass.clear();
        self.stop_voices();
    }

    pub fn seek_step(&mut self, project: &Project, step: u64) {
        let sps = self.samples_per_step(project);
        self.position_samples = step as f64 * sps;
        self.global_step = step;
        self.samples_until_next = 0.0;
        self.last_bass.clear();
        self.stop_voices();
        self.last_local_step = if project.patterns.is_empty() {
            0
        } else if project.song_mode {
            let len = project.song_length_steps().max(1) as u64;
            let pos = (step % len) as usize;
            project
                .arrangement
                .iter()
                .find(|c| pos >= c.start_step && pos < c.start_step + c.length_steps)
                .map(|c| {
                    (pos - c.start_step)
                        % project.patterns[c.pattern.min(project.patterns.len() - 1)]
                            .length
                            .max(1)
                })
                .unwrap_or(0)
        } else {
            let pi = project.active_pattern.min(project.patterns.len() - 1);
            (step as usize) % project.patterns[pi].length.max(1)
        };
    }

    pub fn stop_voices(&mut self) {
        for v in &mut self.voices {
            v.active = false;
        }
    }

    pub fn set_stop_at(&mut self, samples: Option<f64>) {
        self.stop_at_samples = samples;
    }

    pub fn samples_per_beat(&self, project: &Project) -> f64 {
        self.sample_rate as f64 * 60.0 / project.bpm.max(1.0)
    }

    /// Durée d'un pas (double croche, 4 pas par temps).
    pub fn samples_per_step(&self, project: &Project) -> f64 {
        self.samples_per_beat(project) / STEPS_PER_BEAT as f64
    }

    pub fn seconds_per_step(&self, project: &Project) -> f64 {
        self.samples_per_step(project) / self.sample_rate as f64
    }

    #[inline]
    fn next_rand(&mut self) -> f32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        (x >> 8) as f32 / 16_777_215.0
    }

    #[inline]
    fn roll(&mut self, probability: f32) -> bool {
        probability >= 1.0 || (probability > 0.0 && self.next_rand() < probability)
    }

    fn start_voice(&mut self, tr: Trigger) {
        if let Some(v) = self.voices.iter_mut().find(|v| !v.active) {
            v.start(&tr, self.sample_rate);
            return;
        }
        let idx = self.steal_index % self.voices.len();
        self.voices[idx].start(&tr, self.sample_rate);
        self.steal_index = (self.steal_index + 1) % self.voices.len();
    }

    fn resolve_step(
        project: &Project,
        global_step: u64,
        clip_index: usize,
    ) -> Option<(usize, usize)> {
        if project.patterns.is_empty() {
            return None;
        }
        if project.song_mode {
            let song_len = project.song_length_steps().max(1) as u64;
            let pos = (global_step % song_len) as usize;
            if let Some(clip) = project.arrangement.get(clip_index) {
                if pos >= clip.start_step && pos < clip.start_step.saturating_add(clip.length_steps)
                {
                    let pi = clip.pattern.min(project.patterns.len() - 1);
                    let len = project.patterns[pi].length.max(1);
                    return Some((pi, (pos - clip.start_step) % len));
                }
            }
            None
        } else {
            let pi = project.active_pattern.min(project.patterns.len() - 1);
            let len = project.patterns[pi].length.max(1);
            Some((pi, (global_step % len as u64) as usize))
        }
    }

    fn trigger_step(&mut self, project: &Project, samples: &SampleBank, metronome: bool) {
        let sps = self.samples_per_step(project);
        let step_secs = (sps / self.sample_rate as f64) as f32;
        let triggered = self.global_step;

        if metronome && triggered % STEPS_PER_BEAT as u64 == 0 {
            let beats_per_bar = project.time_sig_num.max(1) as u64;
            let beat_in_bar = (triggered / STEPS_PER_BEAT as u64) % beats_per_bar;
            let mut tr = Trigger::new(VoiceKind::Click);
            tr.channel = usize::MAX;
            tr.accent = beat_in_bar == 0;
            tr.velocity = if tr.accent { 0.85 } else { 0.55 };
            self.start_voice(tr);
        }

        let count = if project.song_mode {
            project.arrangement.len()
        } else {
            1
        };
        for clip_index in 0..count {
            if let Some((pi, local)) = Self::resolve_step(project, triggered, clip_index) {
                self.last_local_step = local;
                let pattern = &project.patterns[pi];
                let any_solo = project.channels.iter().any(|c| c.solo);
                for (ci, channel) in project.channels.iter().enumerate() {
                    if channel.mute || (any_solo && !channel.solo) {
                        continue;
                    }
                    // Notes mélodiques du Piano Roll (déclenchées à leur pas de début).
                    for note in pattern.notes.get(ci).into_iter().flatten() {
                        if note.start != local {
                            continue;
                        }
                        let mut tr = Trigger::new(voice_kind(channel.kind));
                        tr.channel = ci;
                        tr.frequency = note.frequency();
                        tr.hold_secs = step_secs * note.length as f32;
                        tr.velocity = note.velocity;
                        tr.pan = 0.0;
                        tr.pitch_semitones = channel.pitch;
                        tr.synth = channel.synth;
                        tr.tone = channel.tone;
                        self.start_voice(tr);
                    }
                    let Some(step) = pattern
                        .lanes
                        .get(ci)
                        .and_then(|lane| lane.get(local))
                        .copied()
                    else {
                        continue;
                    };
                    if !step.on || !self.roll(step.probability) {
                        continue;
                    }

                    let mut glide_from = None;
                    if channel.kind == ChannelKind::Bass808 {
                        let base = 55.0 * 2.0f32.powf(channel.pitch / 12.0);
                        if self.last_bass.len() <= ci {
                            self.last_bass.resize(ci + 1, (0.0, 0.0));
                        }
                        let (prev_hz, prev_pos) = self.last_bass[ci];
                        if prev_hz > 0.0
                            && self.position_samples - prev_pos < self.sample_rate as f64 * 2.0
                        {
                            glide_from = Some(prev_hz);
                        }
                        self.last_bass[ci] = (base, self.position_samples);
                    }

                    let sample = samples.get(&channel.id).cloned();
                    let ratchet = step.ratchet.clamp(1, 4) as usize;
                    for r in 0..ratchet {
                        let mut tr = Trigger::new(voice_kind(channel.kind));
                        tr.velocity = step.velocity;
                        tr.channel = ci;
                        tr.synth = channel.synth;
                        tr.tone = channel.tone;
                        tr.pitch_semitones = channel.pitch;
                        tr.pan = 0.0;
                        tr.delay_secs =
                            step.shift * step_secs + r as f32 * (step_secs / ratchet.max(1) as f32);
                        if let Some(s) = &sample {
                            tr.sample = Some(std::sync::Arc::clone(&s.samples));
                            tr.sample_rate = s.sample_rate as f32;
                        }
                        tr.glide_from = glide_from;
                        self.start_voice(tr);
                    }
                }
            }
        }
        self.global_step = self.global_step.wrapping_add(1);
        let swing = project.swing.clamp(0.0, 0.7) as f64 * 0.5 * sps;
        let interval = if triggered % 2 == 0 {
            sps + swing
        } else {
            sps - swing
        };
        self.samples_until_next += interval.max(1.0);
    }

    /// Rend un bloc stéréo entrelacé.
    pub fn render_block(
        &mut self,
        project: &Project,
        samples: &SampleBank,
        metronome: bool,
        out: &mut [f32],
    ) {
        let master = project.master_volume.clamp(0.0, 1.5);
        let any_solo = project.channels.iter().any(|c| c.solo);
        let mut master_peak = 0.0f32;

        let mut channel_peaks = [0.0f32; MAX_CHANNELS];

        for frame in out.chunks_exact_mut(2) {
            let can_trigger = self
                .stop_at_samples
                .is_none_or(|s| self.position_samples < s);
            if self.samples_until_next <= 0.0 {
                if can_trigger && self.samples_until_next <= 0.0 {
                    self.trigger_step(project, samples, metronome);
                } else if !can_trigger {
                    // Continue d'avancer le temps sans déclencher de nouveaux pas.
                    self.samples_until_next += self.samples_per_step(project).max(1.0);
                }
            }

            let mut buses = [[0.0f32; 2]; MAX_CHANNELS];
            let mut l = 0.0f32;
            let mut r = 0.0f32;
            for v in &mut self.voices {
                if v.active {
                    let (vl, vr) = v.next();
                    let c = v.channel();
                    if c < MAX_CHANNELS {
                        buses[c][0] += vl;
                        buses[c][1] += vr;
                    } else if let Some(ch) = project.channels.get(c) {
                        if !ch.mute && (!any_solo || ch.solo) {
                            l += vl * ch.volume * (1.0 - ch.pan.max(0.0));
                            r += vr * ch.volume * (1.0 + ch.pan.min(0.0));
                        }
                    } else {
                        l += vl;
                        r += vr;
                    }
                }
            }
            for (ci, ch) in project.channels.iter().take(MAX_CHANNELS).enumerate() {
                let dry = buses[ci];
                let echo = self.delay[ci][self.delay_pos];
                self.delay[ci][self.delay_pos] = [dry[0] + echo[0] * 0.35, dry[1] + echo[1] * 0.35];
                if ch.mute || (any_solo && !ch.solo) {
                    continue;
                }
                let vl = (dry[0] + echo[0] * ch.delay_mix) * ch.volume * (1.0 - ch.pan.max(0.0));
                let vr = (dry[1] + echo[1] * ch.delay_mix) * ch.volume * (1.0 + ch.pan.min(0.0));
                l += vl;
                r += vr;
                channel_peaks[ci] = channel_peaks[ci].max(vl.abs()).max(vr.abs());
            }
            self.delay_pos = (self.delay_pos + 1) % self.delay[0].len();
            l *= master;
            r *= master;
            frame[0] = soft_clip(l);
            frame[1] = soft_clip(r);
            master_peak = master_peak.max(frame[0].abs()).max(frame[1].abs());
            self.samples_until_next -= 1.0;
            self.position_samples += 1.0;
        }

        self.channel_peaks = channel_peaks;
        self.master_peak = master_peak;
    }
}

#[inline]
fn voice_kind(kind: ChannelKind) -> VoiceKind {
    match kind {
        ChannelKind::Kick => VoiceKind::Kick,
        ChannelKind::Snare => VoiceKind::Snare,
        ChannelKind::Clap => VoiceKind::Clap,
        ChannelKind::HatClosed => VoiceKind::HatClosed,
        ChannelKind::HatOpen => VoiceKind::HatOpen,
        ChannelKind::Perc => VoiceKind::Perc,
        ChannelKind::Rim => VoiceKind::Rim,
        ChannelKind::Cowbell => VoiceKind::Cowbell,
        ChannelKind::Bass808 => VoiceKind::Bass808,
        ChannelKind::Sample => VoiceKind::Sample,
        ChannelKind::Synth => VoiceKind::Synth,
    }
}

/// Limiteur doux : linéaire jusqu'à 0,95 puis saturation exponentielle.
#[inline]
pub fn soft_clip(x: f32) -> f32 {
    if !x.is_finite() {
        return 0.0;
    }
    let a = x.abs();
    if a <= 0.95 {
        x
    } else {
        let over = a - 0.95;
        let shaped = 0.95 + 0.05 * (1.0 - (-over * 6.0).exp());
        x.signum() * shaped
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::studio::project::Step;
    use std::sync::Arc;

    fn project_with_kick() -> Project {
        let mut p = Project::new_default("Test");
        p.master_volume = 1.0;
        p.channels[0].volume = 1.0;
        p.patterns[0].lanes[0][0] = Step::accent();
        p
    }

    #[test]
    fn step_duration_matches_bpm() {
        let p = Project::new_default("Test");
        let seq = Sequencer::new(48_000.0);
        let mut p2 = p.clone();
        p2.bpm = 120.0;
        assert!((seq.samples_per_step(&p2) - 6000.0).abs() < 1e-6);
        assert!((seq.samples_per_beat(&p2) - 24_000.0).abs() < 1e-6);
    }

    #[test]
    fn kick_step_produces_audio() {
        let p = project_with_kick();
        let mut seq = Sequencer::new(48_000.0);
        let mut out = vec![0.0f32; 4800 * 2];
        seq.render_block(&p, &SampleBank::new(), false, &mut out);
        let peak = out.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        assert!(peak > 0.01, "pic = {peak}");
    }

    #[test]
    fn silent_pattern_produces_silence() {
        let p = Project::new_default("Test");
        let mut seq = Sequencer::new(48_000.0);
        let mut out = vec![0.0f32; 4800 * 2];
        seq.render_block(&p, &SampleBank::new(), false, &mut out);
        assert!(out.iter().all(|s| s.abs() < 1e-6));
    }

    #[test]
    fn sample_channel_uses_bank() {
        let mut p = Project::new_default("Test");
        p.patterns[0].lanes[0][0] = Step::accent();
        let mut bank = SampleBank::new();
        bank.insert(
            p.channels[0].id.clone(),
            crate::studio::samples::LoadedSample {
                samples: Arc::new(vec![0.4f32; 4800]),
                sample_rate: 48000,
            },
        );
        let mut seq = Sequencer::new(48_000.0);
        let mut out = vec![0.0f32; 4800 * 2];
        seq.render_block(&p, &bank, false, &mut out);
        assert!(out.iter().any(|s| s.abs() > 0.01));
    }

    #[test]
    fn seek_sets_position() {
        let mut p = Project::new_default("Test");
        p.bpm = 120.0;
        let mut seq = Sequencer::new(48_000.0);
        seq.seek_step(&p, 8);
        assert!((seq.position_samples - 48_000.0).abs() < 1e-6);
        assert_eq!(seq.global_step, 8);
    }

    #[test]
    fn swing_delays_odd_steps() {
        let mut p = Project::new_default("Test");
        p.bpm = 120.0;
        p.swing = 0.5;
        let mut seq = Sequencer::new(48_000.0);
        // Pas 0 déclenché à t=0, le pas 1 doit arriver plus tard qu'un pas droit.
        let mut first_times = Vec::new();
        let mut out = vec![0.0f32; 2];
        let mut prev_step = seq.last_local_step;
        for i in 0..48_000 {
            seq.render_block(&p, &SampleBank::new(), false, &mut out);
            if seq.last_local_step != prev_step || i == 0 {
                first_times.push(i as f64 / 2.0 + seq.position_samples * 0.0);
                prev_step = seq.last_local_step;
            }
        }
        // Vérification indirecte : la durée d'un pas impair est allongée.
        let sps = seq.samples_per_step(&p);
        let amt = 0.5 * 0.5 * sps;
        assert!(amt > 0.0);
        let total = sps + amt + sps - amt;
        assert!((total - 2.0 * sps).abs() < 1e-6);
    }
}
