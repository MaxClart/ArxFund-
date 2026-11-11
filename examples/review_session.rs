//! Reproducible end-to-end fixture, never writes to personal folders.
use arxfund::{
    library::{Database, import_one},
    studio::{
        export::{ExportOptions, render_to_wav},
        project::{Clip, Note, Project, Step},
        samples::SampleBank,
    },
};
fn main() -> Result<(), String> {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let artifacts = root.join("review-artifacts");
    std::fs::create_dir_all(&artifacts).map_err(|e| e.to_string())?;
    let portable = root.join("target/debug/portable-data");
    let projects = portable.join("Music/Projects");
    std::fs::create_dir_all(&projects).map_err(|e| e.to_string())?;
    let data = portable.join("Data");
    std::fs::create_dir_all(&data).map_err(|e| e.to_string())?;
    let mut p = Project::new_default("Midnight circuit — Review");
    p.bpm = 112.0;
    for step in [0, 4, 8, 12] {
        p.patterns[0].lanes[0][step] = Step::accent();
    }
    for step in [4, 12] {
        p.patterns[0].lanes[1][step] = Step::accent();
    }
    for step in (0..16).step_by(2) {
        p.patterns[0].lanes[2][step] = Step::accent();
    }
    for (start, pitch) in [(0, 60), (4, 63), (8, 67), (12, 65)] {
        p.patterns[0].notes[4].push(Note {
            start,
            pitch,
            length: 3,
            velocity: 0.65,
        });
    }
    p.channels[4].delay_mix = 0.3;
    p.channels[4].pan = 0.25;
    p.song_mode = true;
    p.arrangement = vec![Clip {
        pattern: 0,
        start_step: 0,
        length_steps: 32,
    }];
    let path = artifacts.join("Midnight-circuit.rmsproj");
    p.save_to(&path)?;
    let loaded = Project::load_from(&path)?;
    loaded.save_to(&projects.join("Review.rmsproj"))?;
    let wav = artifacts.join("Midnight-circuit.wav");
    let result = render_to_wav(
        &loaded,
        &SampleBank::new(),
        &wav,
        &ExportOptions {
            bars: 2,
            bit_depth: 24,
            ..Default::default()
        },
        |_| {},
    )?;
    let db = Database::open(&data.join("library.sqlite")).map_err(|e| e.to_string())?;
    import_one(&db, &data.join("covers"), &wav)?;
    let songs = db
        .list_songs(
            arxfund::library::SortField::Title,
            arxfund::library::SortDir::Asc,
        )
        .map_err(|e| e.to_string())?;
    if db.list_playlists().map_err(|e| e.to_string())?.is_empty() {
        db.create_playlist("review", "Session de test", None)
            .map_err(|e| e.to_string())?;
    }
    for song in songs {
        db.add_track_to_playlist("review", &song.id)
            .map_err(|e| e.to_string())?;
    }
    println!(
        "Saved, reopened, rendered and imported: {} frames, {:.3}s, peak {:.3}",
        result.frames, result.duration_secs, result.peak
    );
    Ok(())
}
