//! Génère les presets de projets et les samples de démonstration d'ARXFUND.
//!
//! Usage : cargo run --release --example make_assets
//! Les fichiers sont écrits dans resources/presets et resources/samples.
//! Tous les sons sont synthétisés proceduralement : aucune ressource tierce.

use std::f32::consts::TAU;
use std::fs;
use std::path::Path;

use arxfund::studio::project::{ChannelKind, Project, Step};
use hound::{SampleFormat, WavSpec, WavWriter};

fn channel_index(project: &Project, kind: ChannelKind) -> usize {
    project
        .channels
        .iter()
        .position(|c| c.kind == kind)
        .unwrap_or_else(|| panic!("canal absent : {kind:?}"))
}

fn set_steps(project: &mut Project, kind: ChannelKind, steps: &[(usize, f32)]) {
    let index = channel_index(project, kind);
    let pattern = project.active_pattern.min(project.patterns.len() - 1);
    let lane = &mut project.patterns[pattern].lanes[index];
    for &(step, velocity) in steps {
        if let Some(target) = lane.get_mut(step) {
            *target = Step {
                on: true,
                velocity,
                ..Step::default()
            };
        }
    }
}

fn set_ratchet(project: &mut Project, kind: ChannelKind, step: usize, ratchet: u8) {
    let index = channel_index(project, kind);
    let pattern = project.active_pattern.min(project.patterns.len() - 1);
    if let Some(target) = project.patterns[pattern].lanes[index].get_mut(step) {
        target.on = true;
        target.ratchet = ratchet.clamp(1, 4);
    }
}

fn base_project(name: &str, bpm: f64, swing: f32) -> Project {
    let mut project = Project::new_default(name);
    project.bpm = bpm;
    project.swing = swing;
    project.steps = 16;
    project.ensure_invariants();
    project
}

fn boom_bap() -> Project {
    let mut project = base_project("ARXFUND — Boom Bap 90", 90.0, 0.16);
    for kind in [
        ChannelKind::Kick,
        ChannelKind::Snare,
        ChannelKind::HatClosed,
        ChannelKind::Bass808,
    ] {
        if !project.channels.iter().any(|c| c.kind == kind) {
            project.add_channel(kind, None);
        }
    }
    set_steps(
        &mut project,
        ChannelKind::Kick,
        &[(0, 1.0), (3, 0.7), (6, 0.95), (10, 0.8)],
    );
    set_steps(&mut project, ChannelKind::Snare, &[(4, 1.0), (12, 1.0)]);
    set_steps(
        &mut project,
        ChannelKind::HatClosed,
        &[
            (0, 0.5),
            (2, 0.8),
            (4, 0.55),
            (6, 0.85),
            (8, 0.5),
            (10, 0.8),
            (12, 0.55),
            (14, 0.9),
        ],
    );
    set_steps(
        &mut project,
        ChannelKind::Bass808,
        &[(0, 0.9), (6, 0.85), (10, 0.8), (14, 0.9)],
    );
    project
}

fn trap() -> Project {
    let mut project = base_project("ARXFUND — Trap 140", 140.0, 0.0);
    for kind in [
        ChannelKind::Kick,
        ChannelKind::Clap,
        ChannelKind::HatClosed,
        ChannelKind::Bass808,
    ] {
        if !project.channels.iter().any(|c| c.kind == kind) {
            project.add_channel(kind, None);
        }
    }
    set_steps(
        &mut project,
        ChannelKind::Kick,
        &[(0, 1.0), (7, 0.9), (10, 0.95)],
    );
    set_steps(&mut project, ChannelKind::Clap, &[(4, 1.0), (12, 1.0)]);
    set_steps(
        &mut project,
        ChannelKind::HatClosed,
        &(0..16)
            .map(|step| (step, if step % 2 == 0 { 0.8 } else { 0.55 }))
            .collect::<Vec<_>>(),
    );
    for step in [2usize, 6, 10, 14] {
        set_ratchet(&mut project, ChannelKind::HatClosed, step, 2);
    }
    set_steps(
        &mut project,
        ChannelKind::Bass808,
        &[(0, 1.0), (6, 0.9), (10, 1.0), (14, 0.85)],
    );
    project
}

