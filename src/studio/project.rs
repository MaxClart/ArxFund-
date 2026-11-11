//! Modèle de projet du studio (séquenceur pas à pas).
//!
//! Le projet est sérialisé en JSON dans un fichier `.rmsproj`. Toutes les
//! structures sont versionnées implicitement par `serde(default)` afin de
//! rester compatibles avec les évolutions des phases suivantes.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

pub const STEPS_PER_BEAT: usize = 4;
pub const MIN_STEPS: usize = 16;
pub const MAX_STEPS: usize = 64;
pub const MIN_BPM: f64 = 40.0;
pub const MAX_BPM: f64 = 300.0;

/// Instrument d'un canal du Channel Rack.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChannelKind {
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
}

impl ChannelKind {
    pub const ALL: [ChannelKind; 11] = [
        ChannelKind::Kick,
        ChannelKind::Snare,
        ChannelKind::Clap,
        ChannelKind::HatClosed,
        ChannelKind::HatOpen,
        ChannelKind::Perc,
        ChannelKind::Rim,
        ChannelKind::Cowbell,
        ChannelKind::Bass808,
        ChannelKind::Sample,
        ChannelKind::Synth,
    ];

    /// Les canaux pour lesquels le Piano Roll est pertinent.
    pub fn is_melodic(self) -> bool {
        matches!(self, ChannelKind::Bass808 | ChannelKind::Synth)
    }

    pub fn label(self) -> &'static str {
        match self {
            ChannelKind::Kick => "Kick",
            ChannelKind::Snare => "Snare",
            ChannelKind::Clap => "Clap",
            ChannelKind::HatClosed => "Charley fermé",
            ChannelKind::HatOpen => "Charley ouvert",
            ChannelKind::Perc => "Percussion",
            ChannelKind::Rim => "Rimshot",
            ChannelKind::Cowbell => "Cowbell",
            ChannelKind::Bass808 => "Basse 808",
            ChannelKind::Sample => "Échantillon",
            ChannelKind::Synth => "Synthé",
        }
    }

    pub fn default_color(self) -> [u8; 3] {
        match self {
            ChannelKind::Kick => [248, 113, 113],
            ChannelKind::Snare => [251, 191, 36],
            ChannelKind::Clap => [244, 114, 182],
            ChannelKind::HatClosed => [96, 165, 250],
            ChannelKind::HatOpen => [125, 211, 252],
            ChannelKind::Perc => [74, 222, 128],
            ChannelKind::Rim => [250, 204, 21],
            ChannelKind::Cowbell => [167, 139, 250],
            ChannelKind::Bass808 => [139, 92, 246],
            ChannelKind::Sample => [148, 163, 184],
            ChannelKind::Synth => [56, 189, 248],
        }
    }

    pub fn short_label(self) -> &'static str {
        match self {
            ChannelKind::Kick => "KICK",
            ChannelKind::Snare => "SNR",
            ChannelKind::Clap => "CLAP",
            ChannelKind::HatClosed => "HH",
            ChannelKind::HatOpen => "OH",
            ChannelKind::Perc => "PERC",
            ChannelKind::Rim => "RIM",
            ChannelKind::Cowbell => "CB",
            ChannelKind::Bass808 => "808",
            ChannelKind::Sample => "SMP",
            ChannelKind::Synth => "SYN",
        }
    }
}

/// Un pas du séquenceur.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Step {
    pub on: bool,
    pub velocity: f32,
    /// Probabilité de déclenchement (0..1).
    pub probability: f32,
    /// Décalage du pas, en fraction de pas (0..0.9).
    pub shift: f32,
    /// Nombre de frappes dans le pas (1..4).
    pub ratchet: u8,
}

impl Default for Step {
    fn default() -> Self {
        Self {
            on: false,
            velocity: 0.8,
            probability: 1.0,
            shift: 0.0,
            ratchet: 1,
        }
    }
}

