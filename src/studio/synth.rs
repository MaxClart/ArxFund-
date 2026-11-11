//! Synthèse des instruments de batterie du Channel Rack.
//!
//! Tous les sons sont générés proceduralement : aucune banque d'échantillons
//! propriétaire n'est utilisée. Les voix sont pré-allouées par le séquenceur
//! afin de ne jamais allouer pendant le rendu audio.

use std::sync::Arc;

use crate::studio::project::SynthParams;

const TAU: f32 = std::f32::consts::TAU;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VoiceKind {
    Kick,
    Snare,
    Clap,
    HatClosed,
    HatOpen,
    Perc,
    Rim,
    Cowbell,
    Bass808,
    Sample,
    Synth,
    /// Clic de métronome (interne, non exposé dans le Channel Rack).
    Click,
}

/// Paramètres d'un déclenchement de voix.
#[derive(Clone)]
pub struct Trigger {
    pub kind: VoiceKind,
    pub velocity: f32,
    pub pitch_semitones: f32,
    pub pan: f32,
    pub delay_secs: f32,
    pub sample: Option<Arc<Vec<f32>>>,
    pub sample_rate: f32,
    pub accent: bool,
    pub glide_from: Option<f32>,
    /// Index du canal, pour les VU-mètres par tranche.
    pub channel: usize,
    /// Fréquence exacte de la note (0 = utiliser la hauteur du canal).
    pub frequency: f32,
    /// Durée de maintien de la note en secondes (0 = one-shot).
    pub hold_secs: f32,
    /// Paramètres ADSR du synthé.
    pub synth: SynthParams,
    /// Filtre passe-bas de canal (0 = fermé, 1 = ouvert).
    pub tone: f32,
}

impl Trigger {
    pub fn new(kind: VoiceKind) -> Self {
        Self {
            kind,
            velocity: 0.8,
            pitch_semitones: 0.0,
            pan: 0.0,
            delay_secs: 0.0,
            sample: None,
            sample_rate: 44100.0,
            accent: false,
            glide_from: None,
            channel: 0,
            frequency: 0.0,
            hold_secs: 0.0,
            synth: SynthParams::default(),
            tone: 1.0,
        }
    }
}

/// Une voix de synthèse. Les champs sont privés : le séquenceur ne manipule
/// que `active` et `next`.
pub struct Voice {
    pub active: bool,
    kind: VoiceKind,
    out_rate: f32,
    t: f32,
    delay: f32,
    phase: f32,
    phase2: f32,
    noise: u32,
    prev_noise: f32,
    velocity: f32,
    pitch_mult: f32,
    pan_l: f32,
    pan_r: f32,
    sample: Option<Arc<Vec<f32>>>,
    sample_pos: f64,
    src_rate: f32,
    glide_from: f32,
    accent: bool,
    channel: usize,
    frequency: f32,
    hold: f32,
    env_attack: f32,
    env_decay: f32,
    env_sustain: f32,
    env_release: f32,
    waveform: u8,
    glide_time: f32,
    tone: f32,
    filter: f32,
}

impl Voice {
    pub fn silent() -> Self {
        Self {
            active: false,
            kind: VoiceKind::Click,
            out_rate: 48000.0,
            t: 0.0,
            delay: 0.0,
            phase: 0.0,
            phase2: 0.0,
            noise: 0x1234_5678,
            prev_noise: 0.0,
            velocity: 0.0,
            pitch_mult: 1.0,
            pan_l: 0.707,
            pan_r: 0.707,
            sample: None,
            sample_pos: 0.0,
            src_rate: 44100.0,
            glide_from: 0.0,
            accent: false,
            channel: 0,
            frequency: 0.0,
            hold: 0.0,
            env_attack: 0.01,
            env_decay: 0.16,
            env_sustain: 0.72,
            env_release: 0.22,
            waveform: 1,
            glide_time: 0.0,
            tone: 1.0,
            filter: 0.0,
        }
    }

    pub fn channel(&self) -> usize {
        self.channel
    }

