//! Composants d'interface ARXFUND.
//!
//! Compatible avec egui/eframe 0.37.

use eframe::egui;
use eframe::egui::{Align2, Color32, FontId, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2};

use crate::ui::theme::ARXFUNDTheme;

// ---------------------------------------------------------------------------
// ARXFUND Button — bouton principal (accent)
// ---------------------------------------------------------------------------

pub fn button(ui: &mut Ui, theme: &ARXFUNDTheme, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(
            RichText::new(text)
                .strong()
                .color(theme.colors.button_accent_text),
        )
        .fill(theme.colors.button_accent_bg)
        .corner_radius(theme.radius.medium)
        .min_size(Vec2::new(0.0, 32.0)),
    )
}

pub fn button_small(ui: &mut Ui, theme: &ARXFUNDTheme, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(text).color(theme.colors.button_accent_text))
            .fill(theme.colors.button_accent_bg)
            .corner_radius(theme.radius.small)
            .min_size(Vec2::new(0.0, 24.0)),
    )
}

// ---------------------------------------------------------------------------
// ARXFUND Icon Button — bouton icône uniquement (fond ghost)
// ---------------------------------------------------------------------------

pub fn icon_button(ui: &mut Ui, theme: &ARXFUNDTheme, icon: &str, size: f32) -> egui::Response {
    let size_vec = Vec2::splat(size);
    ui.add_sized(
        size_vec,
        egui::Button::new(
            RichText::new(icon)
                .size(size * 0.65)
                .color(theme.colors.text_primary),
        )
        .fill(theme.colors.button_ghost_bg)
        .corner_radius(theme.radius.medium),
    )
}

pub fn icon_button_small(ui: &mut Ui, theme: &ARXFUNDTheme, icon: &str) -> egui::Response {
    icon_button(ui, theme, icon, 28.0)
}

// ---------------------------------------------------------------------------
// ARXFUND Toggle — interrupteur On/Off
// ---------------------------------------------------------------------------

pub fn toggle(
    ui: &mut Ui,
    theme: &ARXFUNDTheme,
    on: &mut bool,
    label: Option<&str>,
) -> egui::Response {
    let width = 44.0;
    let height = 24.0;
    let knob_size = 16.0;
    let knob_margin = 4.0;
    let label_w = label.map(|_| 90.0).unwrap_or(0.0);
    let total_w = width + label_w;

    let (rect, response) = ui.allocate_exact_size(Vec2::new(total_w, height), Sense::click());
    if response.clicked() {
        *on = !*on;
    }

    let painter = ui.painter_at(rect);
    let track_rect = Rect::from_min_size(
        Pos2::new(rect.min.x, rect.center().y - height / 2.0),
        Vec2::new(width, height),
    );
    let knob_x = if *on {
        track_rect.max.x - knob_size - knob_margin
    } else {
        track_rect.min.x + knob_margin
    };
    let knob_rect = Rect::from_min_size(
        Pos2::new(knob_x, track_rect.min.y + knob_margin),
        Vec2::splat(knob_size),
    );

    painter.rect_filled(
        track_rect,
        theme.radius.full,
        if *on {
            theme.colors.accent_primary
        } else {
            theme.colors.separator
        },
    );
    painter.rect_filled(knob_rect, theme.radius.full, Color32::WHITE);
    painter.rect_stroke(
        track_rect,
        theme.radius.full,
        Stroke::new(1.0, theme.colors.separator),
        eframe::egui::StrokeKind::Inside,
    );

    if let Some(label) = label {
        painter.text(
            Pos2::new(rect.min.x + width + 8.0, rect.center().y),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(12.0),
            theme.colors.text_primary,
        );
    }

    response
}

// ---------------------------------------------------------------------------
// ARXFUND Slider — slider stylisé avec piste custom
// ---------------------------------------------------------------------------