impl Step {
    pub fn accent() -> Self {
        Self {
            on: true,
            velocity: 1.0,
            ..Self::default()
        }
    }

    pub fn sanitize(&mut self) {
        self.velocity = self.velocity.clamp(0.0, 1.0);
        self.probability = self.probability.clamp(0.0, 1.0);
        self.shift = self.shift.clamp(0.0, 0.9);
        self.ratchet = self.ratchet.clamp(1, 4);
    }
}

/// Une note du Piano Roll, exprimée en numéro MIDI.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Note {
    /// Hauteur MIDI (0..127). 69 = La 440 Hz.
    pub pitch: u8,
    /// Début en pas depuis le début du pattern.
    pub start: usize,
    /// Durée en pas (>= 1).
    pub length: usize,
    /// Vélocité (0..1).
    pub velocity: f32,
}

impl Default for Note {
    fn default() -> Self {
        Self {
            pitch: 60,
            start: 0,
            length: 1,
            velocity: 0.85,
        }
    }
}

impl Note {
    pub fn sanitize(&mut self) {
        self.pitch = self.pitch.min(127);
        self.length = self.length.clamp(1, 64);
        self.velocity = self.velocity.clamp(0.0, 1.0);
        self.start = self.start.min(MAX_STEPS);
    }

    /// Fréquence en hertz pour une hauteur MIDI.
    pub fn frequency(&self) -> f32 {
        440.0 * 2.0f32.powf((self.pitch as f32 - 69.0) / 12.0)
    }
}

/// Paramètres du synthétiseur mélodique (ADSR + forme d'onde).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct SynthParams {
    /// 0 = sinus, 1 = dent de scie, 2 = carré, 3 = triangle.
    pub waveform: u8,
    pub attack: f32,
    pub decay: f32,
    pub sustain: f32,
    pub release: f32,
    /// Glide en secondes entre deux notes (0 = désactivé).
    pub glide: f32,
}

impl Default for SynthParams {
    fn default() -> Self {
        Self {
            waveform: 1,
            attack: 0.01,
            decay: 0.16,
            sustain: 0.72,
            release: 0.22,
            glide: 0.0,
        }
    }
}

impl SynthParams {
    pub fn sanitize(&mut self) {
        self.waveform = self.waveform.min(3);
        self.attack = self.attack.clamp(0.001, 4.0);
        self.decay = self.decay.clamp(0.001, 4.0);
        self.sustain = self.sustain.clamp(0.0, 1.0);
        self.release = self.release.clamp(0.005, 6.0);
        self.glide = self.glide.clamp(0.0, 2.0);
    }
}

