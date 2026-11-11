//! Interface du studio de production ARXFUND.
//!
//! Tous les contrôles affichés agissent réellement sur le projet ou le moteur.
//! Les fonctions non implémentées sont explicitement signalées comme telles.

use eframe::egui;
use egui::{Align2, Color32, FontId, Pos2, Rect, RichText, Sense, Stroke, Vec2};

use crate::studio::project::{ChannelKind, MAX_BPM, MAX_STEPS, MIN_BPM, Note};
use crate::studio::state::{StudioAction, StudioState};
use crate::ui::components::{audio_meter, button, button_small, icon_button, icon_button_small};
use crate::ui::theme::ARXFUNDTheme;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StudioTab {
    ChannelRack,
    PianoRoll,
    Playlist,
    Mixer,
}

impl StudioTab {
    fn label(self) -> &'static str {
        match self {
            StudioTab::ChannelRack => "Channel Rack",
            StudioTab::PianoRoll => "Piano Roll",
            StudioTab::Playlist => "Playlist",
            StudioTab::Mixer => "Mixer",
        }
    }
}

/// État d'interface propre au studio (non persistant).
pub struct StudioUiState {
    pub tab: StudioTab,
    pub selected_channel: usize,
    pub tutorial_step: usize,
    pub tutorial_completed: bool,
    pub show_shortcuts: bool,
    pub show_tutorial: bool,
    /// Note en cours de déplacement dans le Piano Roll (index dans la lane du canal sélectionné).
    pub note_drag: Option<(usize, Note, bool)>,
    pub piano_scroll: Option<f32>,
    pub clip_drag: Option<(usize, usize)>,
}

impl Default for StudioUiState {
    fn default() -> Self {
        Self {
            tab: StudioTab::ChannelRack,
            selected_channel: 0,
            tutorial_step: 0,
            tutorial_completed: false,
            show_shortcuts: false,
            show_tutorial: false,
            note_drag: None,
            piano_scroll: Some(430.0),
            clip_drag: None,
        }
    }
}

/// Rendu complet de la page Studio.
pub fn render(ui: &mut egui::Ui, state: &mut StudioState) -> Vec<StudioAction> {
    let theme = ARXFUNDTheme::dark();
    let ctx = ui.ctx().clone();
    let mut actions: Vec<StudioAction> = Vec::new();

    handle_shortcuts(&ctx, state);

    transport_bar(ui, &theme, state, &mut actions);
    ui.add_space(theme.spacing.sm);

    tabs_bar(ui, &theme, state);
    ui.add_space(theme.spacing.sm);

    egui::Panel::bottom("studio_footer")
        .resizable(false)
        .exact_size(34.0)
        .show(ui, |ui| {
            footer(ui, &theme, state, &mut actions);
        });

    egui::Panel::right("studio_export_panel")
        .resizable(true)
        .default_size(245.0)
        .min_size(200.0)
        .max_size(360.0)
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("studio_inspector")
                .show(ui, |ui| {
                    side_panel(ui, &theme, state, &mut actions);
                });
        });

    egui::CentralPanel::default().show(ui, |ui| match state.ui.tab {
        StudioTab::ChannelRack => channel_rack(ui, &theme, state),
        StudioTab::PianoRoll => piano_roll(ui, &theme, state),
        StudioTab::Playlist => playlist(ui, &theme, state),
        StudioTab::Mixer => mixer(ui, &theme, state),
    });

    help_window(&ctx, &theme, state);
    shortcuts_window(&ctx, &theme, state);
    tutorial_window(&ctx, &theme, state);

    actions
}

// ---------------------------------------------------------------------------
// Raccourcis clavier
// ---------------------------------------------------------------------------

fn handle_shortcuts(ctx: &egui::Context, state: &mut StudioState) {
    if ctx.text_edit_focused() {
        return;
    }
    let command = ctx.input(|i| i.modifiers.command);
    if command && ctx.input(|i| i.key_pressed(egui::Key::S)) {
        state.save();
    }
    if command && ctx.input(|i| i.key_pressed(egui::Key::N)) {
        state.new_project();
    }
    if command && ctx.input(|i| i.key_pressed(egui::Key::O)) {
        if let Some(path) = rfd::FileDialog::new()
            .set_directory(&state.project_dir)
            .add_filter("Projet ARXFUND", &["rmsproj"])
            .pick_file()
        {
            state.open(path);
        }
    }
    if ctx.input(|i| i.key_pressed(egui::Key::F1)) {
        state.show_help = true;
    }
}

// ---------------------------------------------------------------------------
// Barre de transport
// ---------------------------------------------------------------------------