pub fn slider(
    ui: &mut Ui,
    theme: &ARXFUNDTheme,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    label: Option<&str>,
) -> egui::Response {
    if let Some(label) = label {
        ui.label(
            RichText::new(label)
                .small()
                .color(theme.colors.text_secondary),
        );
    }

    let range_start = *range.start();
    let range_end = *range.end();

    let response = ui.add(egui::Slider::new(value, range).show_value(false));

    if response.rect.intersects(ui.clip_rect()) {
        let inner = response.rect.intersect(ui.clip_rect());
        let painter = ui.painter();
        let track_y = inner.center().y;
        let track_height = 4.0;
        let track_rect = Rect::from_min_size(
            Pos2::new(inner.min.x, track_y - track_height / 2.0),
            Vec2::new(inner.width(), track_height),
        );

        painter.rect_filled(track_rect, theme.radius.full, theme.colors.separator);

        let t = (*value - range_start) / (range_end - range_start);
        let t = t.clamp(0.0, 1.0);
        let fill_width = inner.width() * t;
        let fill_rect = Rect::from_min_size(
            Pos2::new(inner.min.x, track_y - track_height / 2.0),
            Vec2::new(fill_width, track_height),
        );
        painter.rect_filled(fill_rect, theme.radius.full, theme.colors.accent_primary);

        let knob_x = inner.min.x + fill_width;
        painter.circle_filled(Pos2::new(knob_x, track_y), 7.0, Color32::WHITE);
        painter.circle_stroke(
            Pos2::new(knob_x, track_y),
            7.0,
            Stroke::new(1.5, theme.colors.accent_primary),
        );
    }

    response
}

// ---------------------------------------------------------------------------
// ARXFUND Knob — potentiomètre rotatif
// ---------------------------------------------------------------------------

pub fn knob(
    ui: &mut Ui,
    theme: &ARXFUNDTheme,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    label: Option<&str>,
) -> egui::Response {
    let size = 64.0;
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click_and_drag());

    if response.dragged() {
        let delta = response.drag_delta();
        let step = (*range.end() - *range.start()) / 200.0;
        *value = (*value - delta.y * step * 0.5).clamp(*range.start(), *range.end());
    }

    let painter = ui.painter_at(rect);
    let center = rect.center();
    let radius = size / 2.0 - 4.0;

    let t = (*value - *range.start()) / (*range.end() - *range.start());
    let t = t.clamp(0.0, 1.0);
    let angle_deg = -135.0_f32 + t * 270.0_f32;
    let angle_rad = angle_deg.to_radians();

    painter.circle_filled(center, radius, Color32::from_rgb(0x2A, 0x30, 0x3E));
    painter.circle_stroke(center, radius, Stroke::new(1.0, theme.colors.separator));

    let start_angle = (-135.0_f32).to_radians();
    let mut arc_points = Vec::new();
    let steps = 24;
    for i in 0..=steps {
        let a = start_angle + (angle_rad - start_angle) * (i as f32 / steps as f32);
        arc_points.push(Pos2::new(
            center.x + radius * a.cos(),
            center.y + radius * a.sin(),
        ));
    }
    if arc_points.len() >= 2 {
        painter.add(egui::Shape::line(
            arc_points,
            Stroke::new(3.0, theme.colors.accent_primary),
        ));
    }

    let needle_len = radius * 0.75;
    let needle_end = Pos2::new(
        center.x + needle_len * angle_rad.cos(),
        center.y + needle_len * angle_rad.sin(),
    );
    painter.line_segment([center, needle_end], Stroke::new(2.0, Color32::WHITE));
    painter.circle_filled(center, 4.0, Color32::WHITE);

    if let Some(label) = label {
        painter.text(
            Pos2::new(center.x, rect.max.y - 2.0),
            Align2::CENTER_BOTTOM,
            label,
            FontId::proportional(10.0),
            theme.colors.text_secondary,
        );
    }

    let value_text = format!("{:.1}", value);
    painter.text(
        Pos2::new(center.x, rect.center().y - 4.0),
        Align2::CENTER_CENTER,
        value_text,
        FontId::monospace(10.0),
        theme.colors.accent_primary,
    );

    response
}

// ---------------------------------------------------------------------------
// ARXFUND Dropdown — sélection déroulante (ComboBox natif stylisé)
// ---------------------------------------------------------------------------

pub fn dropdown<T: Clone + PartialEq + ToString>(
    ui: &mut Ui,
    theme: &ARXFUNDTheme,
    current: &T,
    items: &[(T, &str)],
    id_salt: &str,
) -> Option<T> {
    let mut new_value = None;
    let current_label = items
        .iter()
        .find(|(v, _)| v == current)
        .map(|(_, l)| *l)
        .unwrap_or(items.first().map(|(_, l)| *l).unwrap_or(""));

    egui::ComboBox::from_id_salt(id_salt)
        .selected_text(RichText::new(current_label).color(theme.colors.text_primary))
        .show_ui(ui, |ui| {
            for (value, label) in items.iter() {
                let selected = value == current;
                if ui
                    .selectable_label(
                        selected,
                        RichText::new(*label).color(theme.colors.text_primary),
                    )
                    .clicked()
                {
                    new_value = Some(value.clone());
                }
            }
        });

    new_value
}

