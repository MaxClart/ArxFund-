//! Music Player connecté à la bibliothèque locale.
use crate::app::{Page, RepeatMode, StudioApp, format_duration};
use crate::library::Song;
use crate::ui::theme::ARXFUNDTheme;
use eframe::egui::{self, Color32, RichText, Ui};
#[derive(Clone, Default)]
pub struct PlayerTheme {
    pub arxfund: ARXFUNDTheme,
}
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum LibraryView {
    #[default]
    Tracks,
    Albums,
    Artists,
    Playlists,
}

pub fn sidebar(ui: &mut Ui, theme: &ARXFUNDTheme, app: &mut StudioApp) -> Option<Page> {
    ui.add_space(18.0);
    ui.label(RichText::new("ARXFUND").size(24.0).strong());
    ui.label(RichText::new("Écouter. Composer.").color(theme.colors.text_secondary));
    ui.add_space(28.0);
    let mut next = None;
    for (label, page) in [
        ("Accueil", Page::Home),
        ("Recherche", Page::Search),
        ("Bibliothèque", Page::Library),
        ("Favoris", Page::Favorites),
        ("Playlists", Page::Playlists),
        ("Importer de la musique", Page::Imports),
        ("Production Studio", Page::Studio),
        ("Paramètres", Page::Settings),
    ] {
        if page == Page::Studio {
            ui.add_space(18.0);
            ui.separator();
            ui.add_space(10.0);
        }
        if ui
            .add_sized(
                [ui.available_width(), 36.0],
                egui::Button::new(label)
                    .fill(if app.page == page {
                        theme.colors.row_selected
                    } else {
                        Color32::TRANSPARENT
                    })
                    .selected(app.page == page),
            )
            .clicked()
        {
            next = Some(page);
        }
        ui.add_space(4.0);
    }
    ui.add_space(22.0);
    if ui
        .checkbox(&mut app.studio.beginner, "Aide à la composition")
        .changed()
    {
        if let Err(e) = app.db.set_setting(
            "studio.beginner",
            if app.studio.beginner { "1" } else { "0" },
        ) {
            app.error_msg = Some(e.to_string());
        }
    }
    next
}

pub fn home(ui: &mut Ui, theme: &ARXFUNDTheme, app: &mut StudioApp, ctx: &egui::Context) {
    ui.add_space(20.0);
    ui.heading(RichText::new("Votre musique, votre espace.").size(30.0));
    ui.label(
        RichText::new("Une bibliothèque locale et un studio pour vos idées.")
            .color(theme.colors.text_secondary),
    );
    ui.add_space(22.0);
    ui.horizontal(|ui| {
        if ui.button("Importer des morceaux").clicked() {
            app.page = Page::Imports;
        }
        if ui.button("Ouvrir le studio").clicked() {
            app.page = Page::Studio;
        }
    });
    ui.add_space(24.0);
    if app.songs.is_empty() {
        ui.label("Ajoutez des fichiers WAV, FLAC, MP3, OGG ou AAC pour commencer.");
    } else {
        ui.heading("À écouter");
        track_list(ui, theme, app, app.songs.clone(), ctx, None);
    }
}

pub fn library(ui: &mut Ui, theme: &ARXFUNDTheme, app: &mut StudioApp, ctx: &egui::Context) {
    let title = match app.page {
        Page::Favorites => "Favoris",
        Page::Search => "Recherche",
        _ => "Bibliothèque",
    };
    ui.heading(RichText::new(title).size(28.0));
    ui.add_space(12.0);
    ui.add_sized(
        [ui.available_width().min(560.0), 32.0],
        egui::TextEdit::singleline(&mut app.search).hint_text("Morceau, artiste ou album"),
    );
    let query = app.search.trim().to_lowercase();
    let songs = app
        .songs
        .iter()
        .filter(|s| {
            (app.page != Page::Favorites || s.is_favorite)
                && (query.is_empty()
                    || format!(
                        "{} {} {}",
                        s.title,
                        s.artist.as_deref().unwrap_or(""),
                        s.album.as_deref().unwrap_or("")
                    )
                    .to_lowercase()
                    .contains(&query))
        })
        .cloned()
        .collect::<Vec<_>>();
    ui.add_space(12.0);
    ui.label(RichText::new(format!("{} morceaux", songs.len())).color(theme.colors.text_secondary));
    track_list(ui, theme, app, songs, ctx, None);
}