fn transport_bar(
    ui: &mut egui::Ui,
    theme: &ARXFUNDTheme,
    state: &mut StudioState,
    actions: &mut Vec<StudioAction>,
) {
    egui::Frame::new()
        .fill(theme.colors.bg_panel)
        .corner_radius(theme.radius.large)
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                let playing = state.is_playing();
                let play_label = if playing { "⏸" } else { "▶" };
                if icon_button(ui, theme, play_label, 34.0)
                    .on_hover_text("Lecture / pause (Espace)")
                    .clicked()
                {
                    if playing {
                        state.pause_playback();
                    } else {
                        state.play();
                        actions.push(StudioAction::PauseMusic);
                    }
                }
                if icon_button(ui, theme, "⏹", 34.0)
                    .on_hover_text("Stop et retour au début")
                    .clicked()
                {
                    state.stop();
                }
                if icon_button(ui, theme, "⏮", 30.0)
                    .on_hover_text("Revenir au début de la mesure")
                    .clicked()
                {
                    let step = state.playhead_step();
                    let bar = state.project.bar_steps().max(1);
                    let target = (step / bar).saturating_sub(1) * bar;
                    state.seek_step(target as u64);
                }

                ui.add_space(theme.spacing.sm);
                ui.label(
                    RichText::new(state.position_label())
                        .font(FontId::monospace(20.0))
                        .strong()
                        .color(theme.colors.accent_secondary),
                );
                ui.label(
                    RichText::new("mesure:temps:pas")
                        .small()
                        .color(theme.colors.text_muted),
                );

                ui.add_space(theme.spacing.md);
                ui.label(RichText::new("BPM").small().color(theme.colors.text_secondary));
                let bpm_resp = ui.add(
                    egui::DragValue::new(&mut state.project.bpm)
                        .range(MIN_BPM..=MAX_BPM)
                        .speed(0.5),
                );
                if bpm_resp.changed() {
                    state.mark_dirty();
                }
                bpm_resp.on_hover_text("Tempo du projet (40 à 300 BPM). Glisser ou double-cliquer pour saisir.");

                ui.add_space(theme.spacing.sm);
                ui.label(RichText::new("Signature").small().color(theme.colors.text_secondary));
                let mut num = state.project.time_sig_num;
                if ui.add(egui::DragValue::new(&mut num).range(1..=16)).changed() {
                    state.project.time_sig_num = num;
                    state.mark_dirty();
                }
                ui.label(RichText::new("/").color(theme.colors.text_muted));
                let mut den = state.project.time_sig_den;
                egui::ComboBox::from_id_salt("time_sig_den")
                    .selected_text(format!("{den}"))
                    .show_ui(ui, |ui| {
                        for v in [2u8, 4, 8, 16] {
                            ui.selectable_value(&mut den, v, format!("{v}"));
                        }
                    });
                if den != state.project.time_sig_den {
                    state.project.time_sig_den = den;
                    state.mark_dirty();
                }

                ui.add_space(theme.spacing.sm);
                ui.label(RichText::new("Swing").small().color(theme.colors.text_secondary));
                if ui
                    .add(
                        egui::DragValue::new(&mut state.project.swing)
                            .range(0.0..=0.7)
                            .speed(0.01),
                    )
                    .on_hover_text("Décalage des pas impairs (0 à 0,7).")
                    .changed()
                {
                    state.mark_dirty();
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let mut beginner = state.beginner;
                    if ui
                        .checkbox(&mut beginner, "Mode débutant")
                        .on_hover_text(
                            "Simplifie uniquement la présentation. Toutes les fonctions restent disponibles.",
                        )
                        .changed()
                    {
                        state.beginner = beginner;
                        actions.push(StudioAction::PersistBeginner(beginner));
                    }
                    if icon_button_small(ui, theme, "?")
                        .on_hover_text("Aide contextuelle (F1)")
                        .clicked()
                    {
                        state.show_help = true;
                    }
                    if icon_button_small(ui, theme, "⌨")
                        .on_hover_text("Raccourcis clavier")
                        .clicked()
                    {
                        state.ui.show_shortcuts = true;
                    }
                    if icon_button_small(ui, theme, "🎓")
                        .on_hover_text("Tutoriel « Mon premier beat »")
                        .clicked()
                    {
                        state.ui.show_tutorial = true;
                    }
                });
            });

            ui.horizontal(|ui| {
                if button_small(ui, theme, "Enregistrer").clicked() {
                    state.save();
                }
                if button_small(ui, theme, "Enregistrer sous…").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .set_directory(&state.project_dir)
                        .set_file_name(format!("{}.rmsproj", state.project.id))
                        .add_filter("Projet ARXFUND", &["rmsproj"])
                        .save_file()
                    {
                        state.save_as(path);
                    }
                }
                if button_small(ui, theme, "Nouveau").clicked() {
                    state.new_project();
                }
                if button_small(ui, theme, "Ouvrir…").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .set_directory(&state.project_dir)
                        .add_filter("Projet ARXFUND", &["rmsproj"])
                        .pick_file()
                    {
                        state.open(path);
                    }
                }
                ui.add_space(theme.spacing.sm);
                if ui
                    .add_enabled(!state.undo.is_empty(), egui::Button::new("↶ Annuler"))
                    .on_hover_text("Ctrl+Z")
                    .clicked()
                {
                    state.undo();
                }
                if ui
                    .add_enabled(!state.redo.is_empty(), egui::Button::new("↷ Rétablir"))
                    .on_hover_text("Ctrl+Y")
                    .clicked()
                {
                    state.redo();
                }
                ui.add_space(theme.spacing.sm);
                let mut metronome = state.metronome;
                if ui
                    .checkbox(&mut metronome, "Métronome")
                    .on_hover_text("Clic de tempo pendant la lecture.")
                    .changed()
                {
                    state.metronome = metronome;
                    if let Some(e) = &state.engine {
                        e.set_metronome(metronome);
                    }
                }
                let mut song = state.project.song_mode;
                if ui
                    .selectable_label(song, "Mode Song")
                    .on_hover_text("Lit l'arrangement au lieu de boucler le pattern actif.")
                    .clicked()
                {
                    song = !song;
                    state.project.song_mode = song;
                    state.mark_dirty();
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!(
                            "{}  ·  {}",
                            state.project.name,
                            if state.dirty { "modifié" } else { "enregistré" }
                        ))
                        .small()
                        .color(if state.dirty {
                            theme.colors.warning
                        } else {
                            theme.colors.text_muted
                        }),
                    );
                });
            });
        });
}

// ---------------------------------------------------------------------------
// Onglets
// ---------------------------------------------------------------------------

fn tabs_bar(ui: &mut egui::Ui, theme: &ARXFUNDTheme, state: &mut StudioState) {
    ui.horizontal(|ui| {
        for tab in [
            StudioTab::ChannelRack,
            StudioTab::PianoRoll,
            StudioTab::Playlist,
            StudioTab::Mixer,
        ] {
            let selected = state.ui.tab == tab;
            let text = RichText::new(tab.label()).color(if selected {
                theme.colors.text_primary
            } else {
                theme.colors.text_secondary
            });
            if ui.selectable_label(selected, text).clicked() {
                state.ui.tab = tab;
            }
        }
    });
}

// ---------------------------------------------------------------------------
// Channel Rack
// ---------------------------------------------------------------------------