fn house() -> Project {
    let mut project = base_project("ARXFUND — House 124", 124.0, 0.0);
    for kind in [
        ChannelKind::Kick,
        ChannelKind::Clap,
        ChannelKind::HatOpen,
        ChannelKind::Bass808,
        ChannelKind::Perc,
    ] {
        if !project.channels.iter().any(|c| c.kind == kind) {
            project.add_channel(kind, None);
        }
    }
    set_steps(
        &mut project,
        ChannelKind::Kick,
        &[(0, 1.0), (4, 1.0), (8, 1.0), (12, 1.0)],
    );
    set_steps(&mut project, ChannelKind::Clap, &[(4, 0.9), (12, 0.9)]);
    set_steps(
        &mut project,
        ChannelKind::HatOpen,
        &[(2, 0.7), (6, 0.75), (10, 0.7), (14, 0.8)],
    );
    set_steps(
        &mut project,
        ChannelKind::Bass808,
        &[(0, 0.85), (4, 0.8), (8, 0.85), (12, 0.8)],
    );
    set_steps(&mut project, ChannelKind::Perc, &[(3, 0.5), (11, 0.55)]);
    project
}

fn write_project(dir: &Path, project: &Project) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{}.rmsproj", project.name));
    project.save_to(&path)
}

fn write_wav(dir: &Path, name: &str, samples: &[f32]) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let spec = WavSpec {
        channels: 1,
        sample_rate: 44_100,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };
    let path = dir.join(name);
    let mut writer = WavWriter::create(&path, spec).map_err(|e| e.to_string())?;
    for &sample in samples {
        let value = (sample.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16;
        writer.write_sample(value).map_err(|e| e.to_string())?;
    }
    writer.finalize().map_err(|e| e.to_string())
}

fn kick() -> Vec<f32> {
    let rate = 44_100.0f32;
    let mut phase = 0.0f32;
    (0..(0.55 * rate) as usize)
        .map(|i| {
            let t = i as f32 / rate;
            let freq = 45.0 + 95.0 * (-t * 26.0).exp();
            phase += TAU * freq / rate;
            let body = phase.sin() * (-t * 7.0).exp();
            let click = (phase * 2.0).sin() * (-t * 150.0).exp() * 0.25;
            (body + click) * 0.9
        })
        .collect()
}

fn snare() -> Vec<f32> {
    let rate = 44_100.0f32;
    let mut noise = 0x1234_5678u32;
    (0..(0.30 * rate) as usize)
        .map(|i| {
            let t = i as f32 / rate;
            noise ^= noise << 13;
            noise ^= noise >> 17;
            noise ^= noise << 5;
            let n = (noise as f32 / u32::MAX as f32) * 2.0 - 1.0;
            let tone = (TAU * 190.0 * t).sin() * (-t * 24.0).exp() * 0.35;
            (n * (-t * 28.0).exp() * 0.65 + tone) * 0.85
        })
        .collect()
}

fn hihat() -> Vec<f32> {
    let rate = 44_100.0f32;
    let mut noise = 0x9E37_79B9u32;
    let mut previous = 0.0f32;
    (0..(0.16 * rate) as usize)
        .map(|i| {
            let t = i as f32 / rate;
            noise ^= noise << 13;
            noise ^= noise >> 17;
            noise ^= noise << 5;
            let n = (noise as f32 / u32::MAX as f32) * 2.0 - 1.0;
            let high = n - previous;
            previous = n;
            high * (-t * 85.0).exp() * 0.45
        })
        .collect()
}

fn main() -> Result<(), String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let presets = root.join("resources").join("presets");
    let samples = root.join("resources").join("samples");

    write_project(&presets, &boom_bap())?;
    write_project(&presets, &trap())?;
    write_project(&presets, &house())?;

    write_wav(&samples, "Kick Demo.wav", &kick())?;
    write_wav(&samples, "Snare Demo.wav", &snare())?;
    write_wav(&samples, "HiHat Demo.wav", &hihat())?;

    fs::create_dir_all(&presets).map_err(|e| e.to_string())?;
    fs::write(
        presets.join("LISEZ-MOI.txt"),
        "Presets de projets ARXFUND.\n\nCes projets sont des points de départ originaux.\nOuvrez-les depuis Studio de production > Ouvrir, puis sauvegardez votre propre version.\n",
    )
    .map_err(|e| e.to_string())?;
    fs::create_dir_all(&samples).map_err(|e| e.to_string())?;
    fs::write(
        samples.join("LISEZ-MOI.txt"),
        "Samples de démonstration ARXFUND.\n\nKick, Snare et HiHat sont synthétisés proceduralement pour ce projet.\nIls peuvent être chargés sur un canal « Échantillon » du Channel Rack.\n",
    )
    .map_err(|e| e.to_string())?;

    println!(
        "Presets et samples générés dans {}",
        root.join("resources").display()
    );
    Ok(())
}