fn track_list(
    ui: &mut Ui,
    theme: &ARXFUNDTheme,
    app: &mut StudioApp,
    songs: Vec<Song>,
    _ctx: &egui::Context,
    playlist: Option<&str>,
) {
    ui.add_space(12.0);
    if songs.is_empty() {
        ui.label("Aucun morceau. Importez de la musique ou modifiez votre recherche.");
    }
    egui::ScrollArea::vertical()
        .id_salt("tracks")
        .auto_shrink([false, false])
        .show_rows(ui, 58.0, songs.len(), |ui, range| {
            for i in range {
                let song = &songs[i];
                let selected = app.current_song().is_some_and(|s| s.id == song.id);
                egui::Frame::new()
                    .fill(if selected {
                        theme.colors.row_selected
                    } else if i % 2 == 0 {
                        theme.colors.bg_secondary
                    } else {
                        Color32::TRANSPARENT
                    })
                    .inner_margin(8)
                    .corner_radius(4.0)
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.horizontal(|ui| {
                            if ui.button("▶").on_hover_text("Lire ce morceau").clicked() {
                                app.play_queue(songs.clone(), i);
                            }
                            ui.add_space(8.0);
                            ui.vertical(|ui| {
                                ui.set_width((ui.available_width() - 230.0).max(100.0));
                                ui.label(RichText::new(&song.title).strong());
                                ui.label(
                                    RichText::new(
                                        song.artist.as_deref().unwrap_or("Artiste inconnu"),
                                    )
                                    .small()
                                    .color(theme.colors.text_secondary),
                                );
                            });
                            ui.label(format_duration(song.duration.unwrap_or(0.0)));
                            if ui
                                .button(if song.is_favorite { "♥" } else { "♡" })
                                .on_hover_text("Favori")
                                .clicked()
                            {
                                match app.db.set_favorite(&song.id, !song.is_favorite) {
                                    Ok(()) => app.load_library(),
                                    Err(e) => app.error_msg = Some(e.to_string()),
                                }
                            }
                            ui.menu_button("+", |ui| {
                                if app.playlists.is_empty() {
                                    ui.label("Créez une playlist dans Playlists.");
                                }
                                for pl in app.playlists.clone() {
                                    if ui.button(&pl.name).clicked() {
                                        if let Err(e) =
                                            app.db.add_track_to_playlist(&pl.id, &song.id)
                                        {
                                            app.error_msg = Some(e.to_string());
                                        }
                                        app.load_playlists();
                                        ui.close();
                                    }
                                }
                            })
                            .response
                            .on_hover_text("Ajouter à une playlist");
                            if let Some(id) = playlist {
                                if ui.button("Retirer").clicked() {
                                    if let Err(e) = app.db.remove_track_from_playlist(id, &song.id)
                                    {
                                        app.error_msg = Some(e.to_string());
                                    }
                                    app.playlist_tracks =
                                        app.db.playlist_tracks(id).unwrap_or_default();
                                    app.load_playlists();
                                }
                            }
                        });
                    });
            }
        });
}