fn channel_rack(ui: &mut egui::Ui, theme: &ARXFUNDTheme, state: &mut StudioState) {
    ui.horizontal_wrapped(|ui| {
        ui.label(
            RichText::new("Pattern")
                .small()
                .color(theme.colors.text_secondary),
        );
        let names: Vec<String> = state
            .project
            .patterns
            .iter()
            .map(|p| p.name.clone())
            .collect();
        let mut active = state.project.active_pattern;
        egui::ComboBox::from_id_salt("active_pattern")
            .selected_text(names.get(active).cloned().unwrap_or_default())
            .show_ui(ui, |ui| {
                for (i, name) in names.iter().enumerate() {
                    ui.selectable_value(&mut active, i, name);
                }
            });
        if active != state.project.active_pattern {
            state.project.active_pattern = active;
            state.mark_dirty();
        }
        if button_small(ui, theme, "+ Pattern").clicked() {
            state.push_undo();
            state.project.add_pattern();
            state.mark_dirty();
        }
        if button_small(ui, theme, "Dupliquer").clicked() {
            state.push_undo();
            let index = state.project.active_pattern;
            if let Some(new_index) = state.project.duplicate_pattern(index) {
                state.project.active_pattern = new_index;
            }
            state.mark_dirty();
        }
        if ui
            .add_enabled(
                state.project.patterns.len() > 1,
                egui::Button::new("Supprimer pattern"),
            )
            .clicked()
        {
            state.push_undo();
            let index = state.project.active_pattern;
            state.project.delete_pattern(index);
            state.mark_dirty();
        }

        ui.add_space(theme.spacing.md);
        ui.label(
            RichText::new("Pas")
                .small()
                .color(theme.colors.text_secondary),
        );
        let mut steps = state.project.steps;
        egui::ComboBox::from_id_salt("step_count")
            .selected_text(format!("{steps}"))
            .show_ui(ui, |ui| {
                for value in [16usize, 32, 48, 64] {
                    ui.selectable_value(&mut steps, value, format!("{value}"));
                }
            });
        if steps != state.project.steps {
            state.push_undo();
            state.project.resize_steps(steps.min(MAX_STEPS));
            state.mark_dirty();
        }

        ui.add_space(theme.spacing.md);
        ui.label(
            RichText::new("Ajouter")
                .small()
                .color(theme.colors.text_secondary),
        );
        let current_kind = state.pending_channel_kind;
        egui::ComboBox::from_id_salt("channel_kind")
            .selected_text(current_kind.label())
            .show_ui(ui, |ui| {
                for kind in ChannelKind::ALL {
                    ui.selectable_value(&mut state.pending_channel_kind, kind, kind.label());
                }
            });
        if button_small(ui, theme, "Canal +").clicked() {
            state.push_undo();
            state.project.add_channel(state.pending_channel_kind, None);
            state.mark_dirty();
        }
    });

    ui.add_space(theme.spacing.sm);
    let playhead = state.playhead_step();
    let steps = state.project.steps;
    let channels = state.project.channels.len();

    egui::ScrollArea::both()
        .auto_shrink([false, false])
        .max_height(ui.available_height() - 170.0)
        .show(ui, |ui| {
            egui::Grid::new("channel_rack_grid").min_col_width(24.0)
                .spacing(Vec2::new(2.0, 2.0))
                .show(ui, |ui| {
                    ui.label("");
                    for s in 0..steps {
                        let color = if s % 16 == 0 {
                            theme.colors.accent_secondary
                        } else if s % 4 == 0 {
                            theme.colors.text_secondary
                        } else {
                            theme.colors.text_muted
                        };
                        ui.label(
                            RichText::new(format!("{}", s + 1))
                                .font(FontId::monospace(11.0))
                                .color(color),
                        );
                    }
                    ui.end_row();

                    for i in 0..channels {
                        let (name, color, mute, solo) = {
                            let ch = &state.project.channels[i];
                            (ch.name.clone(), ch.color, ch.mute, ch.solo)
                        };
                        ui.horizontal(|ui| {
                            let (rect, _) = ui.allocate_exact_size(Vec2::new(10.0, 18.0), Sense::hover());
                            ui.painter()
                                .rect_filled(rect, 2.0, Color32::from_rgb(color[0], color[1], color[2]));
                            let selected = state.ui.selected_channel == i;
                            if ui
                                .selectable_label(
                                    selected,
                                    RichText::new(&name)
                                        .small()
                                        .color(if selected {
                                            theme.colors.text_primary
                                        } else {
                                            theme.colors.text_secondary
                                        }),
                                )
                                .clicked()
                            {
                                state.ui.selected_channel = i;
                            }
                            if ui
                                .selectable_label(mute, RichText::new("M").small())
                                .on_hover_text("Muet")
                                .clicked()
                            {
                                state.push_undo();
                                state.project.channels[i].mute = !mute;
                                state.mark_dirty();
                            }
                            if ui
                                .selectable_label(solo, RichText::new("S").small())
                                .on_hover_text("Solo")
                                .clicked()
                            {
                                state.push_undo();
                                state.project.channels[i].solo = !solo;
                                state.mark_dirty();
                            }
                        });

                        for s in 0..steps {
                            let step = state.project.step(i, s).unwrap_or_default();
                            let (rect, resp) = ui.allocate_exact_size(
                                Vec2::splat(20.0),
                                Sense::click_and_drag(),
                            );
                            let painter = ui.painter_at(rect);
                            let bg = if step.on {
                                if step.velocity > 0.9 {
                                    theme.colors.accent_primary
                                } else {
                                    Color32::from_rgb(110, 76, 200)
                                }
                            } else if s == playhead {
                                theme.colors.row_selected
                            } else if s % 4 == 0 {
                                theme.colors.bg_surface
                            } else {
                                theme.colors.step_off
                            };
                            painter.rect_filled(rect, 3.0, bg);
                            if s == playhead {
                                painter.rect_stroke(
                                    rect,
                                    3.0,
                                    Stroke::new(1.5, theme.colors.accent_secondary),
                                    egui::StrokeKind::Inside,
                                );
                            }

                            if resp.drag_started() {
                                state.push_undo();
                                state.drag_paint = Some((i, !step.on));
                                if let Some(st) = state.project.step_mut(i, s) {
                                    st.on = !step.on;
                                }
                                state.mark_dirty();
                            } else if resp.clicked() {
                                state.push_undo();
                                if let Some(st) = state.project.step_mut(i, s) {
                                    st.on = !step.on;
                                }
                                state.mark_dirty();
                            } else if resp.secondary_clicked() {
                                state.push_undo();
                                if let Some(st) = state.project.step_mut(i, s) {
                                    st.on = true;
                                    st.velocity = 1.0;
                                }
                                state.mark_dirty();
                            } else if ui.input(|inp| inp.pointer.primary_down()) {
                                if let Some((lane, value)) = state.drag_paint {
                                    let over = ui.input(|inp| {
                                        inp.pointer
                                            .interact_pos()
                                            .map(|p| rect.contains(p))
                                            .unwrap_or(false)
                                    });
                                    if lane == i && over {
                                        if let Some(st) = state.project.step_mut(i, s) {
                                            st.on = value;
                                        }
                                        state.mark_dirty();
                                    }
                                }
                            }
                            resp.on_hover_text(format!(
                                "Canal {} · pas {}\nClic : activer/désactiver\nClic droit : accent\nGlisser : peindre",
                                name,
                                s + 1
                            ));
                        }
                        ui.end_row();
                    }
                });
        });

    if state.drag_paint.is_some() && !ui.input(|inp| inp.pointer.primary_down()) {
        state.drag_paint = None;
    }

    ui.separator();
    channel_inspector(ui, theme, state);
}

