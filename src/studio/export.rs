//! Export hors ligne du projet en fichier WAV.
//!
//! Le rendu utilise exactement le même séquenceur et les mêmes voix que la
//! lecture temps réel, ce qui garantit la cohérence entre écoute et export.

use std::path::{Path, PathBuf};

use hound::{SampleFormat as HoundSampleFormat, WavSpec, WavWriter};

use crate::studio::project::{Project, STEPS_PER_BEAT};
use crate::studio::samples::SampleBank;
use crate::studio::sequencer::Sequencer;

#[derive(Clone, Debug)]
pub struct ExportOptions {
    pub sample_rate: u32,
    pub bars: usize,
    pub tail_secs: f32,
    pub normalize: bool,
    pub bit_depth: u16,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            sample_rate: 44_100,
            bars: 1,
            tail_secs: 1.0,
            normalize: true,
            bit_depth: 16,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ExportResult {
    pub path: PathBuf,
    pub frames: u64,
    pub duration_secs: f64,
    pub peak: f32,
    pub sample_rate: u32,
    pub bit_depth: u16,
}

/// Rend le projet et écrit un WAV stéréo PCM.
///
/// `progress` reçoit une valeur 0..1 pendant le rendu.
pub fn render_to_wav(
    project: &Project,
    bank: &SampleBank,
    path: &Path,
    options: &ExportOptions,
    mut progress: impl FnMut(f32),
) -> Result<ExportResult, String> {
    if options.bars == 0 {
        return Err("Le nombre de mesures doit être supérieur à 0".to_string());
    }
    let rate = options.sample_rate.clamp(8_000, 192_000);
    let bar_steps = STEPS_PER_BEAT * project.time_sig_num.max(1) as usize;
    let seq_probe = Sequencer::new(rate as f32);
    let sps = seq_probe.samples_per_step(project);
    let musical_frames = (options.bars as f64 * bar_steps as f64 * sps).round() as u64;
    let tail_frames = (options.tail_secs.max(0.0) as f64 * rate as f64).round() as u64;
    let total_frames = musical_frames + tail_frames;
    if total_frames == 0 {
        return Err("Durée d'export nulle".to_string());
    }

    let mut seq = Sequencer::new(rate as f32);
    seq.set_stop_at(Some(musical_frames as f64));
    let mut samples: Vec<f32> = vec![0.0; (total_frames as usize).saturating_mul(2)];
    let chunk_frames = 4096usize;
    let mut done = 0u64;
    let mut peak = 0.0f32;
    while done < total_frames {
        let frames = ((total_frames - done) as usize).min(chunk_frames);
        let start = done as usize * 2;
        let end = start + frames * 2;
        seq.render_block(project, bank, false, &mut samples[start..end]);
        for s in &samples[start..end] {
            peak = peak.max(s.abs());
        }
        done += frames as u64;
        progress(done as f32 / total_frames as f32);
    }

    let gain = if options.normalize && peak > 1e-6 {
        0.98 / peak
    } else {
        1.0
    };
    let master_peak = (peak * gain).min(1.0);

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let spec = WavSpec {
        channels: 2,
        sample_rate: rate,
        bits_per_sample: options.bit_depth,
        sample_format: HoundSampleFormat::Int,
    };
    let mut writer = WavWriter::create(path, spec).map_err(|e| format!("WAV : {e}"))?;
    let mut written = 0u64;
    for frame in samples.chunks_exact(2) {
        let l = (frame[0] * gain).clamp(-1.0, 1.0);
        let r = (frame[1] * gain).clamp(-1.0, 1.0);
        write_frame(&mut writer, l, r, options.bit_depth)?;
        written += 1;
        if written % 8192 == 0 {
            progress((written as f32 / total_frames as f32).min(1.0));
        }
    }
    writer
        .finalize()
        .map_err(|e| format!("Finalisation WAV : {e}"))?;
    progress(1.0);

    Ok(ExportResult {
        path: path.to_path_buf(),
        frames: total_frames,
        duration_secs: total_frames as f64 / rate as f64,
        peak: master_peak,
        sample_rate: rate,
        bit_depth: options.bit_depth,
    })
}

fn write_frame(
    writer: &mut WavWriter<std::io::BufWriter<std::fs::File>>,
    l: f32,
    r: f32,
    bit_depth: u16,
) -> Result<(), String> {
    match bit_depth {
        16 => {
            let scale = i16::MAX as f32;
            writer
                .write_sample((l * scale).round() as i16)
                .map_err(|e| e.to_string())?;
            writer
                .write_sample((r * scale).round() as i16)
                .map_err(|e| e.to_string())?;
        }
        24 => {
            let scale = 8_388_607.0f32;
            writer
                .write_sample((l * scale).round() as i32)
                .map_err(|e| e.to_string())?;
            writer
                .write_sample((r * scale).round() as i32)
                .map_err(|e| e.to_string())?;
        }
        32 => {
            let scale = i32::MAX as f32;
            writer
                .write_sample((l * scale).round() as i32)
                .map_err(|e| e.to_string())?;
            writer
                .write_sample((r * scale).round() as i32)
                .map_err(|e| e.to_string())?;
        }
        other => return Err(format!("Profondeur non supportée : {other} bits")),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::studio::project::Step;

    #[test]
    fn export_wav_has_expected_duration_and_audio() {
        let mut p = Project::new_default("Export test");
        p.bpm = 120.0;
        p.steps = 16;
        p.patterns[0].lanes[0][0] = Step::accent();
        p.patterns[0].lanes[2][2] = Step::accent();
        let opts = ExportOptions {
            sample_rate: 44_100,
            bars: 1,
            tail_secs: 0.5,
            normalize: true,
            bit_depth: 16,
        };
        let dir = std::env::temp_dir().join("rms-export-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!(
            "test-{}.wav",
            crate::studio::project::unique_id("x")
        ));
        let result = render_to_wav(&p, &SampleBank::new(), &path, &opts, |_| {}).expect("export");
        let expected = 44_100.0 * (2.0 + 0.5);
        assert!(
            (result.frames as f64 - expected).abs() < 2000.0,
            "frames = {}",
            result.frames
        );
        assert!(result.peak > 0.05, "pic = {}", result.peak);
        let data = std::fs::read(&path).unwrap();
        assert!(data.len() > 44);
        let _ = std::fs::remove_file(&path);
    }
}
