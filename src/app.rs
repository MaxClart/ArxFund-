use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::Instant;

use eframe::egui;
use egui::TextureHandle;
use egui::{Color32, RichText};

use crate::audio::AudioEngine;
use crate::library::models::Playlist;
use crate::library::{Database, Song, SortDir, SortField, collect_audio_files, import_one};
use crate::paths;
use crate::studio::engine::StudioEngine;
use crate::studio::state::{StudioAction, StudioState};
use crate::studio::ui as studio_ui;
use crate::ui::components::{
    button, section_header,
};
use crate::ui::player::{home, library, player_bar, playlists, sidebar};
use crate::ui::theme::ARXFUNDTheme;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Home,
    Search,
    Library,
    Playlists,
    Favorites,
    Imports,
    Dj,
    Studio,
    Settings,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RepeatMode {
    Off,
    All,
    One,
}

#[derive(Debug)]
pub struct ImportProgress {
    pub done: usize,
    pub total: usize,
    pub imported: usize,
    pub duplicates: usize,
    pub errors: usize,
}

pub struct StudioApp {
    pub db: Database,
    pub engine: AudioEngine,
    pub page: Page,
    pub songs: Vec<Song>,
    pub playlists: Vec<Playlist>,
    pub search: String,
    pub sort: (SortField, SortDir),
    pub queue: Vec<Song>,
    pub current: Option<usize>,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    pub volume: f32,
    pub import_rx: Option<mpsc::Receiver<ImportProgress>>,
    pub import_progress: Option<ImportProgress>,
    pub import_results: Vec<(String, String)>,
    pub cover_textures: HashMap<String, TextureHandle>,
    pub resume_song: Option<Song>,
    pub resume_pos: f64,
    pub data_dir: PathBuf,
    pub covers_dir: PathBuf,
    pub db_path: PathBuf,
    pub error_msg: Option<String>,
    pub selected_playlist: Option<String>,
    pub playlist_tracks: Vec<Song>,
    pub new_playlist_name: String,
    pub studio: StudioState,
    pub audio_error: Option<String>,
    pub startup_notes: Vec<String>,
    pub splash_until: Instant,
    pub about_open: bool,
    pub theme: ARXFUNDTheme,
    pub library_view: Option<crate::ui::player::LibraryView>,
    pub library_selected_song: Option<String>,
}

pub fn format_duration(s: f64) -> String {
    if !s.is_finite() || s < 0.0 {
        return "--:--".to_string();
    }
    let t = s.floor() as i64;
    let h = t / 3600;
    let m = (t % 3600) / 60;
    let sec = t % 60;
    if h > 0 {
        format!("{}:{:02}:{:02}", h, m, sec)
    } else {
        format!("{}:{:02}", m, sec)
    }
}

pub fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