fn channel_inspector(ui: &mut egui::Ui, theme: &ARXFUNDTheme, state: &mut StudioState) {
    let index = state
        .ui
        .selected_channel
        .min(state.project.channels.len().saturating_sub(1));
    state.ui.selected_channel = index;

    let Some(channel) = state.project.channels.get(index) else {
        return;
    };
    let kind = channel.kind;
    let mut name = channel.name.clone();
    let mut volume = channel.volume;
    let mut pan = channel.pan;
    let mut pitch = channel.pitch;
    let mut changed = false;
    let mut load_sample = false;
    let mut delete_channel = false;
    let can_delete = state.project.channels.len() > 1;

    ui.horizontal_wrapped(|ui| {
        ui.label(
            RichText::new("Canal sélectionné :")
                .small()
                .color(theme.colors.text_secondary),
        );
        if ui
            .add(egui::TextEdit::singleline(&mut name).desired_width(140.0))
            .changed()
        {
            changed = true;
        }
        ui.label(
            RichText::new(kind.label())
                .small()
                .color(theme.colors.accent_secondary),
        );

        ui.add_space(theme.spacing.sm);
        ui.label(
            RichText::new("Gain")
                .small()
                .color(theme.colors.text_secondary),
        );
        if ui
            .add(egui::Slider::new(&mut volume, 0.0..=1.5).show_value(true))
            .on_hover_text("Gain du canal (0 à 150 %).")
            .changed()
        {
            changed = true;
        }
        ui.label(
            RichText::new("Pan")
                .small()
                .color(theme.colors.text_secondary),
        );
        if ui
            .add(egui::Slider::new(&mut pan, -1.0..=1.0).show_value(true))
            .on_hover_text("Position stéréo : -1 gauche, 0 centre, +1 droite.")
            .changed()
        {
            changed = true;
        }
        ui.label(
            RichText::new("Pitch")
                .small()
                .color(theme.colors.text_secondary),
        );
        if ui
            .add(egui::Slider::new(&mut pitch, -24.0..=24.0).show_value(true))
            .on_hover_text("Transposition du canal en demi-tons.")
            .changed()
        {
            changed = true;
        }

        if kind == ChannelKind::Sample
            && button_small(ui, theme, "Charger un échantillon…").clicked()
        {
            load_sample = true;
        }

        if ui
            .add_enabled(can_delete, egui::Button::new("Supprimer le canal"))
            .clicked()
        {
            delete_channel = true;
        }
    });

    if changed {
        if let Some(ch) = state.project.channels.get_mut(index) {
            ch.name = name;
            ch.volume = volume;
            ch.pan = pan;
            ch.pitch = pitch;
        }
        state.mark_dirty();
    }

    if load_sample {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Audio", &["wav", "mp3", "flac", "ogg", "m4a"])
            .pick_file()
        {
            state.assign_sample(index, path);
        }
    }

    if delete_channel {
        state.push_undo();
        state.project.remove_channel(index);
        state.ui.selected_channel = index.saturating_sub(1);
        state.mark_dirty();
    }
}

// ---------------------------------------------------------------------------
// Piano Roll
// ---------------------------------------------------------------------------