// ---------------------------------------------------------------------------
// ARXFUND Search Field — champ de recherche
// ---------------------------------------------------------------------------

pub fn search_field(
    ui: &mut Ui,
    theme: &ARXFUNDTheme,
    text: &mut String,
    placeholder: &str,
) -> egui::Response {
    // Allouer le rect du champ, dessiner le fond, puis placer le TextEdit
    let desired = Vec2::new(ui.available_width(), 32.0);
    let (rect, _alloc_response) = ui.allocate_exact_size(desired, Sense::click());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, theme.radius.medium, theme.colors.button_ghost_bg);
    painter.rect_stroke(
        rect,
        theme.radius.medium,
        Stroke::new(1.0, theme.colors.separator),
        eframe::egui::StrokeKind::Inside,
    );

    // Placer le TextEdit à l'intérieur (frame par défaut)
    let text_response = ui.put(
        rect.shrink(8.0),
        egui::TextEdit::singleline(text)
            .hint_text(RichText::new(placeholder).color(theme.colors.text_muted))
            .text_color(theme.colors.text_primary),
    );

    text_response
}

// ---------------------------------------------------------------------------
// Section Header — titre de section
// ---------------------------------------------------------------------------

pub fn section_header(ui: &mut Ui, theme: &ARXFUNDTheme, title: &str, subtitle: Option<&str>) {
    ui.add_space(theme.spacing.xs);
    ui.label(
        RichText::new(title)
            .strong()
            .size(14.0)
            .color(theme.colors.text_primary),
    );
    if let Some(sub) = subtitle {
        ui.label(
            RichText::new(sub)
                .size(10.5)
                .color(theme.colors.text_secondary),
        );
    }
    ui.add_space(theme.spacing.sm);
    ui.separator();
}

// ---------------------------------------------------------------------------
// Card — panneau surélevé
// ---------------------------------------------------------------------------

pub fn card<R>(ui: &mut Ui, theme: &ARXFUNDTheme, content: impl FnOnce(&mut Ui) -> R) -> R {
    let frame = eframe::egui::Frame {
        inner_margin: eframe::egui::vec2(theme.spacing.md, theme.spacing.md).into(),
        outer_margin: Default::default(),
        corner_radius: theme.radius.large.into(),
        fill: theme.colors.bg_panel,
        stroke: Stroke::new(1.0, theme.colors.separator),
        shadow: eframe::egui::epaint::Shadow::NONE,
    };
    frame.show(ui, |ui| content(ui)).inner
}

/// Amélioration du composant Card — fond personnalisé du thème (alias)
pub fn card_v2<R>(ui: &mut Ui, theme: &ARXFUNDTheme, content: impl FnOnce(&mut Ui) -> R) -> R {
    card(ui, theme, content)
}

// ---------------------------------------------------------------------------
// Empty State — état vide élégant
// ---------------------------------------------------------------------------

pub fn empty_state(ui: &mut Ui, theme: &ARXFUNDTheme, icon: &str, title: &str, description: &str) {
    ui.vertical_centered(|ui| {
        ui.add_space(theme.spacing.lg);
        ui.label(
            RichText::new(icon)
                .size(32.0)
                .color(theme.colors.text_muted),
        );
        ui.add_space(theme.spacing.sm);
        ui.label(
            RichText::new(title)
                .strong()
                .color(theme.colors.text_primary),
        );
        ui.add_space(theme.spacing.xs);
        ui.label(RichText::new(description).color(theme.colors.text_secondary));
        ui.add_space(theme.spacing.lg);
    });
}

// ---------------------------------------------------------------------------
// Divider
// ---------------------------------------------------------------------------

pub fn divider(ui: &mut Ui, _theme: &ARXFUNDTheme) {
    ui.separator();
}

pub fn section_divider(ui: &mut Ui, theme: &ARXFUNDTheme, label: &str) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(label)
                .small()
                .strong()
                .color(theme.colors.accent_primary),
        );
        ui.separator();
    });
}

// ---------------------------------------------------------------------------
// Track Header — en-tête de piste
// ---------------------------------------------------------------------------

