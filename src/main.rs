#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use arxfund::app::StudioApp;
use arxfund::audio::AudioEngine;
use arxfund::library::Database;
use arxfund::paths;

const APP_NAME: &str = "ARXFUND";

fn load_icon() -> Option<eframe::egui::IconData> {
    let bytes = include_bytes!("../assets/arxfund.png");
    let image = image::load_from_memory(bytes).ok()?.into_rgba8();
    let (width, height) = image.dimensions();
    Some(eframe::egui::IconData {
        rgba: image.into_raw(),
        width,
        height,
    })
}

fn fatal(message: &str) -> ! {
    eprintln!("{APP_NAME} : {message}");
    let _ = rfd::MessageDialog::new()
        .set_title(APP_NAME)
        .set_description(message)
        .set_level(rfd::MessageLevel::Error)
        .show();
    std::process::exit(1);
}

fn main() -> eframe::Result<()> {
    // Migration non destructive des anciennes données (%APPDATA%\rust-music-studio).
    let mut startup_notes = paths::migrate_legacy_data();
    if let Err(e) = paths::ensure_user_dirs() {
        fatal(&format!(
            "Impossible de préparer les dossiers utilisateur :\n{e}\n\nVérifiez les droits d'accès à votre dossier Musique et à %APPDATA%."
        ));
    }
    startup_notes.extend(paths::seed_user_content());
    for note in &startup_notes {
        eprintln!("{APP_NAME} : {note}");
    }

    let data_dir = paths::data_dir();
    let covers_dir = paths::covers_dir();
    let db_path = paths::db_path();

    let db = match Database::open(&db_path) {
        Ok(db) => db,
        Err(e) => fatal(&format!(
            "Impossible d'ouvrir la bibliothèque ARXFUND.\n\n{}\n\nErreur : {e}",
            db_path.display()
        )),
    };

    // Si aucun périphérique audio n'est disponible, ARXFUND reste utilisable
    // (bibliothèque, édition, export) et affiche un message clair.
    let engine = match AudioEngine::new() {
        Ok(engine) => engine,
        Err(e) => {
            eprintln!("{APP_NAME} : sortie audio indisponible : {e}");
            AudioEngine::silent(format!(
                "Aucune sortie audio disponible : {e}. Branchez ou activez un périphérique audio, puis relancez ARXFUND. La bibliothèque et l'export restent utilisables."
            ))
        }
    };
    let audio_error = engine.take_error();

    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_inner_size([1280.0, 800.0])
        .with_min_inner_size([1024.0, 640.0])
        .with_title(APP_NAME)
        .with_app_id(APP_NAME);
    if let Some(icon) = load_icon() {
        viewport = viewport.with_icon(icon);
    }

    let native_options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        APP_NAME,
        native_options,
        Box::new(move |cc| {
            Ok(Box::new(StudioApp::new(
                cc,
                db,
                engine,
                db_path,
                data_dir,
                covers_dir,
                audio_error,
                startup_notes,
            )))
        }),
    )
}
