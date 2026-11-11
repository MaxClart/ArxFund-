//! Système de design ARXFUND — composants et thème centralisés.

pub mod audio_components;
pub mod components;
pub mod player;
pub mod theme;

pub use audio_components::{
    audio_meter as audio_meter_v2, bpm_display, bpm_label, knob, mute_button, solo_button,
    vertical_slider,
};
pub use components::{
    BadgeKind, Modal, NotificationKind, NotificationManager, audio_meter, badge, button,
    button_small, card, card_v2, context_menu, divider, dropdown, empty_state, icon_button,
    icon_button_small, search_field, section_divider, section_header, text_button, toggle,
    track_header,
};
pub use theme::{ARXFUNDTheme, Colors, Radius, Spacing, Typography};
