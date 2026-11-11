//! Parakeet v3 sometimes writes a short English phrase in Cyrillic ("Ес,
//! плиз." for "Yes, please."), about one dictation in a hundred, and its
//! language setting can't stop it. It also returns nothing for some short
//! replies it clearly heard ("Vielen Dank"). Those recordings are heard
//! again by Cohere Transcribe, which hears them right, when it is installed.
//! A quiet recording is never retried: Cohere makes up text for silence.

use std::io::Cursor;

use crate::transcription::models::{self, TranscriptionModelId, TranscriptionSelection};
use crate::transcription::service::AudioClip;

const FALLBACK_MODEL: TranscriptionModelId = TranscriptionModelId::CohereTranscribe;
const CYRILLIC_LANGUAGES: &[&str] = &["ru", "uk", "bg", "be", "sr", "mk", "kk", "auto"];

/// Root mean square (of samples in -1..1) above which an empty transcript
/// missed speech. The journal's empty transcripts of speech measured 0.031
/// and 0.056; silence 0.0005.
const SPEECH_RMS: f32 = 0.015;

/// Why a recording is heard again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Reason {
    WrongScript,
    MissedSpeech,
}

impl Reason {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::WrongScript => "wrong-script",
            Self::MissedSpeech => "missed-speech",
        }
    }
}

/// Why `raw`, heard from `samples` with `selection`, should be heard again.
pub(super) fn reason(
    raw: &str,
    samples: &[f32],
    selection: &TranscriptionSelection,
) -> Option<Reason> {
    if raw.trim().is_empty() {
        (rms(samples) > SPEECH_RMS).then_some(Reason::MissedSpeech)
    } else {
        (expects_latin(&selection.language) && has_cyrillic(raw)).then_some(Reason::WrongScript)
    }
}

/// The model and language to hear a recording again with, while Cohere is
/// installed and isn't the model that already heard it.
pub(super) fn retry_selection(
    selection: &TranscriptionSelection,
) -> Option<TranscriptionSelection> {
    let fallback = models::definition(FALLBACK_MODEL);
    (selection.model != FALLBACK_MODEL && models::is_verified(fallback)).then(|| {
        TranscriptionSelection {
            model: FALLBACK_MODEL,
            language: selection.language.clone(),
        }
    })
}

/// The retry's text replaces the transcript only when it has words in a
/// script the language writes.
pub(super) fn accept(retried: &str, language: &str) -> Option<String> {
    let retried = retried.trim();
    (!retried.is_empty() && !(expects_latin(language) && has_cyrillic(retried)))
        .then(|| retried.to_owned())
}

fn expects_latin(language: &str) -> bool {
    !language.is_empty() && !CYRILLIC_LANGUAGES.contains(&language)
}

fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let energy: f32 = samples.iter().map(|sample| sample * sample).sum();
    (energy / samples.len() as f32).sqrt()
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
        let speech = vec![0.05_f32; 1600];
        let ru = selection(TranscriptionModelId::ParakeetV3, "ru");
        assert_eq!(reason("Ес, плиз.", &speech, &ru), None);
        let latin = selection(TranscriptionModelId::ParakeetV3, "de");
        assert_eq!(reason("Yes, please.", &speech, &latin), None);
        assert_eq!(
            reason("Ес, плиз.", &speech, &latin),
            Some(Reason::WrongScript)
        );
    }

    #[test]
    fn only_an_empty_transcript_of_a_loud_recording_missed_speech() {
        let de = selection(TranscriptionModelId::ParakeetV3, "de");
        let speech = vec![0.031_f32; 1600];
        let silence = vec![0.0005_f32; 1600];
        assert_eq!(reason(" ", &speech, &de), Some(Reason::MissedSpeech));
        assert_eq!(reason("", &silence, &de), None);
        assert_eq!(reason("", &[], &de), None);
    }

    #[test]
    fn cohere_never_retries_itself() {
        let cohere = selection(TranscriptionModelId::CohereTranscribe, "de");
        assert_eq!(retry_selection(&cohere), None);
    }

    #[test]
    fn a_retry_still_in_cyrillic_is_not_taken() {
        assert_eq!(accept(" Yes, please. ", "de"), Some("Yes, please.".into()));
        assert_eq!(accept("Эй", "de"), None);
        assert_eq!(accept("Эй", "ru"), Some("Эй".into()));
        assert_eq!(accept("  ", "de"), None);
    }

    #[test]
    fn samples_survive_the_clip() {
        let samples = vec![0.25_f32; 16_000];
        let clip = clip(&samples).expect("clip");
        assert_eq!(clip.duration_ms, 1000);
        assert_eq!(clip.samples().len(), samples.len());
    }
}