fn note_name(pitch: u8) -> String {
    const NAMES: &[&str] = &[
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let octave = (pitch as i32 / 12) - 1;
    format!("{}{}", NAMES[pitch as usize % 12], octave)
}

fn piano_roll(ui: &mut egui::Ui, theme: &ARXFUNDTheme, state: &mut StudioState) {
    ui.horizontal_wrapped(|ui| {
        ui.heading("Piano Roll");
        egui::ComboBox::from_id_salt("piano_channel")
            .selected_text(
                state
                    .project
                    .channels
                    .get(state.ui.selected_channel)
                    .map(|c| c.name.as_str())
                    .unwrap_or("Instrument"),
            )
            .show_ui(ui, |ui| {
                for (i, ch) in state.project.channels.iter().enumerate() {
                    if ch.kind.is_melodic() {
                        ui.selectable_value(&mut state.ui.selected_channel, i, &ch.name);
                    }
                }
            });
        ui.label("Glisser : déplacer · Maj + glisser : durée · Clic droit : supprimer");
    });
    let ci = state.ui.selected_channel;
    let Some(channel) = state.project.channels.get(ci) else {
        return;
    };
    if !channel.kind.is_melodic() {
        ui.label("Choisissez Basse 808 ou Synthé dans le menu Instrument.");
        return;
    }
    let c = channel.color;
    let color = Color32::from_rgb(c[0], c[1], c[2]);
    let steps = state.project.steps;
    let step_w = ((ui.available_width() - 64.0) / steps as f32).max(24.0);
    const ROW: f32 = 18.0;
    const LOW: u8 = 24;
    const HIGH: u8 = 96;
    ui.add_space(8.0);
    let mut scroll = egui::ScrollArea::both()
        .id_salt("piano_grid")
        .auto_shrink([false, false]);
    if let Some(offset) = state.ui.piano_scroll.take() {
        scroll = scroll.vertical_scroll_offset(offset);
    }
    scroll.show(ui, |ui| {
        let (whole, response) = ui.allocate_exact_size(
            Vec2::new(
                64.0 + steps as f32 * step_w,
                (HIGH - LOW + 1) as f32 * ROW + 24.0,
            ),
            Sense::click_and_drag(),
        );
        let grid = Rect::from_min_max(whole.min + Vec2::new(64.0, 24.0), whole.max);
        let painter = ui.painter_at(whole);
        painter.rect_filled(whole, 4.0, theme.colors.bg_secondary);
        for pitch in LOW..=HIGH {
            let y = grid.top() + (HIGH - pitch) as f32 * ROW;
            let black = matches!(pitch % 12, 1 | 3 | 6 | 8 | 10);
            let row = Rect::from_min_size(Pos2::new(grid.left(), y), Vec2::new(grid.width(), ROW));
            painter.rect_filled(
                row,
                0.0,
                if black {
                    theme.colors.bg_primary
                } else {
                    theme.colors.bg_secondary
                },
            );
            let key = Rect::from_min_size(Pos2::new(whole.left(), y), Vec2::new(60.0, ROW - 1.0));
            painter.rect_filled(
                key,
                1.0,
                if black {
                    theme.colors.bg_surface
                } else {
                    Color32::from_rgb(190, 199, 214)
                },
            );
            if pitch % 12 == 0 {
                painter.text(
                    key.center(),
                    Align2::CENTER_CENTER,
                    note_name(pitch),
                    FontId::proportional(11.0),
                    theme.colors.bg_primary,
                );
            }
        }
        for step in 0..=steps {
            let x = grid.left() + step as f32 * step_w;
            painter.line_segment(
                [Pos2::new(x, grid.top()), Pos2::new(x, grid.bottom())],
                Stroke::new(
                    if step % 4 == 0 { 1.3 } else { 0.5 },
                    theme.colors.separator,
                ),
            );
            if step % 4 == 0 {
                painter.text(
                    Pos2::new(x + 4.0, whole.top() + 5.0),
                    Align2::LEFT_TOP,
                    format!("{}", step / 4 + 1),
                    FontId::proportional(11.0),
                    theme.colors.text_secondary,
                );
            }
        }
        let note_rect = |n: &Note| {
            Rect::from_min_size(
                Pos2::new(
                    grid.left() + n.start as f32 * step_w,
                    grid.top() + (HIGH as f32 - n.pitch as f32) * ROW,
                ),
                Vec2::new(n.length as f32 * step_w - 2.0, ROW - 2.0),
            )
        };
        for note in state.project.notes(ci) {
            let rect = note_rect(note);
            painter.rect_filled(rect, 3.0, color);
            painter.text(
                rect.left_center() + Vec2::new(4.0, 0.0),
                Align2::LEFT_CENTER,
                note_name(note.pitch),
                FontId::proportional(11.0),
                theme.colors.bg_primary,
            );
        }
        if state.is_playing() {
            let x = grid.left() + state.playhead_step() as f32 * step_w;
            painter.line_segment(
                [Pos2::new(x, grid.top()), Pos2::new(x, grid.bottom())],
                Stroke::new(2.0, theme.colors.accent_secondary),
            );
        }
        if let Some(pos) = response
            .interact_pointer_pos()
            .filter(|p| grid.contains(*p))
        {
            let step = ((pos.x - grid.left()) / step_w).floor() as usize;
            let pitch = HIGH
                .saturating_sub(((pos.y - grid.top()) / ROW).floor() as u8)
                .max(LOW);
            let hit = state
                .project
                .notes(ci)
                .iter()
                .position(|n| note_rect(n).contains(pos));
            if response.secondary_clicked() {
                if let Some(i) = hit {
                    state.push_undo();
                    state.project.notes_mut(ci).remove(i);
                    state.mark_dirty();
                }
            } else if response.clicked() && hit.is_none() {
                state.push_undo();
                state.project.notes_mut(ci).push(Note {
                    pitch,
                    start: step.min(steps - 1),
                    length: 1,
                    velocity: 0.85,
                });
                state.mark_dirty();
            }
        }
        if response.drag_started() {
            if let Some(origin) = ui.input(|i| i.pointer.press_origin()) {
                if let Some(i) = state
                    .project
                    .notes(ci)
                    .iter()
                    .position(|n| note_rect(n).contains(origin))
                {
                    let note = state.project.notes(ci)[i];
                    state.push_undo();
                    state.ui.note_drag = Some((i, note, ui.input(|i| i.modifiers.shift)));
                }
            }
        }
        if response.dragged() {
            if let Some((i, original, resize)) = state.ui.note_drag {
                let dx =
                    (response.total_drag_delta().unwrap_or_default().x / step_w).round() as isize;
                let dy = (response.total_drag_delta().unwrap_or_default().y / ROW).round() as i16;
                if let Some(note) = state.project.notes_mut(ci).get_mut(i) {
                    if resize {
                        note.length = (original.length as isize + dx)
                            .clamp(1, (steps - note.start) as isize)
                            as usize;
                    } else {
                        note.start = (original.start as isize + dx)
                            .clamp(0, steps.saturating_sub(original.length) as isize)
                            as usize;
                        note.pitch =
                            (original.pitch as i16 - dy).clamp(LOW as i16, HIGH as i16) as u8;
                    }
                }
                state.mark_dirty();
            }
        }
        if response.drag_stopped() {
            state.ui.note_drag = None;
        }
    });
    // ScrollArea persists its own offset after the initial framing.
}

// ---------------------------------------------------------------------------
// Playlist / arrangement
// ---------------------------------------------------------------------------

fn playlist(ui: &mut egui::Ui, theme: &ARXFUNDTheme, state: &mut StudioState) {
    ui.horizontal_wrapped(|ui| {
        let mut song = state.project.song_mode;
        if ui
            .checkbox(&mut song, "Mode Song (lire l'arrangement)")
            .changed()
        {
            state.project.song_mode = song;
            state.mark_dirty();
        }
        if button_small(ui, theme, "+ Clip au playhead").clicked() {
            state.push_undo();
            let start = state.playhead_step();
            let length = state.project.steps;
            let pattern = state.project.active_pattern;
            state
                .project
                .arrangement
                .push(crate::studio::project::Clip {
                    pattern,
                    start_step: start,
                    length_steps: length,
                });
            state.mark_dirty();
        }
        ui.label(
            RichText::new(format!(
                "Longueur totale : {} pas",
                state.project.song_length_steps()
            ))
            .small()
            .color(theme.colors.text_secondary),
        );
    });

    ui.add_space(theme.spacing.sm);
    let total = state.project.song_length_steps();
    let bar_steps = state.project.bar_steps().max(1);
    let bars = total.div_ceil(bar_steps);
    let mut remove = None;
    let mut changed = false;

    egui::Grid::new("playlist_clips")
        .num_columns(5)
        .spacing(Vec2::new(10.0, 4.0))
        .show(ui, |ui| {
            ui.label(RichText::new("Clip").strong());
            ui.label(RichText::new("Pattern").strong());
            ui.label(RichText::new("Début (pas)").strong());
            ui.label(RichText::new("Longueur (pas)").strong());
            ui.label(RichText::new("Action").strong());
            ui.end_row();

            for (i, clip) in state.project.arrangement.iter_mut().enumerate() {
                ui.label(format!("{}", i + 1));

                let names: Vec<String> = state
                    .project
                    .patterns
                    .iter()
                    .map(|p| p.name.clone())
                    .collect();
                let mut pattern = clip.pattern;
                egui::ComboBox::from_id_salt(("clip_pattern", i))
                    .selected_text(names.get(pattern).cloned().unwrap_or_default())
                    .show_ui(ui, |ui| {
                        for (pi, name) in names.iter().enumerate() {
                            ui.selectable_value(&mut pattern, pi, name);
                        }
                    });
                if pattern != clip.pattern {
                    clip.pattern = pattern;
                    changed = true;
                }

                let mut start = clip.start_step;
                if ui
                    .add(egui::DragValue::new(&mut start).range(0..=total.max(64)))
                    .changed()
                {
                    clip.start_step = start;
                    changed = true;
                }
                let mut length = clip.length_steps;
                if ui
                    .add(
                        egui::DragValue::new(&mut length)
                            .range(1..=64usize.max(state.project.steps)),
                    )
                    .changed()
                {
                    clip.length_steps = length;
                    changed = true;
                }
                if ui.button("Retirer").clicked() {
                    remove = Some(i);
                }
                ui.end_row();
            }
        });

    if changed {
        state.mark_dirty();
    }

    if let Some(i) = remove {
        if state.project.arrangement.len() > 1 {
            state.push_undo();
            state.project.arrangement.remove(i);
            state.mark_dirty();
        }
    }

    ui.add_space(theme.spacing.sm);
    ui.label(
        RichText::new(format!("{} mesure(s) affichée(s) dans la timeline", bars))
            .small()
            .color(theme.colors.text_muted),
    );

    ui.label("Glissez un clip pour le déplacer. Les clips superposés sont mixés ensemble.");
    let timeline_steps = total.max(64) + 16;
    let px_per_step = 14.0;
    egui::ScrollArea::both()
        .id_salt("arrangement_timeline")
        .show(ui, |ui| {
            let (rect, response) = ui.allocate_exact_size(
                Vec2::new(
                    timeline_steps as f32 * px_per_step,
                    40.0 + state.project.arrangement.len().max(3) as f32 * 48.0,
                ),
                Sense::click_and_drag(),
            );
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 4.0, theme.colors.bg_secondary);
            for step in (0..timeline_steps).step_by(4) {
                let x = rect.left() + step as f32 * px_per_step;
                painter.line_segment(
                    [Pos2::new(x, rect.top() + 24.0), Pos2::new(x, rect.bottom())],
                    Stroke::new(1.0, theme.colors.separator),
                );
                painter.text(
                    Pos2::new(x + 3.0, rect.top() + 4.0),
                    Align2::LEFT_TOP,
                    format!("{}", step + 1),
                    FontId::proportional(11.0),
                    theme.colors.text_secondary,
                );
            }
            let clip_rect = |i: usize, start: usize, length: usize| {
                Rect::from_min_size(
                    Pos2::new(
                        rect.left() + start as f32 * px_per_step,
                        rect.top() + 30.0 + i as f32 * 48.0,
                    ),
                    Vec2::new(length as f32 * px_per_step - 2.0, 38.0),
                )
            };
            for (i, clip) in state.project.arrangement.iter().enumerate() {
                let r = clip_rect(i, clip.start_step, clip.length_steps);
                painter.rect_filled(r, 5.0, theme.colors.accent_primary);
                painter.text(
                    r.left_center() + Vec2::new(8.0, 0.0),
                    Align2::LEFT_CENTER,
                    &state.project.patterns[clip.pattern].name,
                    FontId::proportional(12.0),
                    Color32::WHITE,
                );
            }
            if response.drag_started() {
                if let Some(pos) = ui.input(|i| i.pointer.press_origin()) {
                    let hit = state
                        .project
                        .arrangement
                        .iter()
                        .enumerate()
                        .find(|(i, c)| clip_rect(*i, c.start_step, c.length_steps).contains(pos))
                        .map(|(i, c)| (i, c.start_step));
                    if let Some(hit) = hit {
                        state.push_undo();
                        state.ui.clip_drag = Some(hit);
                    }
                }
            }
            if response.dragged() {
                if let Some((i, start)) = state.ui.clip_drag {
                    state.project.arrangement[i].start_step = (start as isize
                        + (response.total_drag_delta().unwrap_or_default().x / px_per_step).round()
                            as isize)
                        .max(0)
                        as usize;
                    state.mark_dirty();
                }
            }
            if response.drag_stopped() {
                state.ui.clip_drag = None;
            }
        });
}

