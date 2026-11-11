//! État applicatif du studio : projet courant, moteur, undo/redo, export.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use crate::studio::engine::StudioEngine;
use crate::studio::export::{ExportOptions, ExportResult, render_to_wav};
use crate::studio::project::{
    ChannelKind, Project, ProjectSummary, Step, now_ms, scan_projects, unique_id,
};
use crate::studio::samples::{LoadedSample, SampleBank, decode_to_mono};
use crate::studio::ui::StudioUiState;

/// Actions renvoyées à l'application principale après le rendu de l'UI.
#[derive(Clone, Debug)]
pub enum StudioAction {
    /// Couper le lecteur de musique pour éviter deux lectures simultanées.
    PauseMusic,
    /// Importer un fichier exporté dans la bibliothèque.
    ImportToLibrary(PathBuf),
    /// Persister le mode débutant.
    PersistBeginner(bool),
}

pub enum ExportMsg {
    Progress(f32),
    Done {
        result: Result<ExportResult, String>,
        add_library: bool,
        path: PathBuf,
    },
}

pub struct StudioState {
    pub project: Project,
    pub engine: Option<StudioEngine>,
    pub engine_error: Option<String>,
    pub samples: SampleBank,
    pub project_dir: PathBuf,
    pub export_dir: PathBuf,
    pub projects: Vec<ProjectSummary>,
    pub project_path: Option<PathBuf>,
    pub dirty: bool,
    pub project_dirty: bool,
    pub last_autosave: Instant,
    pub undo: Vec<Project>,
    pub redo: Vec<Project>,
    pub beginner: bool,
    pub metronome: bool,
    pub playhead: usize,
    pub status: Option<String>,
    pub export_rx: Option<Receiver<ExportMsg>>,
    pub export_progress: f32,
    pub export_result: Option<ExportResult>,
    pub export_error: Option<String>,
    pub export_bars: usize,
    pub export_normalize: bool,
    pub export_bit_depth: u16,
    pub export_add_library: bool,
    pub recovery_available: Option<PathBuf>,
    pub drag_paint: Option<(usize, bool)>,
    pub show_help: bool,
    pub pending_channel_kind: ChannelKind,
    pub ui: StudioUiState,
}

impl StudioState {
    pub fn new(
        _base_dir: PathBuf,
        beginner: bool,
        engine: Option<StudioEngine>,
        engine_error: Option<String>,
    ) -> Self {
        // Projets et exports = contenus personnels, dans Musique\ARXFUND.
        // `_base_dir` (%APPDATA%\ARXFUND) reste réservé aux paramètres,
        // sauvegardes automatiques et pochettes.
        let project_dir = crate::paths::projects_dir();
        let export_dir = crate::paths::exports_dir();
        let _ = std::fs::create_dir_all(&project_dir);
        let _ = std::fs::create_dir_all(&export_dir);

        let projects = scan_projects(&project_dir);
        let recovery = projects
            .iter()
            .find(|p| p.is_autosave)
            .map(|p| (p.path.clone(), p.modified_at));

        let loaded = projects.iter().find(|p| !p.is_autosave).and_then(|p| {
            Project::load_from(&p.path)
                .ok()
                .map(|pr| (pr, p.path.clone()))
        });

        let (project, project_path) = match loaded {
            Some((p, path)) => (p, Some(path)),
            None => {
                let mut p = Project::new_default("Nouveau projet");
                p.ensure_invariants();
                let path = project_dir.join(format!("{}.rmsproj", p.id));
                let _ = p.save_to(&path);
                (p, Some(path))
            }
        };

        let recovery_available = recovery.and_then(|(path, modified)| {
            if modified > project.modified_at {
                Some(path)
            } else {
                None
            }
        });

        let mut state = Self {
            project,
            engine,
            engine_error,
            samples: SampleBank::new(),
            project_dir,
            export_dir,
            projects: Vec::new(),
            project_path,
            dirty: false,
            project_dirty: true,
            last_autosave: Instant::now(),
            undo: Vec::new(),
            redo: Vec::new(),
            beginner,
            metronome: false,
            playhead: 0,
            status: None,
            export_rx: None,
            export_progress: 0.0,
            export_result: None,
            export_error: None,
            export_bars: 1,
            export_normalize: true,
            export_bit_depth: 16,
            export_add_library: true,
            recovery_available,
            drag_paint: None,
            show_help: false,
            pending_channel_kind: ChannelKind::Kick,
            ui: StudioUiState::default(),
        };
        state.refresh_projects();
        state.load_samples();
        state.sync_engine();
        state
    }