fn default_tone() -> f32 {
    1.0
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Channel {
    pub id: String,
    pub name: String,
    pub kind: ChannelKind,
    pub color: [u8; 3],
    pub volume: f32,
    pub pan: f32,
    pub pitch: f32,
    pub mute: bool,
    pub solo: bool,
    pub mixer_track: usize,
    #[serde(default)]
    pub sample_path: Option<String>,
    /// Filtre passe-bas de canal : 0 = fermé, 1 = ouvert.
    #[serde(default = "default_tone")]
    pub tone: f32,
    #[serde(default)]
    pub synth: SynthParams,
    /// Mix d’un écho stéréo de 300 ms (0 = bypass).
    #[serde(default)]
    pub delay_mix: f32,
}

impl Channel {
    pub fn new(kind: ChannelKind, name: Option<&str>, id: String) -> Self {
        Self {
            id,
            name: name.unwrap_or(kind.label()).to_string(),
            kind,
            color: kind.default_color(),
            volume: 0.8,
            pan: 0.0,
            pitch: 0.0,
            mute: false,
            solo: false,
            mixer_track: 0,
            sample_path: None,
            tone: 1.0,
            synth: SynthParams::default(),
            delay_mix: 0.0,
        }
    }

    pub fn sanitize(&mut self) {
        self.volume = self.volume.clamp(0.0, 1.5);
        self.pan = self.pan.clamp(-1.0, 1.0);
        self.pitch = self.pitch.clamp(-24.0, 24.0);
        self.tone = if self.tone.is_finite() {
            self.tone.clamp(0.0, 1.0)
        } else {
            1.0
        };
        self.synth.sanitize();
        self.delay_mix = if self.delay_mix.is_finite() {
            self.delay_mix.clamp(0.0, 0.8)
        } else {
            0.0
        };
        if self.name.trim().is_empty() {
            self.name = self.kind.label().to_string();
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Pattern {
    pub id: String,
    pub name: String,
    pub length: usize,
    /// Une lane de pas par canal (même ordre que `Project::channels`).
    pub lanes: Vec<Vec<Step>>,
    /// Une lane de notes par canal, utilisée par le Piano Roll.
    #[serde(default)]
    pub notes: Vec<Vec<Note>>,
}

impl Pattern {
    pub fn new(name: &str, length: usize, channels: usize) -> Self {
        Self {
            id: unique_id("pat"),
            name: name.to_string(),
            length,
            lanes: vec![vec![Step::default(); length]; channels],
            notes: vec![Vec::new(); channels],
        }
    }

    pub fn ensure(&mut self, length: usize, channels: usize) {
        self.length = length;
        self.lanes.resize_with(channels, Vec::new);
        for lane in &mut self.lanes {
            lane.resize(length, Step::default());
            for s in lane.iter_mut() {
                s.sanitize();
            }
        }
        self.notes.resize_with(channels, Vec::new);
        for lane in &mut self.notes {
            lane.retain(|n| n.start < self.length.max(1));
            for note in lane.iter_mut() {
                note.sanitize();
                note.length = note.length.min(self.length.max(1) - note.start);
            }
        }
    }
}

/// Un clip de l'arrangement (Mode Song). L'édition complète arrive en Phase 4.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Clip {
    pub pattern: usize,
    pub start_step: usize,
    pub length_steps: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub bpm: f64,
    pub time_sig_num: u8,
    pub time_sig_den: u8,
    pub steps: usize,
    pub swing: f32,
    pub master_volume: f32,
    pub channels: Vec<Channel>,
    pub patterns: Vec<Pattern>,
    pub active_pattern: usize,
    pub song_mode: bool,
    pub arrangement: Vec<Clip>,
    pub created_at: i64,
    pub modified_at: i64,
    #[serde(default)]
    pub next_pattern_number: u32,
}

impl Default for Project {
    fn default() -> Self {
        Self::new_default("Nouveau projet")
    }
}

impl Project {
    /// Projet de démonstration : 4 pistes prêtes à être programmées.
    pub fn new_default(name: &str) -> Self {
        let now = now_ms();
        let mut project = Self {
            id: unique_id("proj"),
            name: name.to_string(),
            bpm: 140.0,
            time_sig_num: 4,
            time_sig_den: 4,
            steps: 16,
            swing: 0.0,
            master_volume: 0.85,
            channels: Vec::new(),
            patterns: Vec::new(),
            active_pattern: 0,
            song_mode: false,
            arrangement: Vec::new(),
            created_at: now,
            modified_at: now,
            next_pattern_number: 2,
        };
        for kind in [
            ChannelKind::Kick,
            ChannelKind::Clap,
            ChannelKind::HatClosed,
            ChannelKind::Bass808,
            ChannelKind::Synth,
        ] {
            project.add_channel(kind, None);
        }
        project.patterns.push(Pattern::new(
            "Pattern 1",
            project.steps,
            project.channels.len(),
        ));
        project.arrangement.push(Clip {
            pattern: 0,
            start_step: 0,
            length_steps: project.steps,
        });
        project
    }

    /// Garantit les invariants du projet après un chargement ou une édition.
    pub fn ensure_invariants(&mut self) {
        self.bpm = if self.bpm.is_finite() {
            self.bpm.clamp(MIN_BPM, MAX_BPM)
        } else {
            140.0
        };
        self.swing = if self.swing.is_finite() {
            self.swing.clamp(0.0, 0.7)
        } else {
            0.0
        };
        self.master_volume = if self.master_volume.is_finite() {
            self.master_volume.clamp(0.0, 1.5)
        } else {
            0.85
        };
        self.steps = normalize_steps(self.steps);
        self.time_sig_num = self.time_sig_num.clamp(1, 16);
        if !matches!(self.time_sig_den, 2 | 4 | 8 | 16) {
            self.time_sig_den = 4;
        }
        if self.name.trim().is_empty() {
            self.name = "Nouveau projet".to_string();
        }

        if self.channels.is_empty() {
            self.add_channel(ChannelKind::Kick, None);
        }
        for (i, c) in self.channels.iter_mut().enumerate() {
            c.sanitize();
            if c.id.trim().is_empty() {
                c.id = unique_id("ch");
            }
            c.mixer_track = i;
        }

        if self.patterns.is_empty() {
            self.patterns
                .push(Pattern::new("Pattern 1", self.steps, self.channels.len()));
            self.next_pattern_number = 2;
        }
        for p in &mut self.patterns {
            p.ensure(self.steps, self.channels.len());
            if p.name.trim().is_empty() {
                p.name = "Pattern".to_string();
            }
        }
        if self.active_pattern >= self.patterns.len() {
            self.active_pattern = 0;
        }

        for clip in &mut self.arrangement {
            if clip.pattern >= self.patterns.len() {
                clip.pattern = 0;
            }
            clip.length_steps = clip.length_steps.max(1);
        }
        let max_len = self.song_length_steps();
        for clip in &mut self.arrangement {
            clip.start_step = clip.start_step.min(max_len);
        }
        if self.arrangement.is_empty() {
            self.arrangement.push(Clip {
                pattern: self.active_pattern,
                start_step: 0,
                length_steps: self.steps,
            });
        }
        self.modified_at = now_ms();
    }

    pub fn active_pattern_ref(&self) -> &Pattern {
        let i = self
            .active_pattern
            .min(self.patterns.len().saturating_sub(1));
        &self.patterns[i]
    }

    pub fn active_pattern_mut(&mut self) -> &mut Pattern {
        let i = self
            .active_pattern
            .min(self.patterns.len().saturating_sub(1));
        &mut self.patterns[i]
    }

    pub fn bar_steps(&self) -> usize {
        STEPS_PER_BEAT * self.time_sig_num as usize
    }

    pub fn song_length_steps(&self) -> usize {
        self.arrangement
            .iter()
            .map(|c| c.start_step + c.length_steps)
            .max()
            .unwrap_or(self.steps)
            .max(1)
    }

    pub fn add_channel(&mut self, kind: ChannelKind, name: Option<&str>) -> usize {
        let id = unique_id("ch");
        let index = self.channels.len();
        let mut channel = Channel::new(kind, name, id);
        channel.mixer_track = index;
        self.channels.push(channel);
        for p in &mut self.patterns {
            p.lanes.push(vec![Step::default(); p.length]);
            p.notes.push(Vec::new());
        }
        index
    }

    pub fn remove_channel(&mut self, index: usize) {
        if index >= self.channels.len() || self.channels.len() <= 1 {
            return;
        }
        self.channels.remove(index);
        for p in &mut self.patterns {
            if index < p.lanes.len() {
                p.lanes.remove(index);
            }
            if index < p.notes.len() {
                p.notes.remove(index);
            }
        }
    }

    pub fn resize_steps(&mut self, steps: usize) {
        self.steps = normalize_steps(steps);
        for p in &mut self.patterns {
            p.ensure(self.steps, self.channels.len());
        }
        for clip in &mut self.arrangement {
            clip.length_steps = clip.length_steps.min(self.steps);
        }
    }

    pub fn add_pattern(&mut self) -> usize {
        let name = format!("Pattern {}", self.next_pattern_number.max(1));
        self.next_pattern_number = self.next_pattern_number.saturating_add(1);
        let p = Pattern::new(&name, self.steps, self.channels.len());
        self.patterns.push(p);
        self.patterns.len() - 1
    }

    pub fn duplicate_pattern(&mut self, index: usize) -> Option<usize> {
        let original = self.patterns.get(index)?.clone();
        let mut copy = original;
        copy.id = unique_id("pat");
        copy.name = format!("{} copie", copy.name);
        self.patterns.push(copy);
        Some(self.patterns.len() - 1)
    }

    pub fn delete_pattern(&mut self, index: usize) {
        if self.patterns.len() <= 1 || index >= self.patterns.len() {
            return;
        }
        self.patterns.remove(index);
        for clip in &mut self.arrangement {
            if clip.pattern == index {
                clip.pattern = 0;
            } else if clip.pattern > index {
                clip.pattern -= 1;
            }
        }
        if self.active_pattern >= self.patterns.len() {
            self.active_pattern = self.patterns.len() - 1;
        }
    }

    pub fn step_mut(&mut self, channel: usize, step: usize) -> Option<&mut Step> {
        let pattern = self.active_pattern_mut();
        pattern.lanes.get_mut(channel)?.get_mut(step)
    }

    pub fn step(&self, channel: usize, step: usize) -> Option<Step> {
        self.active_pattern_ref()
            .lanes
            .get(channel)?
            .get(step)
            .copied()
    }

    /// Notes du canal dans le pattern actif.
    pub fn notes(&self, channel: usize) -> &[Note] {
        self.active_pattern_ref()
            .notes
            .get(channel)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    pub fn notes_mut(&mut self, channel: usize) -> &mut Vec<Note> {
        let pattern = self.active_pattern_mut();
        if channel >= pattern.notes.len() {
            pattern.notes.resize_with(channel + 1, Vec::new);
        }
        &mut pattern.notes[channel]
    }

    pub fn save_to(&self, path: &Path) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let tmp = path.with_extension("rmsproj.tmp");
        fs::write(&tmp, json).map_err(|e| e.to_string())?;
        fs::rename(&tmp, path).map_err(|e| e.to_string())
    }

    pub fn load_from(path: &Path) -> Result<Self, String> {
        let data = fs::read_to_string(path).map_err(|e| e.to_string())?;
        let mut project: Project = serde_json::from_str(&data).map_err(|e| e.to_string())?;
        project.ensure_invariants();
        Ok(project)
    }

    /// Nom de fichier proposé pour l'export.
    pub fn export_file_stem(&self) -> String {
        let mut stem = String::new();
        for ch in self.name.chars() {
            if ch.is_alphanumeric() {
                stem.push(ch);
            } else if (ch == ' ' || ch == '-' || ch == '_') && !stem.ends_with('_') {
                stem.push('_');
            }
        }
        let stem = stem.trim_matches('_');
        if stem.is_empty() {
            format!("projet_{}", self.bpm.round() as i64)
        } else {
            format!("{}_{}bpm", stem, self.bpm.round() as i64)
        }
    }
}

#[derive(Clone, Debug)]
pub struct ProjectSummary {
    pub path: PathBuf,
    pub name: String,
    pub bpm: f64,
    pub modified_at: i64,
    pub is_autosave: bool,
}

/// Liste les projets d'un dossier, du plus récent au plus ancien.
pub fn scan_projects(dir: &Path) -> Vec<ProjectSummary> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("rmsproj") {
            continue;
        }
        let Ok(data) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&data) else {
            continue;
        };
        let name = value
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("Projet")
            .to_string();
        let bpm = value.get("bpm").and_then(|v| v.as_f64()).unwrap_or(120.0);
        let is_autosave = path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.contains(".autosave"))
            .unwrap_or(false);
        out.push(ProjectSummary {
            modified_at: value
                .get("modified_at")
                .and_then(|v| v.as_i64())
                .unwrap_or(0),
            path,
            name,
            bpm,
            is_autosave,
        });
    }
    out.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));
    out
}

