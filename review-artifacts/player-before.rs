//! Interface Music Player — version simplifiée.

use eframe::egui;
use egui::{Color32, RichText, Ui, Vec2};

use crate::app::StudioApp;
use crate::library::Song;
use crate::ui::theme::ARXFUNDTheme;

#[derive(Clone, Default)]
pub struct PlayerTheme {
    pub arxfund: ARXFUNDTheme,
}

/// Vue de la bibliothèque
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum LibraryView {
    #[default]
    Tracks,
    Albums,
    Artists,
    Playlists,
}

impl LibraryView {
    pub fn all() -> &'static [Self] {
        &[Self::Albums, Self::Artists, Self::Tracks, Self::Playlists]
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Albums => "Albums",
            Self::Artists => "Artistes",
            Self::Tracks => "Morceaux",
            Self::Playlists => "Playlists",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Self::Albums => "💿",
            Self::Artists => "🎤",
            Self::Tracks => "☰",
            Self::Playlists => "⌃",
        }
    }
}

pub fn sidebar(ui: &mut Ui, theme: &ARXFUNDTheme, app: &mut StudioApp) -> Option<crate::app::Page> {
    let mut new_page = None;
    ui.vertical(|ui| {
        ui.add_space(8.0);
        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new("ARXFUND")
                    .size(22.0)
                    .strong()
                    .color(theme.colors.accent_primary),
            );
            ui.label(
                RichText::new("MUSIC · STUDIO")
                    .size(10.0)
                    .color(theme.colors.text_secondary),
            );
        });
        ui.add_space(theme.spacing.lg);

        let items: &[(&str, &str, crate::app::Page)] = &[
            ("🏠", "Accueil", crate::app::Page::Home),
            ("🔍", "Recherche", crate::app::Page::Search),
            ("📚", "Bibliothèque", crate::app::Page::Library),
            ("♥", "Favoris", crate::app::Page::Favorites),
            ("+", "Playlists", crate::app::Page::Playlists),
            ("📥", "Importation", crate::app::Page::Imports),
            ("🎛", "Studio", crate::app::Page::Studio),
            ("⚙", "Paramètres", crate::app::Page::Settings),
        ];
        for (icon, label, page) in items {
            let selected = app.page == *page;
            let btn_bg = if selected {
                theme.colors.row_selected
            } else {
                Color32::TRANSPARENT
            };
            let response = ui.horizontal(|ui| {
                ui.set_min_width(ui.available_width());
                ui.add_space(theme.spacing.sm);
                ui.label(
                    RichText::new(*icon)
                        .size(16.0)
                        .color(theme.colors.text_primary),
                );
                ui.add_space(theme.spacing.sm);
                ui.label(
                    RichText::new(*label)
                        .size(13.0)
                        .strong()
                        .color(if selected {
                            theme.colors.text_primary
                        } else {
                            theme.colors.text_secondary
                        }),
                );
            });
            let response = response.response;
            let rect = response.rect;
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, theme.radius.medium, btn_bg);
            if response.clicked() {
                new_page = Some(*page);
            }
            ui.add_space(theme.spacing.xs);
        }

        ui.add_space(theme.spacing.lg);
        ui.separator();
        ui.add_space(theme.spacing.sm);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Débutant")
                    .small()
                    .color(theme.colors.text_secondary),
            );
            let mut beginner = app.studio.beginner;
            if crate::ui::components::toggle(ui, theme, &mut beginner, None).clicked() {
                app.studio.beginner = beginner;
                let _ = app
                    .db
                    .set_setting("studio.beginner", if beginner { "1" } else { "0" });
            }
        });
    });

    new_page
}

pub fn home(ui: &mut Ui, theme: &ARXFUNDTheme, app: &mut StudioApp, _ctx: &egui::Context) {
    ui.label(
        RichText::new("Accueil")
            .size(theme.typography.display.size)
            .strong()
            .color(theme.colors.text_primary),
    );
    ui.add_space(theme.spacing.md);
    if app.songs.is_empty() {
        crate::ui::components::empty_state(
            ui,
            theme,
            "📭",
            "Bibliothèque vide",
            "Importez vos fichiers audio depuis l'onglet Importation.",
        );
    } else {
        ui.label(format!("{} morceaux dans la bibliothèque", app.songs.len()));
    }
}

pub fn library(ui: &mut Ui, theme: &ARXFUNDTheme, app: &mut StudioApp, ctx: &egui::Context) {
    ui.label(
        RichText::new("Bibliothèque")
            .size(theme.typography.h1.size)
            .strong()
            .color(theme.colors.text_primary),
    );
    ui.add_space(theme.spacing.md);
    crate::ui::components::search_field(
        ui,
        theme,
        &mut app.search,
        "Rechercher un morceau, un artiste, un album…",
    );
    ui.add_space(theme.spacing.md);
    ui.label(format!("{} morceaux dans la bibliothèque", app.songs.len()));
}

