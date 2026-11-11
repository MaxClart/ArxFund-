from pathlib import Path

def edit(name, fn):
    p = Path(name)
    p.write_text(fn(p.read_text(encoding='utf-8')), encoding='utf-8')

def paths(s):
    s = s.replace('    dirs::data_dir()\n', '    if let Some(root) = std::env::var_os("ARXFUND_TEST_ROOT") {\n        return PathBuf::from(root).join("Data");\n    }\n    dirs::data_dir()\n', 1)
    s = s.replace('    dirs::audio_dir()\n', '    if let Some(root) = std::env::var_os("ARXFUND_TEST_ROOT") {\n        return PathBuf::from(root).join("Music");\n    }\n    dirs::audio_dir()\n', 1)
    s = s.replace('    let mut notes = Vec::new();\n    let legacy', '    let mut notes = Vec::new();\n    if std::env::var_os("ARXFUND_TEST_ROOT").is_some() { return notes; }\n    let legacy')
    return s
edit('src/paths.rs', paths)

def engine(s):
    s = s.replace('    pub frames_played: AtomicU64,', '    pub frames_played: AtomicU64,\n    pub position_base: AtomicU64,')
    s = s.replace('            frames_played: AtomicU64::new(0),', '            frames_played: AtomicU64::new(0),\n            position_base: AtomicU64::new(0),')
    s = s.replace('shared_cb.frames_played.store(0, Ordering::Relaxed);', 'shared_cb.frames_played.store(shared_cb.position_base.load(Ordering::Relaxed), Ordering::Relaxed);')
    s = s.replace('let frames = (data.len() / 2) as u64;', 'let frames = (n / 2) as u64;')
    s = s.replace('self.shared.ended.load(Ordering::Acquire)', 'self.shared.ended.swap(false, Ordering::AcqRel)')
    s = s.replace('    let mut resample_needed = false;', '    let mut resample_needed = false;\n    let mut pending = Vec::new();\n    let mut pending_pos = 0;')
    s = s.replace('// reset state\n', '// reset state\n                    pending.clear();\n                    pending_pos = 0;\n                    shared.position_base.store(0, Ordering::Relaxed);\n                    *shared.track.lock() = None;\n')
    a = s.index('                        let _ = fmt.seek(')
    b = s.index('\n                    }\n                }', a)
    s = s[:a] + '''                        match fmt.seek(SeekMode::Coarse, to) {
                            Ok(result) => {
                                if let Some(dec) = decoder.as_mut() { dec.reset(); }
                                let actual = fmt.tracks().iter().find(|t| t.id == result.track_id)
                                    .and_then(|t| t.time_base)
                                    .map(|tb| result.actual_ts.get() as f64 * tb.numer.get() as f64 / tb.denom.get() as f64)
                                    .unwrap_or(secs.max(0.0));
                                pending.clear();
                                pending_pos = 0;
                                shared.position_base.store((actual * device_rate as f64) as u64, Ordering::Relaxed);
                                shared.finished.store(false, Ordering::Relaxed);
                                shared.ended.store(false, Ordering::Relaxed);
                                shared.epoch.fetch_add(1, Ordering::AcqRel);
                            }
                            Err(e) => *shared.error.lock() = Some(format!("Déplacement impossible : {e}")),
                        }''' + s[b:]
    marker = '        let Some(fmt) = format.as_mut() else {'
    s = s.replace(marker, '''        // The consumer acknowledges the flush before new audio is queued.
        if shared.last_seen_epoch.load(Ordering::Acquire) != shared.epoch.load(Ordering::Acquire) {
            thread::sleep(Duration::from_millis(2));
            continue;
        }
        if pending_pos < pending.len() {
            pending_pos += shared.ring.write(&pending[pending_pos..]);
            if pending_pos < pending.len() { thread::sleep(Duration::from_millis(2)); }
            continue;
        }
''' + marker)
    a = s.index('                        // write to ring')
    b = s.index('\n                    }\n                    Err', a)
    s = s[:a] + '                        pending = out_samples;\n                        pending_pos = 0;' + s[b:]
    return s
edit('src/audio/engine.rs', engine)

def sequencer(s):
    # Notes are independent from drum steps, and use the resolved song pattern.
    a = s.index('                // Notes mélodiques')
    b = s.index('\n            }\n        }', a)
    notes = s[a:b].replace('project.notes(ci)', 'pattern.notes.get(ci).into_iter().flatten()')
    s = s[:a] + s[b:]
    a = s.index('                let Some(step) = pattern')
    s = s[:a] + notes + '\n' + s[a:]
    s = s.replace('tr.velocity = (note.velocity * channel.volume).clamp(0.0, 1.5);', 'tr.velocity = note.velocity;')
    s = s.replace('tr.velocity = (step.velocity * channel.volume).clamp(0.0, 1.5);', 'tr.velocity = step.velocity;\n                    tr.channel = ci;\n                    tr.synth = channel.synth;\n                    tr.tone = channel.tone;')
    s = s.replace('tr.pan = channel.pan;', 'tr.pan = 0.0;')
    s = s.replace('            tr.accent = beat_in_bar == 0;', '            tr.channel = MAX_CHANNELS;\n            tr.accent = beat_in_bar == 0;')
    s = s.replace('        let can_trigger = self\n            .stop_at_samples\n            .map(|s| self.position_samples < s)\n            .unwrap_or(true);', '        let any_solo = project.channels.iter().any(|c| c.solo);\n        let mut master_peak = 0.0f32;')
    s = s.replace('        for frame in out.chunks_exact_mut(2) {', '        for frame in out.chunks_exact_mut(2) {\n            let can_trigger = self.stop_at_samples.is_none_or(|s| self.position_samples < s);')
    a = s.index('                    let (vl, vr) = v.next();')
    b = s.index('\n                }\n            }\n            l *=', a)
    s = s[:a] + '''                    let (mut vl, mut vr) = v.next();
                    let c = v.channel();
                    if let Some(ch) = project.channels.get(c) {
                        if ch.mute || (any_solo && !ch.solo) { continue; }
                        vl *= ch.volume * (1.0 - ch.pan.max(0.0));
                        vr *= ch.volume * (1.0 + ch.pan.min(0.0));
                    }
                    l += vl;
                    r += vr;
                    if c < MAX_CHANNELS { channel_peaks[c] = channel_peaks[c].max(vl.abs().max(vr.abs())); }''' + s[b:]
    s = s.replace('            frame[1] = soft_clip(r);', '            frame[1] = soft_clip(r);\n            master_peak = master_peak.max(frame[0].abs()).max(frame[1].abs());')
    s = s.replace('self.channel_peaks.iter().copied().fold(0.0f32, f32::max)', 'master_peak')
    return s
edit('src/studio/sequencer.rs', sequencer)
edit('src/studio/project.rs', lambda s: s.replace('        if path.exists() {\n            fs::remove_file(path).map_err(|e| e.to_string())?;\n        }\n', ''))