// ---------------------------------------------------------------------------
// Mixer
// ---------------------------------------------------------------------------

fn mixer(ui: &mut egui::Ui, theme: &ARXFUNDTheme, state: &mut StudioState) {
    ui.heading("Mixer");
    ui.label("Panoramique et écho stéréo par canal · Protection douce du master active");
    ui.add_space(12.0);
    egui::ScrollArea::both()
        .id_salt("mixer_strips")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                for i in 0..=state.project.channels.len() {
                    let master = i == state.project.channels.len();
                    let name = if master {
                        "Master".to_string()
                    } else {
                        state.project.channels[i].name.clone()
                    };
                    let color = if master {
                        theme.colors.accent_secondary
                    } else {
                        let c = state.project.channels[i].color;
                        Color32::from_rgb(c[0], c[1], c[2])
                    };
                    let level = state
                        .engine
                        .as_ref()
                        .map(|e| {
                            if master {
                                e.master_level()
                            } else {
                                e.channel_level(i)
                            }
                        })
                        .unwrap_or(0.0);
                    egui::Frame::new()
                        .fill(theme.colors.bg_panel)
                        .corner_radius(6.0)
                        .inner_margin(10)
                        .show(ui, |ui| {
                            ui.vertical(|ui| {
                                ui.set_width(110.0);
                                ui.label(RichText::new(name).strong().color(color));
                                let mut volume = if master {
                                    state.project.master_volume
                                } else {
                                    state.project.channels[i].volume
                                };
                                ui.allocate_ui(Vec2::new(110.0, 185.0), |ui| {
                                    ui.horizontal_top(|ui| {
                                        ui.spacing_mut().slider_width = 175.0;
                                        if ui
                                            .add(
                                                egui::Slider::new(&mut volume, 0.0..=1.5)
                                                    .vertical()
                                                    .show_value(false),
                                            )
                                            .changed()
                                        {
                                            if master {
                                                state.project.master_volume = volume;
                                            } else {
                                                state.project.channels[i].volume = volume;
                                            }
                                            state.mark_dirty();
                                        }
                                        audio_meter(ui, theme, level, level);
                                    });
                                });
                                ui.label(format!("{:.0} %", volume * 100.0));
                                if !master {
                                    ui.horizontal(|ui| {
                                        let mute = state.project.channels[i].mute;
                                        let solo = state.project.channels[i].solo;
                                        if ui.selectable_label(mute, "Muet").clicked() {
                                            state.push_undo();
                                            state.project.channels[i].mute = !mute;
                                            state.mark_dirty();
                                        }
                                        if ui.selectable_label(solo, "Solo").clicked() {
                                            state.push_undo();
                                            state.project.channels[i].solo = !solo;
                                            state.mark_dirty();
                                        }
                                    });
                                    ui.spacing_mut().slider_width = 100.0;
                                    ui.label("Panoramique");
                                    let pan = ui.add(
                                        egui::Slider::new(
                                            &mut state.project.channels[i].pan,
                                            -1.0..=1.0,
                                        )
                                        .show_value(false),
                                    );
                                    ui.label("Écho · 300 ms");
                                    let delay = ui.add(
                                        egui::Slider::new(
                                            &mut state.project.channels[i].delay_mix,
                                            0.0..=0.8,
                                        )
                                        .show_value(false),
                                    );
                                    if pan.changed() || delay.changed() {
                                        state.mark_dirty();
                                    }
                                } else {
                                    ui.label(
                                        RichText::new("Sortie stéréo")
                                            .color(theme.colors.text_secondary),
                                    );
                                    ui.label("EQ, compresseur et réverbération : non disponibles.");
                                }
                            });
                        });
                }
            });
        });
}

// ---------------------------------------------------------------------------
// Panneau latéral : projets, export, état
// ---------------------------------------------------------------------------