pub fn playlists(ui: &mut Ui, theme: &ARXFUNDTheme, app: &mut StudioApp) {
    ui.label(
        RichText::new("Playlists")
            .size(theme.typography.h1.size)
            .strong()
            .color(theme.colors.text_primary),
    );
    ui.add_space(theme.spacing.md);
    if app.playlists.is_empty() {
        crate::ui::components::empty_state(
            ui,
            theme,
            "📋",
            "Aucune playlist",
            "Créez votre première playlist pour organiser vos morceaux.",
        );
    } else {
        for pl in &app.playlists {
            crate::ui::components::card(ui, theme, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(&pl.name)
                                .strong()
                                .size(15.0)
                                .color(theme.colors.text_primary),
                        );
                        ui.label(
                            RichText::new(format!("{} morceaux", pl.track_count))
                                .size(12.0)
                                .color(theme.colors.text_secondary),
                        );
                    });
                    ui.with_layout(
                        eframe::egui::Layout::right_to_left(eframe::egui::Align::Center),
                        |ui| {
                            if ui.button("Ouvrir").clicked() {
                                app.selected_playlist = Some(pl.id.clone());
                                if let Ok(t) = app.db.playlist_tracks(&pl.id) {
                                    app.playlist_tracks = t;
                                }
                            }
                        },
                    );
                });
            });
            ui.add_space(theme.spacing.xs);
        }
    }
}

pub fn player_bar(ui: &mut Ui, theme: &ARXFUNDTheme, app: &mut StudioApp, ctx: &egui::Context) {
    let playing = app.engine.is_playing();
    let song = app.current_song().cloned();

    ui.horizontal(|ui| {
        ui.label(
            RichText::new("ARXFUND")
                .size(12.0)
                .strong()
                .color(theme.colors.accent_primary),
        );
        ui.add_space(theme.spacing.sm);
        ui.separator();
        ui.add_space(theme.spacing.sm);

        if let Some(song) = &song {
            if let Some(cover_path) = song.cover_path.as_deref() {
                if let Some(tex) = app.get_cover_texture(ctx, cover_path) {
                    ui.image((tex.id(), Vec2::splat(52.0)));
                }
            }
            ui.add_space(theme.spacing.sm);
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(&song.title)
                        .strong()
                        .color(theme.colors.text_primary),
                );
                ui.label(song.artist.clone().unwrap_or_default());
            });
            ui.add_space(theme.spacing.md);
            ui.separator();
            ui.add_space(theme.spacing.md);

            ui.with_layout(
                eframe::egui::Layout::centered_and_justified(eframe::egui::Direction::TopDown),
                |ui| {
                    ui.horizontal(|ui| {
                        if crate::ui::components::icon_button_small(ui, theme, "⏮").clicked() {
                            app.prev_track();
                        }
                        let play_label = if playing { "⏸" } else { "▶" };
                        if crate::ui::components::button(ui, theme, play_label).clicked() {
                            app.toggle_play();
                        }
                        if crate::ui::components::icon_button_small(ui, theme, "⏭").clicked() {
                            app.next_track();
                        }
                    });
                    let dur = app.engine.duration_secs();
                    let pos = app.engine.position_secs();
                    let frac = if dur > 0.0 {
                        (pos / dur).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                    ui.add_space(theme.spacing.xs);
                    ui.add(egui::Slider::new(&mut 0.0, 0.0..=1.0).show_value(false));
                    ui.label(
                        RichText::new(format!(
                            "{} / {}",
                            crate::app::format_duration(pos),
                            crate::app::format_duration(dur)
                        ))
                        .size(11.0)
                        .color(theme.colors.text_muted),
                    );
                },
            );

            ui.add_space(theme.spacing.md);
            ui.separator();
            ui.add_space(theme.spacing.md);

            ui.with_layout(
                eframe::egui::Layout::right_to_left(eframe::egui::Align::Center),
                |ui| {
                    ui.label(
                        RichText::new("🔊")
                            .size(14.0)
                            .color(theme.colors.text_secondary),
                    );
                    let mut v = app.volume;
                    crate::ui::components::slider(ui, theme, &mut v, 0.0..=1.0, None);
                    if v != app.volume {
                        app.volume = v;
                        app.engine.set_volume(v);
                    }
                },
            );
        } else {
            ui.label(RichText::new("Aucune piste en lecture").color(theme.colors.text_muted));
        }
    });
}
