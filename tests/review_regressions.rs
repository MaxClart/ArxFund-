use arxfund::studio::{
    export::{ExportOptions, render_to_wav},
    project::{Clip, Note, Project, Step},
    samples::SampleBank,
    sequencer::Sequencer,
};
fn melody() -> Project {
    let mut p = Project::new_default("Review session");
    p.bpm = 120.0;
    p.patterns[0].notes[4].push(Note {
        pitch: 60,
        start: 0,
        length: 4,
        velocity: 0.7,
    });
    p
}
fn render(p: &Project, frames: usize) -> Vec<f32> {
    let mut out = vec![0.0; frames * 2];
    Sequencer::new(8000.0).render_block(p, &SampleBank::new(), false, &mut out);
    out
}
fn peak(x: &[f32]) -> f32 {
    x.iter().fold(0.0f32, |a, b| a.max(b.abs()))
}
#[test]
fn piano_notes_play_without_drum_steps_and_follow_song_pattern() {
    let mut p = melody();
    assert!(peak(&render(&p, 2000)) > 0.01);
    p.add_pattern();
    p.active_pattern = 1;
    p.song_mode = true;
    p.arrangement = vec![Clip {
        pattern: 0,
        start_step: 0,
        length_steps: 16,
    }];
    assert!(peak(&render(&p, 2000)) > 0.01);
}
#[test]
fn overlapping_clips_are_audible() {
    let mut p = melody();
    p.add_pattern();
    p.song_mode = true;
    p.arrangement = vec![
        Clip {
            pattern: 1,
            start_step: 0,
            length_steps: 16,
        },
        Clip {
            pattern: 0,
            start_step: 0,
            length_steps: 16,
        },
    ];
    assert!(peak(&render(&p, 2000)) > 0.01);
}
#[test]
fn faders_pan_mute_and_meters_affect_sustaining_notes() {
    let mut p = melody();
    let mut seq = Sequencer::new(8000.0);
    let mut out = vec![0.0; 800];
    seq.render_block(&p, &SampleBank::new(), false, &mut out);
    assert!(seq.channel_peaks[4] > 0.01);
    assert_eq!(seq.channel_peaks[0], 0.0);
    p.channels[4].pan = 1.0;
    seq.render_block(&p, &SampleBank::new(), false, &mut out);
    assert!(out.chunks_exact(2).all(|f| f[0].abs() < 1e-6));
    assert!(peak(&out) > 0.01);
    p.channels[4].volume = 0.0;
    seq.render_block(&p, &SampleBank::new(), false, &mut out);
    assert_eq!(peak(&out), 0.0);
    p.channels[4].volume = 1.0;
    p.channels[4].mute = true;
    seq.render_block(&p, &SampleBank::new(), false, &mut out);
    assert_eq!(peak(&out), 0.0);
    assert_eq!(seq.master_peak, 0.0);
}
#[test]
fn delay_changes_audio_and_resets_deterministically() {
    let mut p = melody();
    let dry = render(&p, 8000);
    p.channels[4].delay_mix = 0.7;
    let wet = render(&p, 8000);
    assert!(dry.iter().zip(&wet).any(|(a, b)| (a - b).abs() > 0.01));
    let mut seq = Sequencer::new(8000.0);
    let mut out = vec![0.0; 16000];
    seq.render_block(&p, &SampleBank::new(), false, &mut out);
    seq.reset();
    seq.render_block(&p, &SampleBank::new(), false, &mut out);
    assert!(out == wet, "reset must clear all delay state");
}
#[test]
fn export_stop_is_sample_exact_inside_a_block() {
    let mut p = Project::new_default("stop");
    p.bpm = 120.0;
    p.patterns[0].lanes[0][1] = Step::accent();
    let mut seq = Sequencer::new(8000.0);
    seq.set_stop_at(Some(1000.0));
    let mut out = vec![0.0; 8000];
    seq.render_block(&p, &SampleBank::new(), false, &mut out);
    assert_eq!(peak(&out), 0.0);
}
#[test]
fn save_reopen_export_all_depths_and_monotonic_progress() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("review.rmsproj");
    let mut p = melody();
    p.channels[4].delay_mix = 0.4;
    p.save_to(&path).unwrap();
    p.name = "Updated review".into();
    p.save_to(&path).unwrap();
    let loaded = Project::load_from(&path).unwrap();
    assert_eq!(loaded.name, p.name);
    assert_eq!(loaded.patterns, p.patterns);
    assert_eq!(loaded.channels, p.channels);
    for depth in [16, 24, 32] {
        let wav = dir.path().join(format!("review-{depth}.wav"));
        let options = ExportOptions {
            sample_rate: 8000,
            bars: 1,
            tail_secs: 0.5,
            bit_depth: depth,
            ..Default::default()
        };
        let mut last = 0.0;
        let result = render_to_wav(&loaded, &SampleBank::new(), &wav, &options, |v| {
            assert!(v >= last);
            last = v;
        })
        .unwrap();
        assert_eq!(result.frames, 20000);
        assert_eq!(last, 1.0);
        let reader = hound::WavReader::open(&wav).unwrap();
        assert_eq!(reader.spec().bits_per_sample, depth);
        assert_eq!(reader.duration(), 20000);
        assert!(reader.into_samples::<i32>().any(|s| s.unwrap() != 0));
        // Invalid options must not truncate an existing export.
        let original = std::fs::read(&wav).unwrap();
        let invalid = ExportOptions {
            bit_depth: 8,
            ..options
        };
        assert!(render_to_wav(&loaded, &SampleBank::new(), &wav, &invalid, |_| {}).is_err());
        assert_eq!(std::fs::read(&wav).unwrap(), original);
    }
}
