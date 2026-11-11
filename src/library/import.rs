use crate::library::db::Database;
use crate::library::models::Song;
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::picture::MimeType;
use lofty::probe::Probe;
use lofty::tag::Accessor;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

fn new_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("song-{}", t)
}

const AUDIO_EXTS: &[&str] = &["mp3", "wav", "flac", "ogg", "oga", "opus", "m4a", "aac"];

pub fn is_audio_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| AUDIO_EXTS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

pub fn collect_audio_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        if let Ok(entries) = fs::read_dir(&d) {
            for e in entries.flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.is_file() && is_audio_file(&p) {
                    out.push(p);
                }
            }
        }
    }
    out
}

pub fn hash_file(path: &Path) -> Result<String, std::io::Error> {
    let data = fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&data);
    Ok(hasher
        .finalize()
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect())
}

pub struct ImportResult {
    pub imported: usize,
    pub duplicates: usize,
    pub errors: Vec<(PathBuf, String)>,
}

pub fn import_one(db: &Database, covers_dir: &Path, path: &Path) -> Result<Option<Song>, String> {
    if !is_audio_file(path) {
        return Err("Format audio non supporté".to_string());
    }
    let hash = hash_file(path).map_err(|e| e.to_string())?;

    // Dédoublonnage
    let exists = db.count_hash(&hash).map_err(|e| e.to_string())?;
    if exists > 0 {
        return Ok(None);
    }

    let tagged = Probe::open(path).map_err(|e| format!("Lecture impossible : {}", e))?;
    let tagged = tagged
        .read()
        .map_err(|e| format!("Lecture impossible : {}", e))?;

    let tag = tagged.primary_tag();
    let title = tag
        .and_then(|t| t.title().map(|s| s.to_string()))
        .unwrap_or_else(|| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Sans titre")
                .to_string()
        });
    let artist = tag.and_then(|t| t.artist().map(|s| s.to_string()));
    let album = tag.and_then(|t| t.album().map(|s| s.to_string()));
    let duration = tagged.properties().duration().as_secs_f64();

    // Pochette
    let mut cover_path = None;
    if let Some(tag) = tag {
        if let Some(pic) = tag.pictures().first() {
            let _ = fs::create_dir_all(covers_dir);
            let ext = match pic.mime_type() {
                Some(MimeType::Png) => "png",
                _ => "jpg",
            };
            let cp = covers_dir.join(format!("{}.{}", hash, ext));
            if fs::write(&cp, pic.data()).is_ok() {
                cover_path = Some(cp.to_string_lossy().to_string());
            }
        }
    }

    let song = Song {
        id: new_id(),
        title,
        artist,
        album,
        duration: if duration > 0.0 { Some(duration) } else { None },
        cover_path,
        provider: crate::library::models::Provider::Local,
        provider_track_id: None,
        source_url: None,
        local_path: Some(path.to_string_lossy().to_string()),
        audio_available: true,
        is_favorite: false,
        added_at: now_ms(),
    };
    let inserted = db
        .insert_song(&song, Some(&hash))
        .map_err(|e| e.to_string())?;
    if !inserted {
        return Ok(None);
    }
    Ok(Some(song))
}

pub fn import_files(db: &Database, covers_dir: &Path, paths: &[PathBuf]) -> ImportResult {
    let mut imported = 0;
    let mut duplicates = 0;
    let mut errors = Vec::new();
    for p in paths {
        match import_one(db, covers_dir, p) {
            Ok(Some(_)) => imported += 1,
            Ok(None) => duplicates += 1,
            Err(e) => errors.push((p.clone(), e)),
        }
    }
    ImportResult {
        imported,
        duplicates,
        errors,
    }
}

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
