//! Thème ARXFUND DARK — palette, typographie, espacements et rayons.
//!
//! Toutes les couleurs proviennent de cette source unique. Aucun écran ne
//! doit définir ses propres constantes de couleur en dur.

use eframe::egui::{Color32, FontId};

/// Palette de couleurs ARXFUND DARK.
#[derive(Clone, Debug)]
pub struct Colors {
    /// Fond principal de l'application.
    pub bg_primary: Color32,
    /// Fond secondaire (zones de contenu).
    pub bg_secondary: Color32,
    /// Panneaux et cartes.
    pub bg_panel: Color32,
    /// Surfaces surélevées (popovers, tooltips).
    pub bg_surface: Color32,
    /// Couleur principale (violet ARXFUND).
    pub accent_primary: Color32,
    /// Accent secondaire (cyan).
    pub accent_secondary: Color32,
    /// Texte principal.
    pub text_primary: Color32,
    /// Texte secondaire (muted).
    pub text_secondary: Color32,
    /// Texte désactivé.
    pub text_muted: Color32,
    /// Séparateurs et bordures subtiles.
    pub separator: Color32,
    /// Bordure d'élément sélectionné.
    pub border_selected: Color32,
    /// États de feedback.
    pub success: Color32,
    pub warning: Color32,
    pub error: Color32,
    /// Fond d'un bouton actif/accentué.
    pub button_accent_bg: Color32,
    /// Texte d'un bouton accentué.
    pub button_accent_text: Color32,
    /// Fond d'un bouton secondaire (ghost).
    pub button_ghost_bg: Color32,
    /// Texte d'un bouton secondaire.
    pub button_ghost_text: Color32,
    /// Fond d'une ligne de sélection dans une table.
    pub row_selected: Color32,
    /// Fond de survol d'une ligne.
    pub row_hover: Color32,
    /// Fond d'un pas inactif du Channel Rack.
    pub step_off: Color32,
    /// Fond d'un pas actif du Channel Rack.
    pub step_on: Color32,
    /// Fond d'un pas en cours de lecture (playhead).
    pub step_playhead: Color32,
}

impl Colors {
    pub fn dark() -> Self {
        Self {
            bg_primary: Color32::from_rgb(0x0D, 0x0F, 0x14),
            bg_secondary: Color32::from_rgb(0x15, 0x19, 0x22),
            bg_panel: Color32::from_rgb(0x1C, 0x22, 0x2D),
            bg_surface: Color32::from_rgb(0x25, 0x2C, 0x39),
            accent_primary: Color32::from_rgb(0x8B, 0x5C, 0xF6),
            accent_secondary: Color32::from_rgb(0x38, 0xBD, 0xF8),
            text_primary: Color32::from_rgb(0xF3, 0xF4, 0xF6),
            text_secondary: Color32::from_rgb(0x9C, 0xA3, 0xAF),
            text_muted: Color32::from_rgb(0x6B, 0x72, 0x80),
            separator: Color32::from_rgb(0x30, 0x37, 0x45),
            border_selected: Color32::from_rgb(0x8B, 0x5C, 0xF6),
            success: Color32::from_rgb(0x4A, 0xDE, 0x80),
            warning: Color32::from_rgb(0xF9, 0x73, 0x16),
            error: Color32::from_rgb(0xF8, 0x71, 0x71),
            button_accent_bg: Color32::from_rgb(0x8B, 0x5C, 0xF6),
            button_accent_text: Color32::WHITE,
            button_ghost_bg: Color32::from_rgb(0x1C, 0x22, 0x2D),
            button_ghost_text: Color32::from_rgb(0xF3, 0xF4, 0xF6),
            row_selected: Color32::from_rgba_unmultiplied(0x8B, 0x5C, 0xF6, (255.0 * 0.18) as u8),
            row_hover: Color32::from_rgba_unmultiplied(0xFF, 0xFF, 0xFF, (255.0 * 0.04) as u8),
            step_off: Color32::from_rgb(0x22, 0x26, 0x32),
            step_on: Color32::from_rgb(0x8B, 0x5C, 0xF6),
            step_playhead: Color32::from_rgba_unmultiplied(0xFF, 0xFF, 0xFF, (255.0 * 0.20) as u8),
        }
    }
}

/// Rayons d'arrondi cohérents.
#[derive(Clone, Copy, Debug)]
pub struct Radius {
    pub small: f32,  // 4px — petits éléments
    pub medium: f32, // 8px — boutons, champs
    pub large: f32,  // 12px — panneaux, cartes
    pub xl: f32,     // 16px — modales, grandes surfaces
    pub full: f32,   // cercle complet
}

