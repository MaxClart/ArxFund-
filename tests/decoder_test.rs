use arxfund::audio::AudioEngine;
use hound::{WavSpec, WavWriter};
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

fn write_wav(path: &PathBuf) {
    let spec = WavSpec {
        channels: 1,
        sample_rate: 8000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = WavWriter::create(path, spec).unwrap();
    for i in 0..240_000 {
        let s = ((i as f32 * 440.0 * 2.0 * std::f32::consts::PI / 8000.0).sin() * 16000.0) as i16;
        w.write_sample(s).unwrap();
    }
    w.finalize().unwrap();
}

#[test]
fn engine_loads_and_decodes_wav() {
    let dir = std::env::temp_dir().join("rms-decoder-test");
    let _ = std::fs::create_dir_all(&dir);
    let wav = dir.join("tone.wav");
    write_wav(&wav);

    let engine = AudioEngine::new().expect("engine");
    engine.load(wav.to_string_lossy().to_string(), false);
    // Wait for decoder to parse.
    thread::sleep(Duration::from_millis(500));
    assert!(engine.take_error().is_none(), "erreur décodeur");
    let ti = engine.track().expect("track info");
    assert!(
        ti.duration > 0.5,
        "durée attendue > 0.5s, got {}",
        ti.duration
    );
    // Play and wait for some frames.
    engine.play();
    thread::sleep(Duration::from_millis(300));
    assert!(engine.position_secs() > 0.05, "position attendue > 0.05s");
    engine.pause();
    thread::sleep(Duration::from_millis(150));
    assert!(
        !engine.is_playing(),
        "pause must survive a full decoder buffer"
    );
    engine.seek(15.0);
    thread::sleep(Duration::from_millis(150));
    assert!(
        (engine.position_secs() - 15.0).abs() < 0.3,
        "seek must report the new position: {}",
        engine.position_secs()
    );
    engine.play();
    thread::sleep(Duration::from_millis(150));
    assert!(engine.position_secs() > 15.05);
    engine.stop();
    let _ = std::fs::remove_dir_all(&dir);
}
