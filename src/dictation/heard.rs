//! A dictation's clip as its personal transformations see it
//! (`context.transcript`, Labs › Transcribe again in transforms): the model
//! that heard it, how long it ran, whether Hex's speech detector heard
//! speech in it, and a way to hear it again with another installed model.
//! The samples stay in memory with the dictation's job and are never written.

use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use serde::Serialize;

use crate::transcription::models::{
    self, AUTO_LANGUAGE, TranscriptionModelId, TranscriptionSelection,
};

const SAMPLE_RATE: f64 = 16_000.0;
/// How long one run may take, model load included; the transformation's own
/// timeout usually stops it first.
const DEADLINE: Duration = Duration::from_secs(60);
/// Pause before asking again whether a dictation still holds the model.
const BUSY_RETRY: Duration = Duration::from_millis(50);

/// How a heard clip reaches a speech model: the app's transcription service,
/// or a fake in tests.
pub(crate) trait Retranscriber: Send + Sync {
    /// Whether `selection` can run without a download.
    fn is_ready(&self, selection: &TranscriptionSelection) -> bool;
    /// Whether a dictation is capturing or transcribing, which goes first.
    fn dictation_busy(&self) -> bool;
    fn transcribe(
        &self,
        selection: TranscriptionSelection,
        samples: Vec<f32>,
        stop: &AtomicBool,
    ) -> Result<String, String>;
}

/// The transcription service: the dictation engine's model when it is the one
/// asked for, else a copy of the service's own for this run.
struct Service;

impl Retranscriber for Service {
    fn is_ready(&self, selection: &TranscriptionSelection) -> bool {
        crate::transcription::service::is_ready(selection)
    }

    fn dictation_busy(&self) -> bool {
        crate::transcription::engine::dictation_busy()
    }

    fn transcribe(
        &self,
        selection: TranscriptionSelection,
        samples: Vec<f32>,
        stop: &AtomicBool,
    ) -> Result<String, String> {
        let clip = crate::transcription::service::AudioClip::from_samples(samples);
        crate::transcription::service::transcribe_once(selection, clip, DEADLINE, stop)
    }
}

/// One dictation's clip, shared by its job and the transformation calls that
/// may hear it again.
#[derive(Clone)]
pub(crate) struct HeardClip(Arc<Clip>);

struct Clip {
    selection: TranscriptionSelection,
    samples: Vec<f32>,
    heard_speech: OnceLock<bool>,
    retranscriber: Arc<dyn Retranscriber>,
}

/// `context.transcript` as the transformation's JSON context carries it;
/// `transcribeAgain` is attached in JavaScript.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TranscriptContext {
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// Seconds.
    pub duration: f64,
    pub heard_speech: bool,
}

/// Whether a dictation's kept clip held speech, so its empty transcript still
/// reaches the transformations.
pub(crate) fn held_speech(clip: Option<&HeardClip>) -> bool {
    clip.is_some_and(HeardClip::heard_speech)
}

/// What `transcribeAgain` asked for.
#[derive(Debug, serde::Deserialize)]
pub(crate) struct Request {
    pub model: String,
    #[serde(default)]
    pub language: Option<String>,
}

impl HeardClip {
    /// The clip `selection` heard, kept only while Labs › Transcribe again in
    /// transforms is on and a mode selects a transformation; otherwise
    /// nothing is copied.
    pub(crate) fn keep(selection: &TranscriptionSelection, samples: &[f32]) -> Option<Self> {
        (crate::labs::transcribe_again()
            && crate::app_settings::custom_transformations_selected()
            && !samples.is_empty())
        .then(|| Self::new(selection.clone(), samples.to_vec(), Arc::new(Service)))
    }

    pub(crate) fn new(
        selection: TranscriptionSelection,
        samples: Vec<f32>,
        retranscriber: Arc<dyn Retranscriber>,
    ) -> Self {
        Self(Arc::new(Clip {
            selection,
            samples,
            heard_speech: OnceLock::new(),
            retranscriber,
        }))
    }

    /// Whether Hex's speech detector heard speech; `false` when it isn't
    /// installed. Asked once per clip.
    pub(crate) fn heard_speech(&self) -> bool {
        *self.0.heard_speech.get_or_init(|| {
            crate::transcription::speech_detector::contains_speech(&self.0.samples) == Some(true)
        })
    }