    pub fn start(&mut self, tr: &Trigger, out_rate: f32) {
        self.active = true;
        self.kind = tr.kind;
        self.out_rate = out_rate.max(1.0);
        self.t = 0.0;
        self.delay = tr.delay_secs.max(0.0);
        self.phase = 0.0;
        self.phase2 = 0.0;
        self.prev_noise = 0.0;
        self.velocity = tr.velocity.clamp(0.0, 1.5);
        self.pitch_mult = 2.0f32.powf(tr.pitch_semitones.clamp(-36.0, 36.0) / 12.0);
        let pan = tr.pan.clamp(-1.0, 1.0);
        let angle = (pan + 1.0) * std::f32::consts::FRAC_PI_4;
        self.pan_l = angle.cos();
        self.pan_r = angle.sin();
        self.sample = tr.sample.clone();
        self.sample_pos = 0.0;
        self.src_rate = tr.sample_rate.max(1.0);
        self.glide_from = tr.glide_from.unwrap_or(0.0).max(0.0);
        self.accent = tr.accent;
        self.channel = tr.channel;
        self.frequency = tr.frequency.max(0.0);
        self.hold = tr.hold_secs.max(0.0);
        self.env_attack = tr.synth.attack.max(0.001);
        self.env_decay = tr.synth.decay.max(0.001);
        self.env_sustain = tr.synth.sustain.clamp(0.0, 1.0);
        self.env_release = tr.synth.release.max(0.005);
        self.waveform = tr.synth.waveform.min(3);
        self.glide_time = tr.synth.glide.max(0.0);
        self.tone = tr.tone.clamp(0.0, 1.0);
        self.filter = 0.0;
        self.noise = 0x9E37_79B9
            ^ (self.pitch_mult.to_bits().rotate_left(7))
            ^ ((tr.velocity * 1000.0) as u32).wrapping_mul(2_654_435_761);
        if self.noise == 0 {
            self.noise = 0x1234_5678;
        }
    }

    #[inline]
    fn next_noise(&mut self) -> f32 {
        let mut x = self.noise;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.noise = x;
        (x as f32 / u32::MAX as f32) * 2.0 - 1.0
    }

