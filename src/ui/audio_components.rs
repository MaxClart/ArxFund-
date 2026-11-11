//! Composants audio réutilisables pour ARXFUND.
//!
//! Potentiomètre rotatif, fader vertical et VU-mètre. Tous les contrôles sont
//! réellement interactifs (glisser pour modifier, double-clic pour réinitialiser).

use eframe::egui;
use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Ui, Vec2};

use crate::ui::theme::ARXFUNDTheme;

const KNOB_SIZE: f32 = 56.0;

/// Potentiomètre rotatif.
pub struct Knob<'a> {
    value: &'a mut f32,
    range: std::ops::RangeInclusive<f32>,
    label: Option<&'a str>,
    unit: Option<&'a str>,
    precision: usize,
    show_value: bool,
    show_label: bool,
    default: f32,
}

impl<'a> Knob<'a> {
    pub fn new(value: &'a mut f32, range: std::ops::RangeInclusive<f32>) -> Self {
        let default = *range.start();
        Self {
            value,
            range,
            label: None,
            unit: None,
            precision: 1,
            show_value: true,
            show_label: true,
            default,
        }
    }

    pub fn label(mut self, text: &'a str) -> Self {
        self.label = Some(text);
        self
    }

    pub fn unit(mut self, text: &'a str) -> Self {
        self.unit = Some(text);
        self
    }

    pub fn precision(mut self, p: usize) -> Self {
        self.precision = p;
        self
    }

    pub fn show_value(mut self, show: bool) -> Self {
        self.show_value = show;
        self
    }

    pub fn show_label(mut self, show: bool) -> Self {
        self.show_label = show;
        self
    }

    /// Valeur restaurée par un double-clic.
    pub fn default_value(mut self, value: f32) -> Self {
        self.default = value;
        self
    }
}

impl<'a> egui::Widget for Knob<'a> {
    fn ui(self, ui: &mut Ui) -> egui::Response {
        let theme = ARXFUNDTheme::dark();
        let (rect, mut response) =
            ui.allocate_exact_size(Vec2::splat(KNOB_SIZE), Sense::click_and_drag());
        let start = *self.range.start();
        let end = *self.range.end();
        let span = (end - start).max(f32::EPSILON);

        if response.dragged() {
            let delta = response.drag_delta();
            *self.value = (*self.value - delta.y * span / 200.0).clamp(start, end);
            response.mark_changed();
        }
        if response.double_clicked() {
            *self.value = self.default.clamp(start, end);
            response.mark_changed();
        }

        let painter = ui.painter_at(rect);
        let center = rect.center();
        let radius = KNOB_SIZE / 2.0 - 6.0;

        let t = ((*self.value - start) / span).clamp(0.0, 1.0);
        let angle_rad = (-135.0_f32 + t * 270.0).to_radians();

        painter.circle_filled(center, radius, Color32::from_rgb(0x2A, 0x30, 0x3E));
        painter.circle_stroke(center, radius, Stroke::new(1.0, theme.colors.separator));

        let start_angle = (-135.0_f32).to_radians();
        let steps = 24;
        let mut arc = Vec::with_capacity(steps + 1);
        for i in 0..=steps {
            let a = start_angle + (angle_rad - start_angle) * (i as f32 / steps as f32);
            arc.push(Pos2::new(
                center.x + radius * a.cos(),
                center.y + radius * a.sin(),
            ));
        }
        if arc.len() >= 2 {
            painter.add(egui::Shape::line(
                arc,
                Stroke::new(3.0, theme.colors.accent_primary),
            ));
        }

        let needle_len = radius * 0.72;
        painter.line_segment(
            [
                center,
                Pos2::new(
                    center.x + needle_len * angle_rad.cos(),
                    center.y + needle_len * angle_rad.sin(),
                ),
            ],
            Stroke::new(2.0, Color32::WHITE),
        );
        painter.circle_filled(center, 3.0, Color32::WHITE);

        if self.show_value {
            let unit = self.unit.map(|u| format!(" {u}")).unwrap_or_default();
            painter.text(
                Pos2::new(center.x, center.y - 2.0),
                Align2::CENTER_CENTER,
                format!("{:.*}{}", self.precision, *self.value, unit),
                FontId::monospace(9.0),
                theme.colors.accent_primary,
            );
        }

        if self.show_label {
            if let Some(text) = self.label {
                painter.text(
                    Pos2::new(center.x, rect.max.y - 1.0),
                    Align2::CENTER_BOTTOM,
                    text,
                    FontId::proportional(9.0),
                    theme.colors.text_secondary,
                );
            }
        }

        response
    }
}

