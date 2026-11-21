//! `context.transcript` and `transcribeAgain` through real scripts, with a
//! fake speech model in place of the transcription service.

use super::*;
use crate::dictation::heard::Retranscriber;
use crate::transcription::models::{TranscriptionModelId, TranscriptionSelection};

const CONFIG: &str = r#"export default {
  transformations: {
    transcript: { name: "Transcript", transform: (_, context) => JSON.stringify(context.transcript ?? null) },
    again: {
      name: "Again",
      transform: async (text, context) =>
        text || (await context.transcript.transcribeAgain({ model: "cohere_transcribe" })),
    },
    french: {
      name: "French",
      transform: (_, context) =>
        context.transcript.transcribeAgain({ model: "cohere_transcribe", language: "fr" }),
    },
    missing: {
      name: "Missing",
      transform: async (_, context) => {
        try {
          return await context.transcript.transcribeAgain({ model: "whisper_large_v3_turbo" })
        } catch (error) {
          return error.message
        }
      },
    },
    many: {
      name: "Many",
      transform: async (_, context) => {
        const heard = []
        for (let run = 0; run < 4; run++) {
          try {
            heard.push(await context.transcript.transcribeAgain({ model: "cohere_transcribe" }))
          } catch (error) {
            heard.push(error.message)
          }
        }
        return heard.join(" | ")
      },
    },
  },
}
"#;

/// Hears every clip as `<model>:<language>`, with only Cohere installed.
struct Fake;

impl Retranscriber for Fake {
    fn is_ready(&self, selection: &TranscriptionSelection) -> bool {
        selection.model == TranscriptionModelId::CohereTranscribe
    }

    fn dictation_busy(&self) -> bool {
        false
    }

    fn transcribe(
        &self,
        selection: TranscriptionSelection,
        _samples: Vec<f32>,
        _stop: &AtomicBool,
    ) -> std::result::Result<String, String> {
        Ok(format!(
            "{}:{}",
            selection.model.as_str(),
            selection.language
        ))
    }
}

/// A German Parakeet dictation of 1.5 s that the speech detector heard.
fn heard() -> HeardClip {
    let _speech = crate::transcription::speech_detector::override_for_test(Some(true));
    let heard = HeardClip::new(
        TranscriptionSelection {
            model: TranscriptionModelId::ParakeetV3,
            language: "de".into(),
        },
        vec![0.1; 24_000],
        Arc::new(Fake),
    );
    // Asked once per clip, so the worker thread reads this verdict.
    assert!(heard.heard_speech());
    heard
}

fn dictation(harness: &Harness, id: &str, text: &str) -> std::result::Result<String, String> {
    harness.transform_heard(
        crate::dictation::processor::Source::Dictation,
        &[id],
        text,
        Some(heard()),
    )
}

#[test]
fn a_dictations_context_carries_its_transcript() {
    let harness = Harness::new(CONFIG);
    let transcript: serde_json::Value =
        serde_json::from_str(&dictation(&harness, "transcript", "hallo").unwrap()).unwrap();
    assert_eq!(
        transcript,
        serde_json::json!({
            "model": "parakeet_v3",
            "language": "de",
            "duration": 1.5,
            "heardSpeech": true,
        })
    );
    // Typed text, and a dictation while the lab is off, have none.
    assert_eq!(harness.transform(&["transcript"], "hallo").unwrap(), "null");
}

#[test]
fn an_empty_transcript_is_heard_again_with_another_model() {
    let harness = Harness::new(CONFIG);
    assert_eq!(
        dictation(&harness, "again", "").unwrap(),
        "cohere_transcribe:de"
    );
    assert_eq!(dictation(&harness, "again", "gehört").unwrap(), "gehört");
    assert_eq!(
        dictation(&harness, "french", "").unwrap(),
        "cohere_transcribe:fr"
    );
}

#[test]
fn an_uninstalled_model_rejects_clearly() {
    let harness = Harness::new(CONFIG);
    let message = dictation(&harness, "missing", "").unwrap();
    assert!(message.contains("isn't installed"), "{message}");
}

