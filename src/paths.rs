//! Emplacements de fichiers d'ARXFUND.
//!
//! Séparation stricte :
//! - `%APPDATA%\ARXFUND` : base de données, paramètres, pochettes, sauvegardes.
//! - `Musique\ARXFUND`    : projets, exports, enregistrements, samples et presets
//!                          personnels (modifiables sans droits administrateur).
//! - Dossier d'installation : ressources en lecture seule (presets et samples
//!   de démonstration fournis avec l'application).
//!
//! Aucune donnée personnelle n'est écrite dans Program Files.

use std::fs;
use std::path::{Path, PathBuf};

pub const PRODUCT_NAME: &str = "ARXFUND";
pub const LEGACY_DIR_NAME: &str = "rust-music-studio";

/// `%APPDATA%\ARXFUND` sous Windows.
fn isolated_root() -> Option<PathBuf> {
    std::env::var_os("ARXFUND_TEST_ROOT")
        .map(PathBuf::from)
        .or_else(|| {
            let exe = std::env::current_exe().ok()?;
            let parent = exe.parent()?;
            parent
                .join("ARXFUND.portable")
                .is_file()
                .then(|| parent.join("portable-data"))
        })
}

pub fn data_dir() -> PathBuf {
    if let Some(root) = isolated_root() {
        return root.join("Data");
    }
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(PRODUCT_NAME)
}

/// Ancien emplacement (`%APPDATA%\rust-music-studio`), conservé pour migration.
pub fn legacy_data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(LEGACY_DIR_NAME)
}

pub fn db_path() -> PathBuf {
    data_dir().join("library.sqlite")
}

pub fn covers_dir() -> PathBuf {
    data_dir().join("covers")
}

pub fn backups_dir() -> PathBuf {
    data_dir().join("backups")
}

/// Racine des contenus personnels : `Musique\ARXFUND` (ou Documents en secours).
pub fn music_root() -> PathBuf {
    if let Some(root) = isolated_root() {
        return root.join("Music");
    }
    dirs::audio_dir()
        .or_else(dirs::document_dir)
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")))
        .join(PRODUCT_NAME)
}

pub fn projects_dir() -> PathBuf {
    music_root().join("Projects")
}

pub fn exports_dir() -> PathBuf {
    music_root().join("Exports")
}

pub fn recordings_dir() -> PathBuf {
    music_root().join("Recordings")
}

pub fn user_samples_dir() -> PathBuf {
    music_root().join("Samples")
}

pub fn user_presets_dir() -> PathBuf {
    music_root().join("Presets")
}

/// Dossier des ressources livrées avec l'application (lecture seule).
pub fn resources_dir() -> PathBuf {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."));
    let resources = exe_dir.join("resources");
    if resources.is_dir() {
        resources
    } else {
        exe_dir
    }
}

pub fn installed_presets_dir() -> PathBuf {
    resources_dir().join("presets")
}

pub fn installed_samples_dir() -> PathBuf {
    resources_dir().join("samples")
}

/// Crée tous les dossiers utilisateur nécessaires.
pub fn ensure_user_dirs() -> std::io::Result<()> {
    for dir in [
        data_dir(),
        covers_dir(),
        backups_dir(),
        music_root(),
        projects_dir(),
        exports_dir(),
        recordings_dir(),
        user_samples_dir(),
        user_presets_dir(),
    ] {
        fs::create_dir_all(&dir)?;
    }
    Ok(())
}

/// Copie récursivement les éléments manquants de `src` vers `dst`.
/// Ne remplace et ne supprime jamais un fichier existant.
fn copy_missing(src: &Path, dst: &Path, notes: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(src) else {
        return;
    };
    for entry in entries.flatten() {
        let from = entry.path();
        let Some(name) = from.file_name() else {
            continue;
        };
        let to = dst.join(name);
        if from.is_dir() {
            if let Err(e) = fs::create_dir_all(&to) {
                notes.push(format!("Dossier non migré {} : {e}", from.display()));
                continue;
            }
            copy_missing(&from, &to, notes);
        } else if from.is_file() && !to.exists() {
            match fs::copy(&from, &to) {
                Ok(_) => notes.push(format!("Migré : {}", to.display())),
                Err(e) => notes.push(format!("Fichier non migré {} : {e}", from.display())),
            }
        }
    }
}

/// Migration non destructive de `%APPDATA%\rust-music-studio` vers
/// `%APPDATA%\ARXFUND`. L'ancien dossier est conservé intact.
///
/// La migration est idempotente : les fichiers déjà présents dans le nouvel
/// emplacement ne sont jamais écrasés.
pub fn migrate_legacy_data() -> Vec<String> {
    let mut notes = Vec::new();
    if isolated_root().is_some() {
        return notes;
    }
    let legacy = legacy_data_dir();
    let current = data_dir();
    if !legacy.is_dir() || legacy == current {
        return notes;
    }
    if let Err(e) = fs::create_dir_all(&current) {
        notes.push(format!(
            "Migration impossible (création {} : {e})",
            current.display()
        ));
        return notes;
    }
    copy_missing(&legacy, &current, &mut notes);
    if !notes.is_empty() {
        notes.insert(
            0,
            format!(
                "Migration des données depuis {} (ancien dossier conservé).",
                legacy.display()
            ),
        );
    }
    notes
}

/// Copie les presets et samples fournis avec l'application vers les dossiers
/// personnels, sans jamais écraser une création de l'utilisateur.
pub fn seed_user_content() -> Vec<String> {
    let mut notes = Vec::new();
    let sources = [
        (installed_presets_dir(), user_presets_dir()),
        (installed_samples_dir(), user_samples_dir()),
    ];
    for (src, dst) in sources {
        if !src.is_dir() {
            continue;
        }
        if let Err(e) = fs::create_dir_all(&dst) {
            notes.push(format!("Création impossible {} : {e}", dst.display()));
            continue;
        }
        copy_missing(&src, &dst, &mut notes);
    }
    notes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_paths_are_separate_from_installed_resources() {
        assert!(
            projects_dir().ends_with("ARXFUND\\Projects")
                || projects_dir().ends_with("ARXFUND/Projects")
        );
        assert!(data_dir().ends_with("ARXFUND"));
        assert!(!data_dir().starts_with(resources_dir().parent().unwrap_or(Path::new("."))));
    }
}