pub fn knob<'a>(value: &'a mut f32, range: std::ops::RangeInclusive<f32>) -> Knob<'a> {
    Knob::new(value, range)
}

/// Fader vertical.
pub struct VerticalSlider<'a> {
    value: &'a mut f32,
    range: std::ops::RangeInclusive<f32>,
    label: Option<&'a str>,
    show_value: bool,
    width: f32,
    height: f32,
}

impl<'a> VerticalSlider<'a> {
    pub fn new(value: &'a mut f32, range: std::ops::RangeInclusive<f32>) -> Self {
        Self {
            value,
            range,
            label: None,
            show_value: true,
            width: 26.0,
            height: 120.0,
        }
    }

    pub fn label(mut self, text: &'a str) -> Self {
        self.label = Some(text);
        self
    }

    pub fn show_value(mut self, show: bool) -> Self {
        self.show_value = show;
        self
    }

    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.width = width;
        self.height = height;
        self
    }
}

impl<'a> egui::Widget for VerticalSlider<'a> {
    fn ui(self, ui: &mut Ui) -> egui::Response {
        let theme = ARXFUNDTheme::dark();
        let (rect, mut response) =
            ui.allocate_exact_size(Vec2::new(self.width, self.height), Sense::click_and_drag());
        let start = *self.range.start();
        let end = *self.range.end();
        let span = (end - start).max(f32::EPSILON);

        if response.dragged() {
            let delta = response.drag_delta();
            *self.value = (*self.value - delta.y * span / 160.0).clamp(start, end);
            response.mark_changed();
        }
        if response.double_clicked() {
            *self.value = start;
            response.mark_changed();
        }
        if response.clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                let t = ((rect.bottom() - pos.y) / rect.height()).clamp(0.0, 1.0);
                *self.value = start + t * span;
                response.mark_changed();
            }
        }

        let painter = ui.painter_at(rect);
        let track_width = 6.0;
        let track_rect = Rect::from_min_size(
            Pos2::new(rect.center().x - track_width / 2.0, rect.top() + 8.0),
            Vec2::new(track_width, (rect.height() - 16.0).max(1.0)),
        );
        painter.rect_filled(track_rect, 3.0, Color32::from_rgb(30, 30, 40));

        let t = ((*self.value - start) / span).clamp(0.0, 1.0);
        let fill_height = track_rect.height() * t;
        let fill_rect = Rect::from_min_size(
            Pos2::new(track_rect.left(), track_rect.bottom() - fill_height),
            Vec2::new(track_width, fill_height),
        );
        painter.rect_filled(fill_rect, 3.0, theme.colors.accent_primary);

        let knob_h = 14.0;
        let knob_y = (track_rect.bottom() - fill_height - knob_h / 2.0)
            .clamp(rect.top(), rect.bottom() - knob_h);
        let knob_rect = Rect::from_min_size(
            Pos2::new(rect.center().x - self.width / 2.0 + 2.0, knob_y),
            Vec2::new(self.width - 4.0, knob_h),
        );
        painter.rect_filled(knob_rect, 3.0, Color32::WHITE);

        if self.show_value {
            painter.text(
                Pos2::new(rect.center().x, rect.bottom()),
                Align2::CENTER_BOTTOM,
                format!("{:.1}", *self.value),
                FontId::monospace(9.0),
                theme.colors.text_primary,
            );
        }
        if let Some(label) = self.label {
            painter.text(
                Pos2::new(rect.center().x, rect.top()),
                Align2::CENTER_BOTTOM,
                label,
                FontId::proportional(9.0),
                theme.colors.text_secondary,
            );
        }

        response
    }
}