    pub(crate) fn context(&self) -> TranscriptContext {
        let language = &self.0.selection.language;
        TranscriptContext {
            model: self.0.selection.model.as_str().to_string(),
            language: (!language.is_empty() && language != AUTO_LANGUAGE).then(|| language.clone()),
            duration: self.0.samples.len() as f64 / SAMPLE_RATE,
            heard_speech: self.heard_speech(),
        }
    }

    /// The selection `request` names: its language, else the dictation's when
    /// the model has it, else the model's default. Refused unless the model
    /// can run now, so nothing downloads.
    pub(crate) fn selection_for(
        &self,
        request: &Request,
    ) -> Result<TranscriptionSelection, String> {
        let model = TranscriptionModelId::from_str(&request.model)
            .map_err(|_| format!("Hex has no speech model \"{}\"", request.model))?;
        let definition = models::definition(model);
        let language = request.language.clone().unwrap_or_else(|| {
            definition
                .language_for(&self.0.selection.language)
                .to_string()
        });
        let selection = TranscriptionSelection { model, language };
        models::validate(&selection).map_err(|error| format!("{error}"))?;
        if !self.0.retranscriber.is_ready(&selection) {
            return Err(format!(
                "{} isn't installed; download it in Hex's model picker first",
                definition.name
            ));
        }
        Ok(selection)
    }

    /// Transcribes the clip again with `selection` once no dictation is
    /// capturing or transcribing. Blocks until the text or `stop`.
    pub(crate) fn transcribe_again(
        &self,
        selection: TranscriptionSelection,
        stop: &AtomicBool,
    ) -> Result<String, String> {
        while self.0.retranscriber.dictation_busy() {
            if stop.load(Ordering::Acquire) {
                return Err("Transcription was cancelled.".into());
            }
            std::thread::sleep(BUSY_RETRY);
        }
        self.0
            .retranscriber
            .transcribe(selection, self.0.samples.clone(), stop)
    }
}