impl Radius {
    pub fn default() -> Self {
        Self {
            small: 4.0,
            medium: 8.0,
            large: 12.0,
            xl: 16.0,
            full: 999.0,
        }
    }
}

/// Espacements cohérents.
#[derive(Clone, Copy, Debug)]
pub struct Spacing {
    pub xs: f32,  // 4px
    pub sm: f32,  // 8px
    pub md: f32,  // 16px
    pub lg: f32,  // 24px
    pub xl: f32,  // 32px
    pub xxl: f32, // 48px
}

impl Spacing {
    pub fn default() -> Self {
        Self {
            xs: 4.0,
            sm: 8.0,
            md: 16.0,
            lg: 24.0,
            xl: 32.0,
            xxl: 48.0,
        }
    }
}

/// Hiérarchie typographique.
#[derive(Clone, Debug)]
pub struct Typography {
    pub display: FontId,    // Titre principal (ARXFUND, sections majeures)
    pub h1: FontId,         // Titre de section
    pub h2: FontId,         // Sous-section
    pub body: FontId,       // Texte courant
    pub body_small: FontId, // Texte secondaire
    pub label: FontId,      // Labels de formulaire
    pub caption: FontId,    // Infos légères, états
    pub mono: FontId,       // Valeurs numériques (BPM, temps)
    pub mono_small: FontId, // Valeurs numériques petites
}

impl Typography {
    pub fn default() -> Self {
        Self {
            display: FontId::proportional(28.0),
            h1: FontId::proportional(18.0),
            h2: FontId::proportional(14.0),
            body: FontId::proportional(13.0),
            body_small: FontId::proportional(12.0),
            label: FontId::proportional(11.0),
            caption: FontId::proportional(10.0),
            mono: FontId::monospace(13.0),
            mono_small: FontId::monospace(11.0),
        }
    }
}

/// Thème complet ARXFUND.
#[derive(Clone, Debug)]
pub struct ARXFUNDTheme {
    pub colors: Colors,
    pub spacing: Spacing,
    pub radius: Radius,
    pub typography: Typography,
}

impl Default for ARXFUNDTheme {
    fn default() -> Self {
        Self::dark()
    }
}

impl ARXFUNDTheme {
    pub fn dark() -> Self {
        Self {
            colors: Colors::dark(),
            spacing: Spacing::default(),
            radius: Radius::default(),
            typography: Typography::default(),
        }
    }
}

/// Applique le thème ARXFUND à un contexte egui.
pub fn apply_theme(ctx: &eframe::egui::Context, theme: &ARXFUNDTheme) {
    let mut style = (*ctx.style_of(eframe::egui::Theme::Dark)).clone();
    style.visuals = eframe::egui::Visuals::dark();
    style.visuals.panel_fill = theme.colors.bg_primary;
    style.visuals.window_fill = theme.colors.bg_panel;
    style.visuals.extreme_bg_color = theme.colors.bg_secondary;
    style.visuals.selection.bg_fill = theme.colors.accent_primary;
    style.visuals.selection.stroke.color = theme.colors.text_primary;
    style.visuals.widgets.inactive.bg_fill = theme.colors.bg_panel;
    style.visuals.widgets.hovered.bg_fill = theme.colors.bg_surface;
    style.visuals.widgets.active.bg_fill = theme.colors.accent_primary;
    style.spacing.item_spacing = eframe::egui::vec2(8.0, 6.0);
    style.spacing.button_padding = eframe::egui::vec2(10.0, 6.0);
    style.text_styles.insert(eframe::egui::TextStyle::Body, FontId::proportional(14.0));
    style.text_styles.insert(eframe::egui::TextStyle::Button, FontId::proportional(13.0));
    style.text_styles.insert(eframe::egui::TextStyle::Small, FontId::proportional(12.0));
    ctx.set_theme(eframe::egui::Theme::Dark);
    ctx.set_style_of(eframe::egui::Theme::Dark, style);
}

/// Helpers de couleur.
impl Colors {
    pub fn accent_with_alpha(&self, alpha: f32) -> Color32 {
        Color32::from_rgba_unmultiplied(
            self.accent_primary.r(),
            self.accent_primary.g(),
            self.accent_primary.b(),
            (255.0 * alpha) as u8,
        )
    }
}