    /// Produit l'échantillon stéréo courant, puis avance la voix.
    pub fn next(&mut self) -> (f32, f32) {
        if !self.active {
            return (0.0, 0.0);
        }
        let dt = 1.0 / self.out_rate;
        if self.delay > 0.0 {
            self.delay -= dt;
            return (0.0, 0.0);
        }
        self.t += dt;

        let mut mono = match self.kind {
            VoiceKind::Kick => {
                let f = 46.0 + 120.0 * (-self.t * 30.0).exp();
                self.phase += TAU * f * dt;
                let body = self.phase.sin() * (-self.t * 6.5).exp();
                let click = self.next_noise() * (-self.t * 140.0).exp() * 0.4;
                (body + click).tanh() * self.velocity
            }
            VoiceKind::Snare => {
                let n = self.next_noise();
                let hp = n - self.prev_noise * 0.6;
                self.prev_noise = n;
                let tone = (TAU * 190.0 * self.t).sin();
                (hp * 0.75 * (-self.t * 17.0).exp() + tone * 0.45 * (-self.t * 24.0).exp())
                    * self.velocity
            }
            VoiceKind::Clap => {
                let n = self.next_noise();
                let mut env = 0.0;
                for k in 0..3 {
                    let local = self.t - k as f32 * 0.011;
                    if local >= 0.0 {
                        env += (-local * 130.0).exp();
                    }
                }
                env += (-self.t * 16.0).exp() * 0.35;
                n * env.min(1.6) * 0.55 * self.velocity
            }
            VoiceKind::HatClosed | VoiceKind::HatOpen => {
                let n = self.next_noise();
                let hp = n - self.prev_noise;
                self.prev_noise = n;
                let decay = if self.kind == VoiceKind::HatClosed {
                    115.0
                } else {
                    14.0
                };
                hp * (-self.t * decay).exp() * 0.45 * self.velocity
            }
            VoiceKind::Perc => {
                let f = 330.0 * self.pitch_mult;
                self.phase += TAU * f * dt;
                let body = self.phase.sin() * (-self.t * 26.0).exp();
                let noise = self.next_noise() * (-self.t * 55.0).exp() * 0.45;
                (body + noise) * self.velocity
            }
            VoiceKind::Rim => {
                self.phase += TAU * 1700.0 * self.pitch_mult * dt;
                let square = if self.phase.sin() >= 0.0 { 1.0 } else { -1.0 };
                let click = square * (-self.t * 95.0).exp() * 0.35;
                let noise = self.next_noise() * (-self.t * 220.0).exp() * 0.3;
                (click + noise) * self.velocity
            }
            VoiceKind::Cowbell => {
                self.phase += TAU * 540.0 * self.pitch_mult * dt;
                self.phase2 += TAU * 800.0 * self.pitch_mult * dt;
                let a = if self.phase.sin() >= 0.0 { 1.0 } else { -1.0 };
                let b = if self.phase2.sin() >= 0.0 { 1.0 } else { -1.0 };
                (a + b) * 0.18 * (-self.t * 18.0).exp() * self.velocity
            }
            VoiceKind::Bass808 => {
                let base = if self.frequency > 0.0 {
                    self.frequency
                } else {
                    55.0 * self.pitch_mult
                };
                let env_f = base * (1.0 + 1.9 * (-self.t * 28.0).exp());
                let f = if self.glide_from > 0.0 {
                    let g = (self.t / 0.06).min(1.0);
                    self.glide_from + (env_f - self.glide_from) * g
                } else {
                    env_f
                };
                self.phase += TAU * f * dt;
                let attack = 1.0 - (-self.t * 120.0).exp();
                let release = if self.hold > 0.0 && self.t > self.hold {
                    (-(self.t - self.hold) / self.env_release.max(0.01)).exp()
                } else {
                    1.0
                };
                let env = if self.hold > 0.0 {
                    attack * (0.45 + 0.55 * (-self.t * 2.2).exp()) * release
                } else {
                    attack * (-self.t * 1.35).exp()
                };
                (self.phase.sin() * 2.1).tanh() * 0.8 * env * self.velocity
            }
            VoiceKind::Synth => {
                let target = if self.frequency > 0.0 {
                    self.frequency
                } else {
                    440.0 * self.pitch_mult
                };
                let f = if self.glide_from > 0.0 && self.glide_time > 0.0 {
                    let g = (self.t / self.glide_time).min(1.0);
                    self.glide_from + (target - self.glide_from) * g
                } else {
                    target
                };
                self.phase += TAU * f * dt;
                if self.phase > TAU {
                    self.phase -= TAU;
                }
                let x = self.phase / TAU;
                let osc = match self.waveform {
                    0 => self.phase.sin(),
                    1 => 2.0 * x - 1.0,
                    2 => {
                        if x < 0.5 {
                            1.0
                        } else {
                            -1.0
                        }
                    }
                    _ => 1.0 - 4.0 * (x - 0.5).abs(),
                };
                let level = if self.t < self.env_attack {
                    self.t / self.env_attack
                } else if self.t < self.env_attack + self.env_decay {
                    let d = (self.t - self.env_attack) / self.env_decay;
                    1.0 - (1.0 - self.env_sustain) * d
                } else {
                    self.env_sustain
                };
                let release = if self.t > self.hold {
                    (-(self.t - self.hold) / self.env_release).exp()
                } else {
                    1.0
                };
                osc * level * release * 0.42 * self.velocity
            }
            VoiceKind::Sample => {
                if let Some(buf) = &self.sample {
                    let len = buf.len();
                    let i = self.sample_pos.floor() as usize;
                    if len == 0 || i + 1 >= len {
                        self.active = false;
                        return (0.0, 0.0);
                    }
                    let frac = (self.sample_pos - i as f64) as f32;
                    let x = buf[i] + (buf[i + 1] - buf[i]) * frac;
                    let step = (self.src_rate / self.out_rate) as f64 * self.pitch_mult as f64;
                    self.sample_pos += step;
                    let fade_in = (self.t / 0.002).min(1.0);
                    let remaining = (len - i) as f32 / self.src_rate.max(1.0);
                    let fade_out = (remaining / 0.005).min(1.0);
                    x * fade_in * fade_out * self.velocity
                } else {
                    self.active = false;
                    return (0.0, 0.0);
                }
            }
            VoiceKind::Click => {
                let f = if self.accent { 1568.0 } else { 1046.0 };
                self.phase += TAU * f * dt;
                self.phase.sin() * (-self.t * 70.0).exp() * 0.28
            }
        };

        // Filtre passe-bas de canal (Tone). À 1.0, le signal est inchangé.
        if self.tone < 0.999 {
            let coeff = 0.015 + self.tone * self.tone * 0.985;
            self.filter += (mono - self.filter) * coeff;
            mono = self.filter;
        }

        let dur = match self.kind {
            VoiceKind::Kick => 0.7,
            VoiceKind::Snare => 0.35,
            VoiceKind::Clap => 0.32,
            VoiceKind::HatClosed => 0.14,
            VoiceKind::HatOpen => 0.55,
            VoiceKind::Perc => 0.35,
            VoiceKind::Rim => 0.12,
            VoiceKind::Cowbell => 0.6,
            VoiceKind::Bass808 => {
                if self.hold > 0.0 {
                    self.hold + self.env_release * 6.0
                } else {
                    4.0
                }
            }
            VoiceKind::Synth => (self.hold + self.env_release * 6.0).max(0.05),
            VoiceKind::Sample => f32::MAX,
            VoiceKind::Click => 0.12,
        };
        if self.t >= dur {
            self.active = false;
        }

        let out = if mono.is_finite() {
            mono.clamp(-4.0, 4.0)
        } else {
            0.0
        };
        (out * self.pan_l, out * self.pan_r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(kind: VoiceKind, seconds: f32) -> Vec<f32> {
        let mut v = Voice::silent();
        let mut tr = Trigger::new(kind);
        tr.velocity = 1.0;
        v.start(&tr, 48000.0);
        let n = (seconds * 48000.0) as usize;
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            let (l, r) = v.next();
            out.push(l + r);
        }
        out
    }

    #[test]
    fn every_instrument_produces_finite_audio() {
        for kind in [
            VoiceKind::Kick,
            VoiceKind::Snare,
            VoiceKind::Clap,
            VoiceKind::HatClosed,
            VoiceKind::HatOpen,
            VoiceKind::Perc,
            VoiceKind::Rim,
            VoiceKind::Cowbell,
            VoiceKind::Bass808,
            VoiceKind::Click,
        ] {
            let out = render(kind, 0.5);
            assert!(out.iter().all(|s| s.is_finite()), "{kind:?} non fini");
            let peak = out.iter().fold(0.0f32, |m, s| m.max(s.abs()));
            assert!(peak > 0.001, "{kind:?} silencieux");
            assert!(peak < 4.0, "{kind:?} trop fort : {peak}");
        }
    }

    #[test]
    fn voices_stop_after_their_duration() {
        let mut v = Voice::silent();
        let mut tr = Trigger::new(VoiceKind::Rim);
        tr.velocity = 1.0;
        v.start(&tr, 48000.0);
        for _ in 0..(0.5 * 48000.0) as usize {
            v.next();
        }
        assert!(!v.active);
    }

    #[test]
    fn sample_voice_reads_the_buffer() {
        let mut v = Voice::silent();
        let buf = Arc::new(vec![0.5f32; 1000]);
        let mut tr = Trigger::new(VoiceKind::Sample);
        tr.sample = Some(buf);
        tr.sample_rate = 48000.0;
        tr.velocity = 1.0;
        v.start(&tr, 48000.0);
        let (l, _) = v.next();
        assert!(l.abs() > 0.0);
        for _ in 0..2000 {
            v.next();
        }
        assert!(!v.active);
    }
}