#[test]
fn a_call_hears_its_dictation_again_a_few_times_at_most() {
    let harness = Harness::new(CONFIG);
    let heard = dictation(&harness, "many", "").unwrap();
    let runs = heard.split(" | ").collect::<Vec<_>>();
    assert_eq!(runs[..3], ["cohere_transcribe:de"; 3]);
    assert!(runs[3].contains("at most 3 times"), "{heard}");
    // The bound is per call.
    assert!(dictation(&harness, "again", "").is_ok());
}

#[test]
fn without_a_clip_transcribe_again_is_not_there() {
    let harness = Harness::new(CONFIG);
    let refused = harness.transform(&["again"], "").unwrap_err();
    assert!(refused.contains("transcribeAgain"), "{refused}");
}

#[test]
fn a_config_that_hears_again_gets_the_longer_default_timeout() {
    let harness = Harness::new(CONFIG);
    let limits = &harness.worker.loaded.as_ref().unwrap().limits;
    assert_eq!(limits["again"], IO_TIMEOUT);
    let pure = Harness::new(
        r#"export default { transformations: { upper: { name: "Upper", transform: (t) => t.toUpperCase() } } }"#,
    );
    assert_eq!(
        pure.worker.loaded.as_ref().unwrap().limits["upper"],
        PURE_TIMEOUT
    );
}

/// The shared Modes stage, as every listener runs it: an empty transcript
/// that held speech still reaches the mode's transformations.
#[test]
fn modes_run_transformations_on_an_empty_transcript_with_speech() {
    let root = unique_temp_dir("hex-transcribe-again");
    let workspace = root.join("hex");
    fs::create_dir_all(&workspace).unwrap();
    fs::write(workspace.join("hex.config.ts"), CONFIG).unwrap();
    let client = Arc::new(TransformationClient::default());
    let (requests, receiver) = mpsc::sync_channel(4);
    let stop = Arc::new(AtomicBool::new(false));
    let running = {
        let (client, requests, stop) = (client.clone(), requests.clone(), stop.clone());
        thread::Builder::new()
            .stack_size(script::THREAD_STACK_SIZE)
            .spawn(move || {
                Worker {
                    transformations: client,
                    requests,
                    workspace,
                    status_file: None,
                    trust_file: None,
                    generation: 0,
                    loaded: None,
                    status: StatusSnapshot::default(),
                }
                .run(&receiver, &stop);
            })
            .unwrap()
    };
    let ready = Instant::now() + Duration::from_secs(5);
    while client
        .active
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .is_none()
    {
        assert!(Instant::now() < ready, "the config never loaded");
        thread::sleep(Duration::from_millis(10));
    }
    let mut settings = crate::app_settings::AppSettings::default();
    settings.dictation_processing.default_mode.transformations = vec!["again".into()];
    let modes = crate::dictation::modes::ModeRuntime::from_settings(&settings);
    let processed = modes.process(
        "",
        "de",
        &ContextSnapshot::default(),
        &client,
        crate::dictation::processor::Source::Dictation,
        Some(&heard()),
    );
    assert_eq!(processed.text, "cohere_transcribe:de");
    // Without a clip, the empty transcript stays empty and pastes nothing.
    let unheard = modes.process(
        "",
        "de",
        &ContextSnapshot::default(),
        &client,
        crate::dictation::processor::Source::Dictation,
        None,
    );
    assert_eq!(unheard.text, "");
    stop.store(true, Ordering::Release);
    let _ = requests.try_send(Request::Reload);
    running.join().unwrap();
    fs::remove_dir_all(root).unwrap();
}

/// `docs/examples/retry-transcripts.ts` loads and retries only what it means to.
#[test]
fn the_retry_example_retries_empty_and_wrong_script_dictations() {
    let harness = Harness::new(include_str!("../../../docs/examples/retry-transcripts.ts"));
    let retry = |text| dictation(&harness, "retry-transcripts", text).unwrap();
    assert_eq!(retry(""), "cohere_transcribe:de");
    assert_eq!(retry("привет мир"), "cohere_transcribe:de");
    assert_eq!(retry("Hallo Welt"), "Hallo Welt");
    assert_eq!(harness.transform(&["retry-transcripts"], "").unwrap(), "");
}