fn side_panel(
    ui: &mut egui::Ui,
    theme: &ARXFUNDTheme,
    state: &mut StudioState,
    actions: &mut Vec<StudioAction>,
) {
    ui.add_space(theme.spacing.xs);
    ui.label(
        RichText::new("PROJET")
            .small()
            .strong()
            .color(theme.colors.text_secondary),
    );
    ui.add_space(4.0);
    if ui
        .add(egui::TextEdit::singleline(&mut state.project.name).desired_width(f32::INFINITY))
        .changed()
    {
        state.mark_dirty();
    }
    ui.label(
        RichText::new(format!(
            "{} canaux · {} patterns · {} pas",
            state.project.channels.len(),
            state.project.patterns.len(),
            state.project.steps
        ))
        .small()
        .color(theme.colors.text_muted),
    );

    if let Some(recovery) = state.recovery_available.clone() {
        ui.add_space(theme.spacing.sm);
        egui::Frame::new()
            .fill(theme.colors.bg_surface)
            .corner_radius(theme.radius.medium)
            .inner_margin(egui::Margin::same(8))
            .show(ui, |ui| {
                ui.label(
                    RichText::new("Sauvegarde automatique disponible")
                        .small()
                        .strong()
                        .color(theme.colors.warning),
                );
                if button_small(ui, theme, "Récupérer").clicked() {
                    state.open(recovery.clone());
                }
            });
    }

    ui.add_space(theme.spacing.sm);
    if button(ui, theme, "🌱 Charger le beat de démonstration").clicked() {
        state.load_demo_beat();
    }
    if button_small(ui, theme, "Vider le pattern actif").clicked() {
        state.clear_pattern();
    }

    ui.add_space(theme.spacing.md);
    ui.separator();
    ui.label(
        RichText::new("EXPORT WAV")
            .small()
            .strong()
            .color(theme.colors.text_secondary),
    );
    ui.add_space(4.0);

    ui.horizontal(|ui| {
        ui.label(RichText::new("Mesures").small());
        ui.add(egui::DragValue::new(&mut state.export_bars).range(1..=64));
    });
    ui.horizontal(|ui| {
        ui.label(RichText::new("Résolution").small());
        egui::ComboBox::from_id_salt("bit_depth")
            .selected_text(format!("{} bits", state.export_bit_depth))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut state.export_bit_depth, 16, "16 bits");
                ui.selectable_value(&mut state.export_bit_depth, 24, "24 bits");
            });
    });
    ui.checkbox(&mut state.export_normalize, "Normaliser le pic");
    ui.checkbox(&mut state.export_add_library, "Ajouter à la bibliothèque");

    let exporting = state.export_rx.is_some();
    if ui
        .add_enabled(!exporting, egui::Button::new("Exporter en WAV…"))
        .clicked()
    {
        state.start_export();
    }
    if exporting {
        ui.add(egui::ProgressBar::new(state.export_progress).show_percentage());
        ui.label(
            RichText::new("Rendu hors ligne en cours…")
                .small()
                .color(theme.colors.text_secondary),
        );
    }
    if let Some(result) = &state.export_result {
        ui.label(
            RichText::new(format!(
                "Dernier export : {:.1} s · pic {:.1} dBFS",
                result.duration_secs,
                20.0 * result.peak.max(1e-6).log10()
            ))
            .small()
            .color(theme.colors.success),
        );
    }
    if let Some(err) = &state.export_error {
        ui.colored_label(theme.colors.error, format!("Export : {err}"));
    }

    ui.add_space(theme.spacing.md);
    ui.separator();
    ui.label(
        RichText::new("PROJETS RÉCENTS")
            .small()
            .strong()
            .color(theme.colors.text_secondary),
    );
    if button_small(ui, theme, "Actualiser").clicked() {
        state.refresh_projects();
    }
    egui::ScrollArea::vertical()
        .id_salt("projects_list")
        .max_height(150.0)
        .show(ui, |ui| {
            let projects = state.projects.clone();
            if projects.is_empty() {
                ui.label(
                    RichText::new("Aucun projet enregistré.")
                        .small()
                        .color(theme.colors.text_muted),
                );
            }
            for summary in projects {
                if summary.is_autosave {
                    continue;
                }
                let label = format!("{} · {:.0} BPM", summary.name, summary.bpm);
                if ui
                    .selectable_label(
                        state.project_path.as_ref() == Some(&summary.path),
                        RichText::new(label).small(),
                    )
                    .clicked()
                {
                    state.open(summary.path.clone());
                }
            }
        });

    ui.add_space(theme.spacing.md);
    ui.separator();
    ui.label(
        RichText::new("AIDE")
            .small()
            .strong()
            .color(theme.colors.text_secondary),
    );
    ui.horizontal(|ui| {
        if button_small(ui, theme, "Aide contextuelle").clicked() {
            state.show_help = true;
        }
        if button_small(ui, theme, "Raccourcis").clicked() {
            state.ui.show_shortcuts = true;
        }
    });
    if button_small(ui, theme, "Tutoriel « Mon premier beat »").clicked() {
        state.ui.show_tutorial = true;
    }

    let mut beginner = state.beginner;
    if ui
        .checkbox(&mut beginner, "Mode débutant")
        .on_hover_text("Simplifie la présentation, sans retirer de fonctionnalité.")
        .changed()
    {
        state.beginner = beginner;
        actions.push(StudioAction::PersistBeginner(beginner));
    }

    ui.add_space(theme.spacing.md);
    ui.label(
        RichText::new("Tout est local : projets et exports restent sur cet ordinateur.")
            .small()
            .color(theme.colors.text_muted),
    );
}

// ---------------------------------------------------------------------------
// Pied de page / notifications
// ---------------------------------------------------------------------------

fn footer(
    ui: &mut egui::Ui,
    theme: &ARXFUNDTheme,
    state: &mut StudioState,
    actions: &mut Vec<StudioAction>,
) {
    ui.separator();
    ui.horizontal(|ui| {
        let mut beginner = state.beginner;
        if ui
            .checkbox(&mut beginner, "Mode débutant")
            .on_hover_text("Simplifie uniquement la présentation.")
            .changed()
        {
            state.beginner = beginner;
            actions.push(StudioAction::PersistBeginner(beginner));
        }
        if state.beginner {
            ui.label(
                RichText::new(
                    "Astuce : place une Kick au pas 1, une Clap au pas 5, puis appuie sur ▶.",
                )
                .small()
                .color(theme.colors.accent_secondary),
            );
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if state.status.is_some() && ui.small_button("×").clicked() {
                state.status = None;
            }
            if let Some(status) = &state.status {
                ui.label(
                    RichText::new(status)
                        .small()
                        .color(theme.colors.accent_secondary),
                );
            } else if let Some(engine) = &state.engine {
                ui.label(
                    RichText::new(format!(
                        "CPU {:.0} % · {:.1} ms · {} Hz",
                        engine.cpu_load() * 100.0,
                        engine.latency_ms(),
                        engine.sample_rate()
                    ))
                    .small()
                    .color(theme.colors.text_muted),
                );
            }
        });
    });
}

// ---------------------------------------------------------------------------
// Fenêtres d'aide
// ---------------------------------------------------------------------------