pub fn vertical_slider<'a>(
    value: &'a mut f32,
    range: std::ops::RangeInclusive<f32>,
) -> VerticalSlider<'a> {
    VerticalSlider::new(value, range)
}

/// VU-mètre vertical avec repère de crête.
pub struct AudioMeter<'a> {
    level: f32,
    peak: f32,
    label: Option<&'a str>,
    show_label: bool,
    width: f32,
    height: f32,
}

impl<'a> AudioMeter<'a> {
    pub fn new(level: f32, peak: f32) -> Self {
        Self {
            level,
            peak,
            label: None,
            show_label: true,
            width: 14.0,
            height: 100.0,
        }
    }

    pub fn label(mut self, text: &'a str) -> Self {
        self.label = Some(text);
        self
    }

    pub fn show_label(mut self, show: bool) -> Self {
        self.show_label = show;
        self
    }

    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.width = width;
        self.height = height;
        self
    }
}

impl<'a> egui::Widget for AudioMeter<'a> {
    fn ui(self, ui: &mut Ui) -> egui::Response {
        let theme = ARXFUNDTheme::dark();
        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(self.width, self.height), Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 2.0, theme.colors.bg_secondary);

        let level = self.level.clamp(0.0, 1.0);
        let fill_height = (rect.height() * level).max(1.0);
        let fill_rect = Rect::from_min_size(
            Pos2::new(rect.min.x, rect.max.y - fill_height),
            Vec2::new(rect.width(), fill_height),
        );
        let color = if level > 0.9 {
            theme.colors.error
        } else if level > 0.7 {
            theme.colors.warning
        } else {
            theme.colors.accent_secondary
        };
        painter.rect_filled(fill_rect, 2.0, color);

        let peak = self.peak.clamp(0.0, 1.0);
        let peak_y = rect.max.y - rect.height() * peak;
        painter.line_segment(
            [Pos2::new(rect.min.x, peak_y), Pos2::new(rect.max.x, peak_y)],
            Stroke::new(1.0, Color32::WHITE),
        );

        if self.show_label {
            if let Some(label) = self.label {
                painter.text(
                    Pos2::new(rect.center().x, rect.top() - 2.0),
                    Align2::CENTER_BOTTOM,
                    label,
                    FontId::proportional(9.0),
                    theme.colors.text_secondary,
                );
            }
        }

        response
    }
}

pub fn audio_meter(level: f32, peak: f32) -> AudioMeter<'static> {
    AudioMeter::new(level, peak)
}

/// Afficheur de tempo éditable.
pub fn bpm_display(ui: &mut Ui, _theme: &ARXFUNDTheme, bpm: &mut f64) -> egui::Response {
    ui.add(
        egui::DragValue::new(bpm)
            .range(40.0..=300.0)
            .speed(0.5)
            .suffix(" BPM"),
    )
}

/// Affichage du tempo en lecture seule.
pub fn bpm_label(ui: &mut Ui, theme: &ARXFUNDTheme, bpm: f64) -> egui::Response {
    ui.label(
        egui::RichText::new(format!("{bpm:.0} BPM"))
            .font(theme.typography.mono.clone())
            .strong()
            .color(theme.colors.text_primary),
    )
}

pub fn mute_button(ui: &mut Ui, theme: &ARXFUNDTheme, active: bool) -> egui::Response {
    let bg = if active {
        theme.colors.error
    } else {
        theme.colors.button_ghost_bg
    };
    ui.add(
        egui::Button::new(egui::RichText::new("M").color(Color32::WHITE))
            .fill(bg)
            .corner_radius(theme.radius.small)
            .min_size(Vec2::new(20.0, 20.0)),
    )
}

pub fn solo_button(ui: &mut Ui, theme: &ARXFUNDTheme, active: bool) -> egui::Response {
    let bg = if active {
        theme.colors.accent_primary
    } else {
        theme.colors.button_ghost_bg
    };
    ui.add(
        egui::Button::new(egui::RichText::new("S").color(Color32::WHITE))
            .fill(bg)
            .corner_radius(theme.radius.small)
            .min_size(Vec2::new(20.0, 20.0)),
    )
}
