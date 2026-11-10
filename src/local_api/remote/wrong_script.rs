//! Parakeet v3 sometimes writes a short English phrase in Cyrillic ("Ес,
//! плиз." for "Yes, please."), about one dictation in a hundred, and its
//! language setting can't stop it. When the transcript has Cyrillic but the
//! set language doesn't use it, the recording is heard again by Cohere
//! Transcribe, which hears those clips right, when it is installed.

use std::io::Cursor;

use crate::transcription::models::{self, TranscriptionModelId, TranscriptionSelection};
use crate::transcription::service::AudioClip;

const FALLBACK_MODEL: TranscriptionModelId = TranscriptionModelId::CohereTranscribe;
const CYRILLIC_LANGUAGES: &[&str] = &["ru", "uk", "bg", "be", "sr", "mk", "kk", "auto"];

/// The model and language to hear `raw`'s recording again with, when `raw`
/// is in a script `selection`'s language never writes.
pub(super) fn retry_selection(
    raw: &str,
    selection: &TranscriptionSelection,
) -> Option<TranscriptionSelection> {
    let language = selection.language.as_str();
    let expected_latin = !language.is_empty() && !CYRILLIC_LANGUAGES.contains(&language);
    let fallback = models::definition(FALLBACK_MODEL);
    (expected_latin
        && has_cyrillic(raw)
        && selection.model != FALLBACK_MODEL
        && models::is_verified(fallback))
    .then(|| TranscriptionSelection {
        model: FALLBACK_MODEL,
        language: language.to_owned(),
    })
}

/// The retry's text replaces `raw` only when it is in the right script.
pub(super) fn accept(retried: &str) -> Option<String> {
    let retried = retried.trim();
    (!retried.is_empty() && !has_cyrillic(retried)).then(|| retried.to_owned())
}

fn has_cyrillic(text: &str) -> bool {
    text.chars()
        .any(|character| ('\u{0400}'..='\u{052F}').contains(&character))
}

/// 16 kHz mono samples as a clip for the transcription service.
pub(super) fn clip(samples: &[f32]) -> Option<AudioClip> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16_000,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut wav = Cursor::new(Vec::new());
    let mut writer = hound::WavWriter::new(&mut wav, spec).ok()?;
    for &sample in samples {
        writer.write_sample(sample).ok()?;
    }
    writer.finalize().ok()?;
    wav.set_position(0);
    AudioClip::decode_wav(wav).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn selection(model: TranscriptionModelId, language: &str) -> TranscriptionSelection {
        TranscriptionSelection {
            model,
            language: language.into(),
        }
    }

    #[test]
    fn cyrillic_is_wrong_only_for_a_latin_language() {
        assert!(has_cyrillic("Ес, плиз."));
        assert!(!has_cyrillic("Yes, please. Grüße, ça va"));
        let ru = selection(TranscriptionModelId::ParakeetV3, "ru");
        assert!(retry_selection("Ес, плиз.", &ru).is_none());
        let latin = selection(TranscriptionModelId::ParakeetV3, "de");
        assert!(retry_selection("Yes, please.", &latin).is_none());
    }

    #[test]
    fn a_retry_still_in_cyrillic_is_not_taken() {
        assert_eq!(accept(" Yes, please. "), Some("Yes, please.".into()));
        assert_eq!(accept("Эй"), None);
        assert_eq!(accept("  "), None);
    }

    #[test]
    fn samples_survive_the_clip() {
        let samples = vec![0.25_f32; 16_000];
        let clip = clip(&samples).expect("clip");
        assert_eq!(clip.duration_ms, 1000);
        assert_eq!(clip.samples().len(), samples.len());
    }
}