const HELP_ENTRIES: &[(&str, &str)] = &[
    (
        "BPM",
        "Battements par minute. Règle la vitesse globale du projet. Glisse la valeur dans la barre de transport.",
    ),
    (
        "Pitch",
        "Hauteur du son en demi-tons. Sur un canal, -12 = une octave plus bas.",
    ),
    (
        "Velocity",
        "Force d'un pas (0 à 100 %). Un pas plus fort sonne plus fort et souvent plus brillant.",
    ),
    (
        "Gain",
        "Niveau d'un canal ou du master. 100 % = niveau nominal, au-delà de 100 % le signal peut saturer.",
    ),
    (
        "Pan",
        "Position stéréo : -1 à gauche, 0 au centre, +1 à droite.",
    ),
    (
        "EQ",
        "Égaliseur. Phase 3 — non implémenté dans cette version.",
    ),
    (
        "Compressor",
        "Réducteur de dynamique. Phase 3 — non implémenté dans cette version.",
    ),
    (
        "Limiter",
        "Plafonne le niveau pour éviter la saturation. Phase 3 — non implémenté.",
    ),
    (
        "Reverb",
        "Simulation d'espace. Phase 3 — non implémenté dans cette version.",
    ),
    (
        "Delay",
        "Écho répété. Phase 3 — non implémenté dans cette version.",
    ),
    (
        "ADSR",
        "Attack, Decay, Sustain, Release : enveloppe d'un son. Contrôles exposés en Phase 3.",
    ),
    (
        "MIDI",
        "Protocole de notes et de contrôleurs. Entrée MIDI matérielle : Phase 3 — non implémenté.",
    ),
    (
        "808",
        "Basse 808 : grosse caisse/basse longue et grave. Disponible comme canal « Basse 808 ».",
    ),
    (
        "Sidechain",
        "Compression déclenchée par un autre canal (pompage). Phase 3 — non implémenté.",
    ),
];

fn help_window(ctx: &egui::Context, theme: &ARXFUNDTheme, state: &mut StudioState) {
    let mut open = state.show_help;
    egui::Window::new("Aide contextuelle ARXFUND")
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(520.0)
        .show(ctx, |ui| {
            ui.label(
                RichText::new(
                    "Survole les contrôles pour une infobulle. Voici les termes audio utilisés.",
                )
                .small()
                .color(theme.colors.text_secondary),
            );
            ui.add_space(theme.spacing.sm);
            egui::ScrollArea::vertical()
                .max_height(420.0)
                .show(ui, |ui| {
                    for (term, description) in HELP_ENTRIES {
                        ui.label(
                            RichText::new(*term)
                                .strong()
                                .color(theme.colors.accent_secondary),
                        );
                        ui.label(RichText::new(*description).color(theme.colors.text_primary));
                        ui.add_space(theme.spacing.sm);
                    }
                });
            ui.add_space(theme.spacing.sm);
            if button_small(ui, theme, "Raccourcis clavier").clicked() {
                state.ui.show_shortcuts = true;
            }
        });
    state.show_help = open;
}

fn shortcuts_window(ctx: &egui::Context, theme: &ARXFUNDTheme, state: &mut StudioState) {
    let mut open = state.ui.show_shortcuts;
    egui::Window::new("Raccourcis clavier")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(420.0)
        .show(ctx, |ui| {
            ui.label(
                RichText::new(
                    "Les raccourcis sont actifs hors des champs de texte. Ils ne sont pas encore \
                     remappables (Phase 3 — non implémenté).",
                )
                .small()
                .color(theme.colors.warning),
            );
            ui.add_space(theme.spacing.sm);
            let shortcuts = [
                ("Espace", "Lecture / pause"),
                ("Ctrl+Z", "Annuler"),
                ("Ctrl+Y", "Rétablir"),
                ("Ctrl+S", "Enregistrer le projet"),
                ("Ctrl+N", "Nouveau projet"),
                ("Ctrl+O", "Ouvrir un projet"),
                ("F1", "Aide contextuelle"),
                ("Clic droit (pas)", "Accent (vélocité maximale)"),
                ("Glisser (pas)", "Peindre une rangée"),
            ];
            egui::Grid::new("shortcut_grid")
                .num_columns(2)
                .spacing(Vec2::new(16.0, 4.0))
                .show(ui, |ui| {
                    for (keys, action) in shortcuts {
                        ui.label(
                            RichText::new(keys)
                                .font(FontId::monospace(11.0))
                                .color(theme.colors.accent_secondary),
                        );
                        ui.label(RichText::new(action).color(theme.colors.text_primary));
                        ui.end_row();
                    }
                });
        });
    state.ui.show_shortcuts = open;
}

fn tutorial_window(ctx: &egui::Context, theme: &ARXFUNDTheme, state: &mut StudioState) {
    let mut open = state.ui.show_tutorial;
    egui::Window::new("Tutoriel — Mon premier beat")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(520.0)
        .show(ctx, |ui| {
            let steps = [
                "1. Ouvre l'onglet « Channel Rack ».",
                "2. Sur la ligne Kick, clique sur le pas 1.",
                "3. Sur la ligne Clap, clique sur les pas 5 et 13.",
                "4. Sur la ligne Charley fermé, clique sur les pas 1, 3, 5, 7, 9, 11, 13, 15.",
                "5. Règle le BPM à 140 dans la barre de transport.",
                "6. Appuie sur ▶ (Espace). Le beat est réellement joué par le moteur audio.",
                "7. Mélodie (piano roll) — Phase 3, non disponible.",
                "8. Arrangement complet du morceau — partiellement disponible via l'onglet Playlist.",
                "9. Mixage avancé (EQ, compression, effets) — Phase 3, non disponible.",
                "10. Export WAV — disponible dans le panneau de droite.",
            ];
            let index = state.ui.tutorial_step.min(steps.len() - 1);
            ui.label(
                RichText::new(format!("Étape {}/{}", index + 1, steps.len()))
                    .strong()
                    .color(theme.colors.accent_primary),
            );
            ui.add_space(theme.spacing.sm);
            let current = steps[index];
            let color = if current.contains("non disponible") {
                theme.colors.warning
            } else {
                theme.colors.text_primary
            };
            ui.label(RichText::new(current).size(14.0).color(color));
            ui.add_space(theme.spacing.md);
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(index > 0, egui::Button::new("← Précédent"))
                    .clicked()
                {
                    state.ui.tutorial_step = index - 1;
                }
                if ui
                    .add_enabled(index + 1 < steps.len(), egui::Button::new("Suivant →"))
                    .clicked()
                {
                    state.ui.tutorial_step = index + 1;
                }
                if button_small(ui, theme, "Terminer").clicked() {
                    state.ui.tutorial_completed = true;
                    state.ui.show_tutorial = false;
                }
            });
        });
    state.ui.show_tutorial = open;
}