pub fn track_header(
    ui: &mut Ui,
    theme: &ARXFUNDTheme,
    color: Color32,
    name: &str,
) -> egui::Response {
    let response = ui.horizontal(|ui| {
        let color_rect = Rect::from_min_size(
            Pos2::new(ui.cursor().min.x, ui.cursor().min.y + 4.0),
            Vec2::new(10.0, 10.0),
        );
        ui.painter().rect_filled(color_rect, 2.0, color);
        ui.add_space(8.0);
        ui.label(RichText::new(name).color(theme.colors.text_primary));
        ui.with_layout(
            eframe::egui::Layout::right_to_left(eframe::egui::Align::Center),
            |ui| {
                if icon_button_small(ui, theme, "S").clicked() {}
                if icon_button_small(ui, theme, "M").clicked() {}
            },
        );
    });

    response.response
}

// ---------------------------------------------------------------------------
// Audio Meter — VU mètre vertical
// ---------------------------------------------------------------------------

pub fn audio_meter(ui: &mut Ui, theme: &ARXFUNDTheme, level: f32, peak: f32) -> egui::Response {
    let width = 14.0;
    let height = ui.available_height().min(200.0);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());

    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, theme.radius.small, theme.colors.bg_secondary);

    let level = level.clamp(0.0, 1.0);
    let fill_height = (rect.height() * level).max(1.0);
    let fill_rect = Rect::from_min_size(
        Pos2::new(rect.min.x, rect.max.y - fill_height),
        Vec2::new(rect.width(), fill_height),
    );
    let level_color = if level > 0.9 {
        theme.colors.error
    } else if level > 0.7 {
        theme.colors.warning
    } else {
        theme.colors.accent_secondary
    };
    painter.rect_filled(fill_rect, theme.radius.small, level_color);

    let peak_y = rect.max.y - (rect.height() * peak.clamp(0.0, 1.0));
    painter.line_segment(
        [Pos2::new(rect.min.x, peak_y), Pos2::new(rect.max.x, peak_y)],
        Stroke::new(1.0, Color32::WHITE),
    );

    response
}

// ---------------------------------------------------------------------------
// Context Menu — menu contextuel
// ---------------------------------------------------------------------------

pub fn context_menu<R>(
    ui: &mut Ui,
    theme: &ARXFUNDTheme,
    id: &str,
    items: &[&str],
    on_click: &mut dyn FnMut(usize) -> R,
) -> Option<R> {
    let mut result = None;
    let _menu_id = ui.make_persistent_id(id);
    ui.menu_button(
        RichText::new("⋮").color(theme.colors.text_secondary),
        |ui| {
            for (i, item) in items.iter().enumerate() {
                if ui
                    .button(RichText::new(*item).color(theme.colors.text_primary))
                    .clicked()
                {
                    result = Some((on_click)(i));
                }
            }
        },
    );
    result
}

// ---------------------------------------------------------------------------
// Badge — petit indicateur coloré (compteur, statut)
// ---------------------------------------------------------------------------

pub fn badge(ui: &mut Ui, theme: &ARXFUNDTheme, text: &str, kind: BadgeKind) -> egui::Response {
    let (bg, fg) = match kind {
        BadgeKind::Neutral => (theme.colors.separator, theme.colors.text_secondary),
        BadgeKind::Accent => (theme.colors.accent_primary, Color32::WHITE),
        BadgeKind::Success => (theme.colors.success, Color32::WHITE),
        BadgeKind::Warning => (theme.colors.warning, Color32::WHITE),
        BadgeKind::Error => (theme.colors.error, Color32::WHITE),
    };

    ui.add(
        egui::Button::new(RichText::new(text).small().strong().color(fg))
            .fill(bg)
            .corner_radius(theme.radius.full)
            .min_size(Vec2::new(0.0, 18.0)),
    )
}

/// Type de badge.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BadgeKind {
    Neutral,
    Accent,
    Success,
    Warning,
    Error,
}

// ---------------------------------------------------------------------------
// Modal — fenêtre modale centrée avec fond sombre
// ---------------------------------------------------------------------------

pub struct Modal<'a> {
    ui: &'a mut Ui,
    theme: &'a ARXFUNDTheme,
    title: Option<&'a str>,
}

impl<'a> Modal<'a> {
    pub fn new(ui: &'a mut Ui, theme: &'a ARXFUNDTheme) -> Self {
        Self {
            ui,
            theme,
            title: None,
        }
    }

    pub fn title(mut self, title: &'a str) -> Self {
        self.title = Some(title);
        self
    }

