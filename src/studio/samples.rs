//! Décodage hors temps réel des échantillons utilisateur (canal « Sample »).
//!
//! Le décodage est volontairement effectué hors du callback audio. Le résultat
//! est un buffer mono que le séquenceur rééchantillonne à la volée.

use std::collections::HashMap;
use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use symphonia::core::codecs::CodecParameters;
use symphonia::core::codecs::audio::{AudioDecoder, AudioDecoderOptions};
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::default::{get_codecs, get_probe};

/// Échantillon chargé, partagé avec le thread de rendu.
#[derive(Clone)]
pub struct LoadedSample {
    pub samples: Arc<Vec<f32>>,
    pub sample_rate: u32,
}

pub type SampleBank = HashMap<String, LoadedSample>;

pub struct DecodedSample {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

/// Décode un fichier audio en mono (moyenne des canaux).
pub fn decode_to_mono(path: &Path) -> Result<DecodedSample, String> {
    let file = File::open(path).map_err(|e| format!("Ouverture impossible : {e}"))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let hint = Hint::new();
    let mut format: Box<dyn FormatReader> = get_probe()
        .probe(
            &hint,
            mss,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(|e| format!("Format non reconnu : {e}"))?;

    let track = format
        .default_track(TrackType::Audio)
        .ok_or("Aucune piste audio dans le fichier")?;
    let track_id = track.id;
    let cp = match track.codec_params.clone() {
        Some(CodecParameters::Audio(a)) => a,
        _ => return Err("Paramètres codec indisponibles".to_string()),
    };
    let sample_rate = cp.sample_rate.unwrap_or(44100);
    let registered = get_codecs()
        .get_audio_decoder(cp.codec)
        .ok_or("Codec non supporté")?;
    let mut decoder: Box<dyn AudioDecoder> =
        (registered.factory)(&cp, &AudioDecoderOptions::default())
            .map_err(|e| format!("Décodeur indisponible : {e}"))?;

    let mut mono: Vec<f32> = Vec::new();
    loop {
        let packet = match format.next_packet() {
            Ok(Some(p)) => p,
            Ok(None) => break,
            Err(e) => return Err(format!("Lecture du flux : {e}")),
        };
        if packet.track_id != track_id {
            continue;
        }
        let decoded = match decoder.decode_ref(&packet.as_packet_ref()) {
            Ok(d) => d,
            Err(_) => continue,
        };
        let spec = decoded.spec();
        let ch = spec.channels().count().max(1);
        let frames = decoded.frames();
        let mut buf = vec![0.0f32; frames * ch];
        decoded.copy_to_slice_interleaved::<f32, _>(&mut buf);
        if ch == 1 {
            mono.extend_from_slice(&buf);
        } else {
            for frame in buf.chunks(ch) {
                let sum: f32 = frame.iter().copied().sum();
                mono.push(sum / ch as f32);
            }
        }
    }

    if mono.is_empty() {
        return Err("Échantillon vide".to_string());
    }
    Ok(DecodedSample {
        samples: mono,
        sample_rate,
    })
}