pub fn playlists(ui: &mut Ui, theme: &ARXFUNDTheme, app: &mut StudioApp) {
    ui.heading(RichText::new("Playlists").size(28.0));
    ui.add_space(12.0);
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut app.new_playlist_name).hint_text("Nom de la playlist"),
        );
        if ui
            .add_enabled(
                !app.new_playlist_name.trim().is_empty(),
                egui::Button::new("Créer"),
            )
            .clicked()
        {
            let id = crate::studio::project::unique_id("playlist");
            match app
                .db
                .create_playlist(&id, app.new_playlist_name.trim(), None)
            {
                Ok(_) => {
                    app.new_playlist_name.clear();
                    app.load_playlists();
                    app.selected_playlist = Some(id);
                    app.playlist_tracks.clear();
                }
                Err(e) => app.error_msg = Some(e.to_string()),
            }
        }
    });
    ui.add_space(12.0);
    ui.horizontal_wrapped(|ui| {
        for pl in app.playlists.clone() {
            if ui
                .selectable_label(
                    app.selected_playlist.as_ref() == Some(&pl.id),
                    format!("{} ({})", pl.name, pl.track_count),
                )
                .clicked()
            {
                match app.db.playlist_tracks(&pl.id) {
                    Ok(tracks) => {
                        app.playlist_tracks = tracks;
                        app.selected_playlist = Some(pl.id);
                    }
                    Err(e) => app.error_msg = Some(e.to_string()),
                }
            }
        }
    });
    let ctx = ui.ctx().clone();
    if let Some(id) = app.selected_playlist.clone() {
        track_list(ui, theme, app, app.playlist_tracks.clone(), &ctx, Some(&id));
    } else {
        ui.label("Créez ou sélectionnez une playlist. Ajoutez des morceaux avec le bouton + de la bibliothèque.");
    }
}

pub fn player_bar(ui: &mut Ui, theme: &ARXFUNDTheme, app: &mut StudioApp, _ctx: &egui::Context) {
    let song = app.current_song().cloned();
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.set_width(200.0);
            ui.label(
                RichText::new(
                    song.as_ref()
                        .map(|s| s.title.as_str())
                        .unwrap_or("Aucun morceau sélectionné"),
                )
                .strong(),
            );
            ui.label(
                RichText::new(
                    song.as_ref()
                        .and_then(|s| s.artist.as_deref())
                        .unwrap_or("Votre bibliothèque locale"),
                )
                .small()
                .color(theme.colors.text_secondary),
            );
        });
        ui.add_enabled_ui(song.is_some(), |ui| {
            if ui.button("|◀").on_hover_text("Précédent").clicked() {
                app.prev_track();
            }
            if ui
                .button(if app.engine.is_playing() {
                    "Pause"
                } else {
                    "Lecture"
                })
                .clicked()
            {
                app.toggle_play();
            }
            if ui.button("▶|").on_hover_text("Suivant").clicked() {
                app.next_track();
            }
        });
        if ui
            .selectable_label(
                app.repeat != RepeatMode::Off,
                match app.repeat {
                    RepeatMode::Off => "Répéter : non",
                    RepeatMode::All => "Répéter : tout",
                    RepeatMode::One => "Répéter : un",
                },
            )
            .clicked()
        {
            app.repeat = match app.repeat {
                RepeatMode::Off => RepeatMode::All,
                RepeatMode::All => RepeatMode::One,
                RepeatMode::One => RepeatMode::Off,
            };
        }
        ui.label("Volume");
        ui.spacing_mut().slider_width = 90.0;
        if ui
            .add(egui::Slider::new(&mut app.volume, 0.0..=1.0).show_value(false))
            .changed()
        {
            app.engine.set_volume(app.volume);
        }
    });
    ui.horizontal(|ui| {
        let duration = app.engine.duration_secs();
        let mut position = app.engine.position_secs().min(duration);
        ui.label(format_duration(position));
        ui.spacing_mut().slider_width = (ui.available_width() - 60.0).max(80.0);
        if ui
            .add_enabled(
                duration > 0.0,
                egui::Slider::new(&mut position, 0.0..=duration.max(0.001)).show_value(false),
            )
            .changed()
        {
            app.engine.seek(position);
        }
        ui.label(format_duration(duration));
    });
}