    pub fn show<F, R>(self, add_contents: F) -> Option<R>
    where
        F: FnOnce(&mut Ui) -> R,
    {
        let ctx = self.ui.ctx();
        let theme = self.theme;
        let mut result = None;

        egui::Window::new(self.title.unwrap_or("ARXFUND"))
            .resizable(false)
            .collapsible(false)
            .title_bar(false)
            .show(ctx, |ui| {
                ui.set_min_size(Vec2::new(400.0, 300.0));

                let frame = eframe::egui::Frame {
                    inner_margin: eframe::egui::vec2(theme.spacing.md, theme.spacing.md).into(),
                    outer_margin: Default::default(),
                    corner_radius: theme.radius.large.into(),
                    fill: theme.colors.bg_panel,
                    stroke: Stroke::new(1.0, theme.colors.separator),
                    shadow: eframe::egui::epaint::Shadow::NONE,
                };
                frame.show(ui, |ui| {
                    if let Some(title) = self.title {
                        ui.label(
                            RichText::new(title)
                                .strong()
                                .size(16.0)
                                .color(theme.colors.text_primary),
                        );
                        ui.add_space(theme.spacing.sm);
                        ui.separator();
                        ui.add_space(theme.spacing.sm);
                    }

                    result = Some(add_contents(ui));
                });
            });

        result
    }
}

// ---------------------------------------------------------------------------
// Notification / Toast — message temporaire en bas à droite
// ---------------------------------------------------------------------------

pub struct Notification {
    pub message: String,
    pub kind: NotificationKind,
    pub timestamp: std::time::Instant,
    pub duration: std::time::Duration,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NotificationKind {
    Info,
    Success,
    Warning,
    Error,
}

pub struct NotificationManager {
    pub notifications: Vec<Notification>,
}

impl NotificationManager {
    pub fn new() -> Self {
        Self {
            notifications: Vec::new(),
        }
    }

    pub fn push(&mut self, message: impl Into<String>, kind: NotificationKind) {
        self.notifications.push(Notification {
            message: message.into(),
            kind,
            timestamp: std::time::Instant::now(),
            duration: std::time::Duration::from_secs(5),
        });
    }

    pub fn update(&mut self) {
        let now = std::time::Instant::now();
        self.notifications
            .retain(|n| now.duration_since(n.timestamp) < n.duration);
    }

    pub fn render(&mut self, ui: &mut Ui, theme: &ARXFUNDTheme) {
        self.update();
        let screen_rect = ui.available_rect_before_wrap();
        let mut y_offset = screen_rect.max.y - 20.0;

        for n in self.notifications.iter().rev() {
            let elapsed = n.timestamp.elapsed().as_secs_f32();
            let progress = (elapsed / n.duration.as_secs_f32()).clamp(0.0, 1.0);
            let alpha = (1.0 - progress).clamp(0.0, 1.0);

            let (bg, fg) = match n.kind {
                NotificationKind::Info => (theme.colors.bg_panel, theme.colors.text_primary),
                NotificationKind::Success => (theme.colors.success, Color32::WHITE),
                NotificationKind::Warning => (theme.colors.warning, Color32::WHITE),
                NotificationKind::Error => (theme.colors.error, Color32::WHITE),
            };

            let width = 300.0;
            let height = 50.0;
            let rect = Rect::from_min_size(
                Pos2::new(screen_rect.max.x - width - 20.0, y_offset - height),
                Vec2::new(width, height),
            );

            let alpha_u8 = (255.0 * alpha) as u8;
            let bg_alpha = Color32::from_rgba_unmultiplied(bg.r(), bg.g(), bg.b(), alpha_u8);
            let fg_alpha = Color32::from_rgba_unmultiplied(fg.r(), fg.g(), fg.b(), alpha_u8);

            let painter = ui.painter();
            painter.rect_filled(rect, theme.radius.medium, bg_alpha);
            painter.rect_stroke(
                rect,
                theme.radius.medium,
                Stroke::new(1.0, theme.colors.separator),
                eframe::egui::StrokeKind::Inside,
            );

            painter.text(
                Pos2::new(rect.min.x + 12.0, rect.center().y - 8.0),
                Align2::LEFT_TOP,
                &n.message,
                FontId::proportional(13.0),
                fg_alpha,
            );

            y_offset -= height + 10.0;
        }
    }
}

/// Bouton texte simple (texte seul, sans fond)
pub fn text_button(ui: &mut Ui, theme: &ARXFUNDTheme, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(text).color(theme.colors.accent_primary))
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::new(0.0, Color32::TRANSPARENT)),
    )
}