pub fn normalize_steps(steps: usize) -> usize {
    let mut steps = steps.clamp(MIN_STEPS, MAX_STEPS);
    steps = (steps / MIN_STEPS) * MIN_STEPS;
    steps.clamp(MIN_STEPS, MAX_STEPS)
}

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub fn unique_id(prefix: &str) -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{prefix}-{t:x}-{n:x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_project_has_channels_and_one_pattern() {
        let p = Project::new_default("Test");
        assert_eq!(p.channels.len(), 5);
        assert_eq!(p.patterns.len(), 1);
        assert_eq!(p.steps, 16);
        assert_eq!(p.patterns[0].lanes.len(), 5);
        assert_eq!(p.patterns[0].notes.len(), 5);
        assert_eq!(p.patterns[0].lanes[0].len(), 16);
        assert_eq!(p.arrangement.len(), 1);
    }

    #[test]
    fn notes_survive_serde_and_resize() {
        let mut p = Project::new_default("Test");
        p.notes_mut(4).push(Note {
            pitch: 64,
            start: 2,
            length: 4,
            velocity: 0.7,
        });
        let json = serde_json::to_string(&p).unwrap();
        let mut back: Project = serde_json::from_str(&json).unwrap();
        back.ensure_invariants();
        assert_eq!(back.notes(4).len(), 1);
        assert_eq!(back.notes(4)[0].pitch, 64);
        back.resize_steps(16);
        assert_eq!(back.notes(4)[0].length, 4);
    }

    #[test]
    fn resize_steps_keeps_lanes_aligned() {
        let mut p = Project::new_default("Test");
        p.resize_steps(64);
        assert_eq!(p.steps, 64);
        for pattern in &p.patterns {
            assert_eq!(pattern.length, 64);
            assert_eq!(pattern.lanes.len(), p.channels.len());
            for lane in &pattern.lanes {
                assert_eq!(lane.len(), 64);
            }
        }
    }

    #[test]
    fn add_and_remove_channel_keeps_lanes_aligned() {
        let mut p = Project::new_default("Test");
        p.add_channel(ChannelKind::Snare, Some("Snare 2"));
        assert_eq!(p.channels.len(), 6);
        for pattern in &p.patterns {
            assert_eq!(pattern.lanes.len(), 6);
            assert_eq!(pattern.notes.len(), 6);
        }
        p.remove_channel(1);
        assert_eq!(p.channels.len(), 5);
        for pattern in &p.patterns {
            assert_eq!(pattern.lanes.len(), 5);
            assert_eq!(pattern.notes.len(), 5);
        }
    }

    #[test]
    fn serde_roundtrip() {
        let mut p = Project::new_default("Roundtrip");
        p.bpm = 174.0;
        p.patterns[0].lanes[0][0] = Step::accent();
        p.patterns[0].lanes[0][4] = Step {
            on: true,
            velocity: 0.55,
            ..Step::default()
        };
        let json = serde_json::to_string(&p).unwrap();
        let mut back: Project = serde_json::from_str(&json).unwrap();
        back.ensure_invariants();
        assert_eq!(back.bpm, 174.0);
        assert!(back.patterns[0].lanes[0][0].on);
        assert_eq!(back.patterns[0].lanes[0][4].velocity, 0.55);
    }

    #[test]
    fn ensure_invariants_repairs_broken_project() {
        let mut p = Project::new_default("Test");
        p.steps = 999;
        p.bpm = -12.0;
        p.active_pattern = 42;
        p.patterns[0].lanes.truncate(1);
        p.ensure_invariants();
        assert_eq!(p.steps, 64);
        assert_eq!(p.bpm, MIN_BPM);
        assert_eq!(p.active_pattern, 0);
        assert_eq!(p.patterns[0].lanes.len(), p.channels.len());
    }
}