impl StudioApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        db: Database,
        engine: AudioEngine,
        db_path: PathBuf,
        data_dir: PathBuf,
        covers_dir: PathBuf,
        audio_error: Option<String>,
        startup_notes: Vec<String>,
    ) -> Self {
        let mut visuals = egui::Visuals::dark();
        visuals.window_fill = Color32::from_rgb(13, 15, 20);
        visuals.panel_fill = Color32::from_rgb(21, 25, 34);
        visuals.faint_bg_color = Color32::from_rgb(30, 34, 46);
        visuals.extreme_bg_color = Color32::from_rgb(13, 15, 20);
        visuals.selection.bg_fill = Color32::from_rgb(139, 92, 246);
        visuals.hyperlink_color = Color32::from_rgb(139, 92, 246);
        cc.egui_ctx.set_visuals(visuals);

        cc.egui_ctx.global_style_mut(|style| {
            style.spacing.item_spacing = egui::vec2(8.0, 6.0);
            style.spacing.button_padding = egui::vec2(10.0, 6.0);
        });

        let beginner = db
            .get_setting("studio.beginner")
            .ok()
            .flatten()
            .map(|v| v == "1")
            .unwrap_or(false);
        let (studio_engine, studio_error) = match StudioEngine::new() {
            Ok(e) => (Some(e), None),
            Err(e) => (None, Some(e)),
        };
        let studio = StudioState::new(
            crate::paths::music_root(),
            beginner,
            studio_engine,
            studio_error,
        );

        let mut app = Self {
            db,
            engine,
            page: Page::Home,
            songs: Vec::new(),
            playlists: Vec::new(),
            search: String::new(),
            sort: (SortField::AddedAt, SortDir::Desc),
            queue: Vec::new(),
            current: None,
            shuffle: false,
            repeat: RepeatMode::Off,
            volume: 0.8,
            import_rx: None,
            import_progress: None,
            import_results: Vec::new(),
            cover_textures: HashMap::new(),
            resume_song: None,
            resume_pos: 0.0,
            data_dir,
            covers_dir,
            db_path,
            error_msg: None,
            selected_playlist: None,
            playlist_tracks: Vec::new(),
            new_playlist_name: String::new(),
            studio,
            audio_error,
            startup_notes,
            splash_until: Instant::now() + std::time::Duration::from_millis(1500),
            about_open: false,
            theme: ARXFUNDTheme::dark(),
            library_view: None,
            library_selected_song: None,
        };
        app.load_library();
        app.load_playlists();
        app.restore_settings();
        app
    }

    pub fn load_library(&mut self) {
        match self.db.list_songs(self.sort.0, self.sort.1) {
            Ok(s) => self.songs = s,
            Err(e) => self.error_msg = Some(format!("Chargement bibliothèque : {}", e)),
        }
    }

    pub fn load_playlists(&mut self) {
        if let Ok(p) = self.db.list_playlists() {
            self.playlists = p;
        }
    }

    fn restore_settings(&mut self) {
        if let Ok(Some(v)) = self.db.get_setting("player.volume") {
            if let Ok(f) = v.parse::<f32>() {
                self.volume = f.clamp(0.0, 1.0);
                self.engine.set_volume(self.volume);
            }
        }
        if let Ok(Some(v)) = self.db.get_setting("player.resume") {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&v) {
                let sid = val.get("song_id").and_then(|x| x.as_str());
                let pos = val.get("position").and_then(|x| x.as_f64()).unwrap_or(0.0);
                if let Some(sid) = sid {
                    if let Ok(Some(song)) = self.db.get_song(sid) {
                        if song.local_path.is_some() {
                            self.resume_song = Some(song);
                            self.resume_pos = pos;
                        }
                    }
                }
            }
        }
    }

    fn persist_resume(&self) {
        if let Some(song) = self.current_song() {
            let info =
                serde_json::json!({ "song_id": song.id, "position": self.engine.position_secs() });
            let _ = self.db.set_setting("player.resume", &info.to_string());
        }
        let _ = self
            .db
            .set_setting("player.volume", &self.volume.to_string());
    }

    pub fn current_song(&self) -> Option<&Song> {
        self.current.and_then(|i| self.queue.get(i))
    }

    fn handle_studio_actions(&mut self, actions: Vec<StudioAction>) {
        for action in actions {
            match action {
                StudioAction::PauseMusic => self.engine.pause(),
                StudioAction::ImportToLibrary(path) => {
                    match import_one(&self.db, &self.covers_dir, &path) {
                        Ok(Some(_)) => {
                            self.load_library();
                            self.studio.status =
                                Some("Export terminé et ajouté à la bibliothèque.".to_string());
                        }
                        Ok(None) => {
                            self.studio.status =
                                Some("Ce fichier existe déjà dans la bibliothèque.".to_string());
                        }
                        Err(e) => {
                            self.studio.status = Some(format!("Import de l'export : {e}"));
                        }
                    }
                }
                StudioAction::PersistBeginner(on) => {
                    let _ = self
                        .db
                        .set_setting("studio.beginner", if on { "1" } else { "0" });
                }
            }
        }
    }

    pub fn play_queue(&mut self, songs: Vec<Song>, start: usize) {
        if songs.is_empty() {
            return;
        }
        self.queue = songs;
        let idx = start.min(self.queue.len().saturating_sub(1));
        self.current = Some(idx);
        self.play_current();
    }

    fn play_current(&mut self) {
        self.studio.pause_playback();
        if let Some(idx) = self.current {
            if let Some(song) = self.queue.get(idx).cloned() {
                if let Some(path) = song.local_path.clone() {
                    self.engine.load(path, true);
                    let _ = self
                        .db
                        .set_setting("player.volume", &self.volume.to_string());
                }
            }
        }
    }

    pub fn toggle_play(&mut self) {
        if self.current.is_none() && !self.queue.is_empty() {
            self.current = Some(0);
            self.play_current();
            return;
        }
        if self.engine.is_playing() {
            self.engine.pause();
        } else {
            self.studio.pause_playback();
            self.engine.play();
        }
    }

    pub fn prev_track(&mut self) {
        if self.queue.is_empty() {
            return;
        }
        if self.engine.position_secs() > 3.0 {
            self.engine.seek(0.0);
            return;
        }
        let n = self.queue.len();
        let cur = self.current.unwrap_or(0);
        let mut pi = cur as isize - 1;
        if pi < 0 {
            if self.repeat == RepeatMode::All {
                pi = n as isize - 1;
            } else {
                pi = 0;
            }
        }
        self.current = Some(pi as usize);
        self.play_current();
    }

    pub fn next_track(&mut self) {
        if self.queue.is_empty() {
            return;
        }
        let n = self.queue.len();
        let cur = self.current.unwrap_or(0);
        let next = if self.shuffle {
            let mut r = (cur + 1) % n;
            if n > 1 {
                while r == cur {
                    r = (r + 1) % n;
                }
            }
            r
        } else {
            let mut ni = cur + 1;
            if ni >= n {
                if self.repeat == RepeatMode::All {
                    ni = 0;
                } else {
                    self.engine.pause();
                    return;
                }
            }
            ni
        };
        self.current = Some(next);
        self.play_current();
    }

    pub fn get_cover_texture(&mut self, ctx: &egui::Context, path: &str) -> Option<TextureHandle> {
        if let Some(t) = self.cover_textures.get(path) {
            return Some(t.clone());
        }
        let bytes = std::fs::read(path).ok()?;
        let img = image::load_from_memory(&bytes).ok()?;
        let rgba = img.to_rgba8();
        let size = [rgba.width() as usize, rgba.height() as usize];
        let color = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
        let tex = ctx.load_texture(path, color, egui::TextureOptions::LINEAR);
        self.cover_textures.insert(path.to_string(), tex.clone());
        Some(tex)
    }

    fn poll_import(&mut self) {
        if let Some(rx) = &self.import_rx {
            while let Ok(p) = rx.try_recv() {
                self.import_progress = Some(p);
            }
        }
        if let Some(p) = &self.import_progress {
            if p.done >= p.total && p.total > 0 && self.import_rx.is_some() {
                self.load_library();
                self.import_rx = None;
            }
        }
    }

    fn start_import(&mut self, paths: Vec<PathBuf>) {
        if paths.is_empty() {
            return;
        }
        let db = self.db_path.clone();
        let covers = self.covers_dir.clone();
        let (tx, rx) = mpsc::channel();
        self.import_rx = Some(rx);
        self.import_progress = Some(ImportProgress {
            done: 0,
            total: paths.len(),
            imported: 0,
            duplicates: 0,
            errors: 0,
        });
        self.import_results.clear();
        thread::spawn(move || {
            let db = match Database::open(&db) {
                Ok(d) => d,
                Err(_) => return,
            };
            let total = paths.len();
            let mut done = 0usize;
            let mut imported = 0usize;
            let mut duplicates = 0usize;
            let mut errors = 0usize;
            for p in &paths {
                match import_one(&db, &covers, p) {
                    Ok(Some(_)) => imported += 1,
                    Ok(None) => duplicates += 1,
                    Err(_) => errors += 1,
                }
                done += 1;
                let _ = tx.send(ImportProgress {
                    done,
                    total,
                    imported,
                    duplicates,
                    errors,
                });
            }
        });
    }

    fn render_imports(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        section_header(
            ui,
            &self.theme,
            "Importation",
            Some("Importez vos fichiers audio dans la bibliothèque"),
        );
        ui.horizontal(|ui| {
            if button(ui, &self.theme, "Fichiers…").clicked() {
                if let Some(paths) = rfd::FileDialog::new()
                    .add_filter(
                        "Audio",
                        &["mp3", "wav", "flac", "ogg", "oga", "opus", "m4a", "aac"],
                    )
                    .pick_files()
                {
                    self.start_import(paths);
                }
            }
            ui.add_space(self.theme.spacing.sm);
            if button(ui, &self.theme, "Dossier…").clicked() {
                if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                    let files = collect_audio_files(&dir);
                    self.start_import(files);
                }
            }
        });
        ui.add_space(self.theme.spacing.md);
        if let Some(p) = &self.import_progress {
            ui.label(
                RichText::new(format!("Import : {}/{}", p.done, p.total))
                    .color(self.theme.colors.text_secondary),
            );
            let frac = if p.total > 0 {
                p.done as f32 / p.total as f32
            } else {
                0.0
            };
            ui.add(egui::ProgressBar::new(frac).show_percentage());
            ui.label(
                RichText::new(format!(
                    "Ajoutés : {}  ·  Doublons : {}  ·  Erreurs : {}",
                    p.imported, p.duplicates, p.errors
                ))
                .color(self.theme.colors.text_secondary),
            );
        }
        ctx.input(|i| {
            if !i.raw.dropped_files.is_empty() {
                let paths: Vec<PathBuf> = i
                    .raw
                    .dropped_files
                    .iter()
                    .map(|f| f.path().to_path_buf())
                    .collect();
                if !paths.is_empty() {
                    self.start_import(paths);
                }
            }
        });
        ui.add_space(self.theme.spacing.md);
        ui.label(
            RichText::new("Glissez-déposez des fichiers audio dans la fenêtre.")
                .color(self.theme.colors.text_muted),
        );
    }

    fn render_settings(&mut self, ui: &mut egui::Ui) {
        section_header(
            ui,
            &self.theme,
            "Paramètres",
            Some("Configuration d'ARXFUND"),
        );
        ui.add_space(self.theme.spacing.md);
        ui.label(
            RichText::new("ARXFUND")
                .size(20.0)
                .strong()
                .color(self.theme.colors.accent_primary),
        );
        ui.label(format!("Version : {}", env!("CARGO_PKG_VERSION")));
        ui.add_space(self.theme.spacing.md);
        ui.label(
            RichText::new("Emplacements")
                .strong()
                .color(self.theme.colors.text_secondary),
        );
        ui.label(format!(
            "Données et paramètres : {}",
            self.data_dir.display()
        ));
        ui.label(format!("Base de données : {}", self.db_path.display()));
        ui.label(format!("Pochettes : {}", self.covers_dir.display()));
        ui.label(format!(
            "Projets studio : {}",
            paths::projects_dir().display()
        ));
        ui.label(format!("Exports WAV : {}", paths::exports_dir().display()));
        ui.label(format!(
            "Enregistrements : {}",
            paths::recordings_dir().display()
        ));
        ui.label(format!(
            "Samples personnels : {}",
            paths::user_samples_dir().display()
        ));
        ui.label(format!(
            "Presets personnels : {}",
            paths::user_presets_dir().display()
        ));
        ui.label(format!(
            "Sauvegardes automatiques : {}",
            paths::backups_dir().display()
        ));
        ui.add_space(self.theme.spacing.md);
        if let Some(err) = &self.audio_error {
            ui.colored_label(self.theme.colors.error, err);
        }
        if !self.startup_notes.is_empty() {
            egui::CollapsingHeader::new("Notes de démarrage / migration").show(ui, |ui| {
                for note in &self.startup_notes {
                    ui.label(RichText::new(note).small());
                }
            });
        }
        ui.add_space(self.theme.spacing.md);
        ui.label(
            RichText::new(
                "Confidentialité : tout est local, aucune télémétrie, aucun compte requis.",
            )
            .color(self.theme.colors.text_secondary),
        );
        ui.add_space(self.theme.spacing.md);
        if button(ui, &self.theme, "Forcer la sauvegarde").clicked() {
            let _ = self
                .db
                .set_setting("player.volume", &self.volume.to_string());
        }
        ui.add_space(self.theme.spacing.sm);
        if button(ui, &self.theme, "À propos d'ARXFUND").clicked() {
            self.about_open = true;
        }
    }

    fn render_about(&mut self, ctx: &egui::Context) {
        let mut open = self.about_open;
        let mut close_requested = false;
        egui::Window::new("À propos d'ARXFUND")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(
                    RichText::new("ARXFUND")
                        .size(32.0)
                        .strong()
                        .color(self.theme.colors.accent_primary),
                );
                ui.label(format!("Version {}", env!("CARGO_PKG_VERSION")));
                ui.add_space(self.theme.spacing.sm);
                ui.label("Application 100 % locale, sans compte et sans télémétrie.");
                ui.label("Vos projets, exports et samples restent sur cet ordinateur.");
                ui.add_space(self.theme.spacing.sm);
                ui.label("Windows 10/11 x64 · Rust · egui/eframe · cpal · symphonia");
                ui.add_space(self.theme.spacing.sm);
                if button(ui, &self.theme, "Fermer").clicked() {
                    close_requested = true;
                }
            });
        if close_requested {
            open = false;
        }
        self.about_open = open;
    }

    fn render_placeholder(&mut self, ui: &mut egui::Ui, title: &str, msg: &str) {
        ui.label(
            RichText::new(title)
                .size(18.0)
                .strong()
                .color(self.theme.colors.text_primary),
        );
        ui.add_space(self.theme.spacing.md);
        ui.label(RichText::new(msg).color(self.theme.colors.warning));
        ui.add_space(self.theme.spacing.md);
        ui.label("Cette fonctionnalité est prévue pour une phase ultérieure.");
    }

    fn render_ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        if Instant::now() < self.splash_until {
            egui::CentralPanel::default()
                .frame(egui::Frame::new().fill(Color32::from_rgb(13, 15, 20)))
                .show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(ui.available_height() * 0.30);
                        ui.label(
                            RichText::new("ARXFUND")
                                .size(58.0)
                                .strong()
                                .color(Color32::from_rgb(167, 139, 246)),
                        );
                        ui.label(
                            RichText::new("Lecteur musical · Studio de production")
                                .size(16.0)
                                .color(Color32::from_rgb(148, 163, 184)),
                        );
                        ui.add_space(22.0);
                        ui.spinner();
                        ui.add_space(10.0);
                        ui.label(format!(
                            "Version {} · chargement de votre studio",
                            env!("CARGO_PKG_VERSION")
                        ));
                    });
                });
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
            return;
        }

        if self.engine.is_finished() {
            if self.repeat == RepeatMode::One {
                self.engine.seek(0.0);
                self.engine.play();
            } else {
                self.next_track();
            }
        }
        if let Some(error) = self.engine.take_error() { self.error_msg = Some(error); }
        self.poll_import();
        let export_actions = self.studio.poll_export();
        self.handle_studio_actions(export_actions);
        self.studio.maybe_autosave();
        self.studio.sync_engine();

        let theme = self.theme.clone();

        if !ctx.text_edit_focused() {
            if self.page == Page::Studio {
                if ctx.input(|i| i.key_pressed(egui::Key::Space)) {
                    if self.studio.is_playing() {
                        self.studio.pause_playback();
                    } else {
                        self.studio.play();
                        self.engine.pause();
                    }
                }
                if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Z)) {
                    self.studio.undo();
                }
                if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Y)) {
                    self.studio.redo();
                }
            } else if ctx.input(|i| i.key_pressed(egui::Key::Space)) {
                self.toggle_play();
            }
        }

        egui::Panel::left("sidebar")
            .resizable(true)
            .default_size(200.0)
            .min_size(170.0)
            .max_size(270.0)
            .show(ui, |ui| {
                if let Some(next_page) = sidebar(ui, &theme, self) {
                    self.page = next_page;
                    if matches!(self.page, Page::Library | Page::Favorites | Page::Search) {
                        self.load_library();
                    }
                    if self.page == Page::Playlists {
                        self.load_playlists();
                    }
                }
            });

        egui::Panel::bottom("playerbar")
            .exact_size(96.0)
            .show(ui, |ui| {
                player_bar(ui, &theme, self, &ctx);
            });

        egui::CentralPanel::default().show(ui, |ui| match self.page {
            Page::Home => home(ui, &theme, self, &ctx),
            Page::Search | Page::Library | Page::Favorites => library(ui, &theme, self, &ctx),
            Page::Playlists => playlists(ui, &theme, self),
            Page::Imports => self.render_imports(ui, &ctx),
            Page::Dj => self.render_placeholder(
                ui,
                "Mixage DJ",
                "Phase 3 — non implémenté. Aucun contrôle factice.",
            ),
            Page::Studio => {
                let actions = studio_ui::render(ui, &mut self.studio);
                self.handle_studio_actions(actions);
            }
            Page::Settings => self.render_settings(ui),
        });

        self.render_about(&ctx);

        ctx.request_repaint_after(std::time::Duration::from_millis(if self.engine.is_playing() || self.studio.is_playing() { 33 } else { 250 }));
    }
}

impl eframe::App for StudioApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.render_ui(ui, _frame);
    }

    fn on_exit(&mut self) {
        self.persist_resume();
        self.studio.save_if_dirty();
        self.studio.pause_playback();
    }
}