impl std::fmt::Debug for HeardClip {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("HeardClip")
            .field("selection", &self.0.selection)
            .field("samples", &self.0.samples.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::time::Instant;

    use super::*;

    /// Hears every clip as `text`, with `ready` models installed, and records
    /// whether a dictation was busy when each run started.
    struct Fake {
        ready: Vec<TranscriptionModelId>,
        busy: AtomicBool,
        runs: Mutex<Vec<(TranscriptionSelection, bool)>>,
    }

    impl Fake {
        fn with(ready: &[TranscriptionModelId]) -> Arc<Self> {
            Arc::new(Self {
                ready: ready.to_vec(),
                busy: AtomicBool::new(false),
                runs: Mutex::new(Vec::new()),
            })
        }
    }

    impl Retranscriber for Fake {
        fn is_ready(&self, selection: &TranscriptionSelection) -> bool {
            self.ready.contains(&selection.model)
        }

        fn dictation_busy(&self) -> bool {
            self.busy.load(Ordering::Acquire)
        }

        fn transcribe(
            &self,
            selection: TranscriptionSelection,
            samples: Vec<f32>,
            _stop: &AtomicBool,
        ) -> Result<String, String> {
            let busy = self.dictation_busy();
            self.runs.lock().unwrap().push((selection, busy));
            Ok(format!("{} samples", samples.len()))
        }
    }

    fn selection(model: TranscriptionModelId, language: &str) -> TranscriptionSelection {
        TranscriptionSelection {
            model,
            language: language.into(),
        }
    }

    fn clip(fake: &Arc<Fake>, language: &str) -> HeardClip {
        HeardClip::new(
            selection(TranscriptionModelId::ParakeetV3, language),
            vec![0.1; 24_000],
            fake.clone(),
        )
    }

    fn request(model: &str, language: Option<&str>) -> Request {
        Request {
            model: model.into(),
            language: language.map(Into::into),
        }
    }

    #[test]
    fn the_context_names_the_model_language_length_and_speech() {
        let _speech = crate::transcription::speech_detector::override_for_test(Some(true));
        let fake = Fake::with(&[]);
        assert_eq!(
            clip(&fake, "de").context(),
            TranscriptContext {
                model: "parakeet_v3".into(),
                language: Some("de".into()),
                duration: 1.5,
                heard_speech: true,
            }
        );
        let json = serde_json::to_value(clip(&fake, AUTO_LANGUAGE).context()).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"model": "parakeet_v3", "duration": 1.5, "heardSpeech": true})
        );
    }

    #[test]
    fn heard_speech_is_false_without_a_detector_verdict() {
        let _speech = crate::transcription::speech_detector::override_for_test(None);
        assert!(!clip(&Fake::with(&[]), "en").heard_speech());
    }

    #[test]
    fn an_uninstalled_or_unknown_model_is_refused_before_anything_runs() {
        let fake = Fake::with(&[TranscriptionModelId::ParakeetV3]);
        let heard = clip(&fake, "en");
        let missing = heard
            .selection_for(&request("cohere_transcribe", None))
            .unwrap_err();
        assert!(missing.contains("isn't installed"), "{missing}");
        let unknown = heard
            .selection_for(&request("no_such_model", None))
            .unwrap_err();
        assert!(unknown.contains("no_such_model"), "{unknown}");
        assert!(fake.runs.lock().unwrap().is_empty());
    }

    #[test]
    fn the_language_defaults_to_the_dictations_when_the_model_has_it() {
        let fake = Fake::with(&[TranscriptionModelId::CohereTranscribe]);
        let cohere = |language| {
            clip(&fake, language)
                .selection_for(&request("cohere_transcribe", None))
                .unwrap()
                .language
        };
        assert_eq!(cohere("de"), "de");
        // Cohere has no language detection, so Auto falls back to its default.
        assert_eq!(cohere(AUTO_LANGUAGE), "en");
        let asked = clip(&fake, "de")
            .selection_for(&request("cohere_transcribe", Some("fr")))
            .unwrap();
        assert_eq!(asked.language, "fr");
        let refused = clip(&fake, "de")
            .selection_for(&request("cohere_transcribe", Some("ru")))
            .unwrap_err();
        assert!(refused.contains("does not support Russian"), "{refused}");
    }

    #[test]
    fn a_dictation_arriving_first_goes_first() {
        let fake = Fake::with(&[TranscriptionModelId::CohereTranscribe]);
        fake.busy.store(true, Ordering::Release);
        let heard = clip(&fake, "en");
        let releasing = fake.clone();
        let release = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(150));
            releasing.busy.store(false, Ordering::Release);
        });
        let started = Instant::now();
        let text = heard
            .transcribe_again(
                selection(TranscriptionModelId::CohereTranscribe, "en"),
                &AtomicBool::new(false),
            )
            .unwrap();
        release.join().unwrap();
        assert_eq!(text, "24000 samples");
        assert!(started.elapsed() >= Duration::from_millis(150));
        assert_eq!(
            *fake.runs.lock().unwrap(),
            vec![(
                selection(TranscriptionModelId::CohereTranscribe, "en"),
                false
            )]
        );
    }

    #[test]
    fn a_stopped_call_gives_up_waiting_for_dictation() {
        let fake = Fake::with(&[TranscriptionModelId::CohereTranscribe]);
        fake.busy.store(true, Ordering::Release);
        let stopped = AtomicBool::new(true);
        let answer = clip(&fake, "en").transcribe_again(
            selection(TranscriptionModelId::CohereTranscribe, "en"),
            &stopped,
        );
        assert!(answer.is_err());
        assert!(fake.runs.lock().unwrap().is_empty());
    }

    #[test]
    fn nothing_is_kept_while_the_lab_is_off_or_no_transformation_is_selected() {
        let samples = vec![0.1; 16_000];
        let dictation = selection(TranscriptionModelId::ParakeetV3, "en");
        crate::labs::apply(&crate::app_settings::LabsSettings::default());
        assert!(!crate::labs::transcribe_again());
        assert!(HeardClip::keep(&dictation, &samples).is_none());
        crate::labs::apply(&crate::app_settings::LabsSettings {
            transcribe_again: true,
            ..Default::default()
        });
        assert!(crate::labs::transcribe_again());
        assert!(!crate::app_settings::custom_transformations_selected());
        assert!(HeardClip::keep(&dictation, &samples).is_none());
        crate::labs::apply(&crate::app_settings::LabsSettings::default());
    }
}