    pub fn refresh_projects(&mut self) {
        self.projects = scan_projects(&self.project_dir);
    }

    pub fn new_channel_label(&self) -> &'static str {
        self.pending_channel_kind.label()
    }

    pub fn push_undo(&mut self) {
        if self.undo.last() != Some(&self.project) {
            self.undo.push(self.project.clone());
            if self.undo.len() > 64 {
                self.undo.remove(0);
            }
        }
        self.redo.clear();
    }

    pub fn undo(&mut self) {
        if let Some(prev) = self.undo.pop() {
            self.redo.push(self.project.clone());
            self.project = prev;
            self.project.ensure_invariants();
            self.after_project_replaced();
        }
    }

    pub fn redo(&mut self) {
        if let Some(next) = self.redo.pop() {
            self.undo.push(self.project.clone());
            self.project = next;
            self.project.ensure_invariants();
            self.after_project_replaced();
        }
    }

    fn after_project_replaced(&mut self) {
        self.project_dirty = true;
        self.dirty = true;
        self.export_bars = self.export_bars.max(1);
        self.playhead = 0;
        if let Some(e) = &self.engine {
            e.stop();
        }
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
        self.project_dirty = true;
        self.project.modified_at = now_ms();
    }

    pub fn sync_engine(&mut self) {
        if !self.project_dirty {
            return;
        }
        if let Some(e) = &self.engine {
            e.set_project(&self.project);
            e.set_metronome(self.metronome);
        }
        self.project_dirty = false;
    }

    pub fn play(&mut self) {
        self.sync_engine();
        if let Some(e) = &self.engine {
            e.set_metronome(self.metronome);
            e.play();
        }
    }

    pub fn pause_playback(&mut self) {
        if let Some(e) = &self.engine {
            e.pause();
        }
    }

    pub fn stop(&mut self) {
        if let Some(e) = &self.engine {
            e.stop();
        }
        self.playhead = 0;
    }

    pub fn seek_step(&mut self, step: u64) {
        if self.project.song_mode {
            let len = self.project.song_length_steps().max(1) as u64;
            if let Some(e) = &self.engine {
                e.seek_step(step % len);
            }
        } else if let Some(e) = &self.engine {
            e.seek_step(step);
        }
    }

    pub fn save(&mut self) -> bool {
        let path = self.project_path.clone().unwrap_or_else(|| {
            self.project_dir
                .join(format!("{}.rmsproj", self.project.id))
        });
        self.project.ensure_invariants();
        if let Err(e) = self.project.save_to(&path) {
            self.status = Some(format!("Échec de l'enregistrement : {e}"));
            return false;
        }
        self.project_path = Some(path.clone());
        self.dirty = false;
        self.status = Some(format!("Projet enregistré : {}", path.display()));
        let auto = self.autosave_path();
        let _ = std::fs::remove_file(auto);
        self.recovery_available = None;
        self.refresh_projects();
        true
    }

    pub fn save_as(&mut self, path: PathBuf) -> bool {
        self.project_path = Some(path);
        self.save()
    }

    pub fn new_project(&mut self) {
        let mut project = Project::new_default("Nouveau projet");
        project.id = unique_id("proj");
        project.ensure_invariants();
        let path = self.project_dir.join(format!("{}.rmsproj", project.id));
        let _ = project.save_to(&path);
        self.project = project;
        self.project_path = Some(path);
        self.undo.clear();
        self.redo.clear();
        self.dirty = false;
        self.project_dirty = true;
        self.export_result = None;
        self.export_error = None;
        self.playhead = 0;
        self.samples.clear();
        self.refresh_projects();
        if let Some(e) = &self.engine {
            e.stop();
        }
        self.sync_engine();
    }

    pub fn open(&mut self, path: PathBuf) {
        match Project::load_from(&path) {
            Ok(project) => {
                self.project = project;
                self.project_path = Some(path);
                self.undo.clear();
                self.redo.clear();
                self.dirty = false;
                self.project_dirty = true;
                self.export_result = None;
                self.export_error = None;
                self.playhead = 0;
                self.load_samples();
                if let Some(e) = &self.engine {
                    e.stop();
                    e.set_samples(self.samples.clone());
                }
                self.sync_engine();
                self.refresh_projects();
                self.status = None;
            }
            Err(e) => self.status = Some(format!("Ouverture impossible : {e}")),
        }
    }

    fn autosave_path(&self) -> PathBuf {
        self.project_dir
            .join(format!("{}.autosave.rmsproj", self.project.id))
    }

    pub fn maybe_autosave(&mut self) {
        if !self.dirty {
            return;
        }
        if self.last_autosave.elapsed() < Duration::from_secs(4) {
            return;
        }
        let path = self.autosave_path();
        if self.project.save_to(&path).is_ok() {
            self.last_autosave = Instant::now();
            if self.recovery_available.is_none() {
                self.recovery_available = Some(path);
            }
        }
    }

    pub fn save_if_dirty(&mut self) {
        if self.dirty {
            let _ = self.save();
        }
    }

    fn load_samples(&mut self) {
        let mut bank = SampleBank::new();
        for channel in &self.project.channels {
            if channel.kind != crate::studio::project::ChannelKind::Sample {
                continue;
            }
            let Some(path) = &channel.sample_path else {
                continue;
            };
            match decode_to_mono(Path::new(path)) {
                Ok(decoded) => {
                    bank.insert(
                        channel.id.clone(),
                        LoadedSample {
                            samples: Arc::new(decoded.samples),
                            sample_rate: decoded.sample_rate,
                        },
                    );
                }
                Err(e) => {
                    self.status = Some(format!("Échantillon « {} » : {e}", channel.name));
                }
            }
        }
        self.samples = bank;
    }

    pub fn assign_sample(&mut self, channel_index: usize, path: PathBuf) {
        let Some(channel) = self.project.channels.get(channel_index) else {
            return;
        };
        let channel_id = channel.id.clone();
        match decode_to_mono(&path) {
            Ok(decoded) => {
                self.push_undo();
                let rate = decoded.sample_rate;
                let len = decoded.samples.len();
                self.samples.insert(
                    channel_id.clone(),
                    LoadedSample {
                        samples: Arc::new(decoded.samples),
                        sample_rate: rate,
                    },
                );
                if let Some(ch) = self.project.channels.get_mut(channel_index) {
                    ch.sample_path = Some(path.display().to_string());
                    if ch.name == ch.kind.label() {
                        ch.name = path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("Échantillon")
                            .to_string();
                    }
                }
                self.mark_dirty();
                if let Some(e) = &self.engine {
                    e.set_samples(self.samples.clone());
                }
                self.status = Some(format!(
                    "Échantillon chargé : {} ({} s)",
                    path.display(),
                    len as f32 / rate.max(1) as f32
                ));
            }
            Err(e) => self.status = Some(format!("Décodage impossible : {e}")),
        }
    }

    /// Remplit le pattern actif avec un beat de démonstration original.
    pub fn load_demo_beat(&mut self) {
        use crate::studio::project::ChannelKind;
        self.push_undo();
        let mut channel_indices: Vec<(ChannelKind, usize)> = Vec::new();
        for kind in [
            ChannelKind::Kick,
            ChannelKind::Clap,
            ChannelKind::HatClosed,
            ChannelKind::Bass808,
        ] {
            let idx = self
                .project
                .channels
                .iter()
                .position(|c| c.kind == kind)
                .unwrap_or_else(|| self.project.add_channel(kind, None));
            channel_indices.push((kind, idx));
        }
        if self.project.steps < 16 {
            self.project.resize_steps(16);
        }
        for lane in &mut self.project.active_pattern_mut().lanes {
            for step in lane.iter_mut() {
                *step = Step::default();
            }
        }
        let pattern = self.project.active_pattern_mut();
        for (kind, idx) in channel_indices {
            let Some(lane) = pattern.lanes.get_mut(idx) else {
                continue;
            };
            match kind {
                ChannelKind::Kick => {
                    for (s, v) in [(0usize, 1.0f32), (3, 0.7), (6, 1.0), (10, 0.8)] {
                        lane[s] = Step {
                            on: true,
                            velocity: v,
                            ..Step::default()
                        };
                    }
                }
                ChannelKind::Clap => {
                    for s in [4usize, 12] {
                        lane[s] = Step::accent();
                    }
                }
                ChannelKind::HatClosed => {
                    for s in (0..16).step_by(2) {
                        lane[s] = Step {
                            on: true,
                            velocity: if s % 4 == 0 { 0.55 } else { 0.85 },
                            ..Step::default()
                        };
                    }
                }
                ChannelKind::Bass808 => {
                    for s in [0usize, 6, 10, 14] {
                        lane[s] = Step {
                            on: true,
                            velocity: 0.95,
                            ..Step::default()
                        };
                    }
                }
                _ => {}
            }
        }
        if self.project.arrangement.is_empty() {
            self.project.arrangement.push(crate::studio::project::Clip {
                pattern: self.project.active_pattern,
                start_step: 0,
                length_steps: self.project.steps,
            });
        }
        self.mark_dirty();
        self.status = Some("Beat de démonstration chargé. Appuie sur ▶.".to_string());
    }

    pub fn clear_pattern(&mut self) {
        self.push_undo();
        let pattern = self.project.active_pattern_mut();
        for lane in &mut pattern.lanes {
            for step in lane.iter_mut() {
                *step = Step::default();
            }
        }
        self.mark_dirty();
    }

    pub fn start_export(&mut self) {
        if self.export_rx.is_some() {
            return;
        }
        let options = ExportOptions {
            sample_rate: 44_100,
            bars: self.export_bars.max(1),
            tail_secs: 1.0,
            normalize: self.export_normalize,
            bit_depth: self.export_bit_depth,
        };
        let default_name = format!("{}.wav", self.project.export_file_stem());
        let selected = rfd::FileDialog::new()
            .set_directory(&self.export_dir)
            .set_file_name(&default_name)
            .add_filter("WAV", &["wav"])
            .save_file();
        let Some(path) = selected else {
            return;
        };
        if let Some(parent) = path.parent() {
            self.export_dir = parent.to_path_buf();
        }
        let (tx, rx) = mpsc::channel();
        self.export_rx = Some(rx);
        self.export_progress = 0.0;
        self.export_result = None;
        self.export_error = None;
        let project = self.project.clone();
        let bank = self.samples.clone();
        let add_library = self.export_add_library;
        thread::spawn(move || {
            let result = render_to_wav(&project, &bank, &path, &options, |p| {
                let _ = tx.send(ExportMsg::Progress(p));
            });
            let _ = tx.send(ExportMsg::Done {
                result,
                add_library,
                path,
            });
        });
    }

    pub fn poll_export(&mut self) -> Vec<StudioAction> {
        let mut actions = Vec::new();
        let mut finished = false;
        if let Some(rx) = &self.export_rx {
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    ExportMsg::Progress(p) => self.export_progress = p,
                    ExportMsg::Done {
                        result,
                        add_library,
                        path,
                    } => {
                        match result {
                            Ok(r) => {
                                self.status = Some(format!(
                                    "Export terminé : {} ({:.1} s, pic {:.1} dBFS)",
                                    r.path.display(),
                                    r.duration_secs,
                                    20.0 * r.peak.max(1e-6).log10()
                                ));
                                if add_library {
                                    actions.push(StudioAction::ImportToLibrary(path));
                                }
                                self.export_result = Some(r);
                            }
                            Err(e) => self.export_error = Some(e),
                        }
                        finished = true;
                    }
                }
            }
        }
        if finished {
            self.export_rx = None;
            self.export_progress = 1.0;
        }
        actions
    }

    pub fn playhead_step(&self) -> usize {
        self.engine
            .as_ref()
            .map(|e| e.current_step())
            .unwrap_or(self.playhead)
    }

    pub fn is_playing(&self) -> bool {
        self.engine
            .as_ref()
            .map(|e| e.is_playing())
            .unwrap_or(false)
    }

    pub fn position_label(&self) -> String {
        let secs = self
            .engine
            .as_ref()
            .map(|e| e.position_secs())
            .unwrap_or(0.0);
        let beats = secs * self.project.bpm.max(1.0) / 60.0;
        let num = self.project.time_sig_num.max(1) as f64;
        let bar = (beats / num).floor() as i64 + 1;
        let beat = (beats % num).floor() as i64 + 1;
        let step = ((beats.fract() * 4.0).floor() as i64).clamp(0, 3) + 1;
        format!("{bar}:{beat}:{step}")
    }
}
