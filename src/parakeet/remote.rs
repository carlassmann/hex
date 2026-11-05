//! Labs › Remote transcription on the dictation worker: the connected Hex
//! hears a job's clip, and this Mac's model is the fallback.

use std::sync::Arc;
use std::time::Instant;

use super::TranscriptionTarget;
use crate::dictation::context::ContextSnapshot;
use crate::dictation::processor::{Processed, ProcessingObservation};
use crate::transcription::engine::{LiveStream, Offline, PendingTranscript, TranscriptionEngine};

/// Where a job's transcript comes from.
pub(super) enum JobTranscript {
    Local(PendingTranscript),
    /// The Hex this one is connected to (Labs › Remote transcription)
    /// transcribes the clip and runs its own Modes. This Mac's model loads
    /// and transcribes it only when that fails.
    Remote {
        connection: crate::remote::client::Connection,
        samples: Vec<f32>,
        engine: Arc<TranscriptionEngine>,
        offline: Offline,
    },
}

/// Whether the connected Hex transcribes, so this Mac loads its model only
/// for a fallback.
pub(super) fn defers_model_load() -> bool {
    let Some(connection) = crate::remote::client::active() else {
        return false;
    };
    tracing::info!(
        machine = connection.machine_label,
        "remote transcription is on; the speech model loads only for a fallback"
    );
    true
}

impl JobTranscript {
    /// A paste or print job goes to the connected Hex when there is one;
    /// everything else finishes on this Mac's engine.
    pub(super) fn start(
        target: TranscriptionTarget,
        engine: &Arc<TranscriptionEngine>,
        stream: Option<LiveStream>,
        samples: Vec<f32>,
        offline: Offline,
    ) -> Result<Self, &'static str> {
        let remote = matches!(
            target,
            TranscriptionTarget::Paste | TranscriptionTarget::Print
        )
        .then(crate::remote::client::active)
        .flatten();
        let Some(connection) = remote else {
            return engine
                .finish_with(stream, samples, offline)
                .map(Self::Local)
                .map_err(|_| "dictation worker is unavailable");
        };
        // Dropping a live stream cancels it; the clip goes as a whole.
        drop(stream);
        Ok(Self::Remote {
            connection,
            samples,
            engine: engine.clone(),
            offline,
        })
    }

    /// The job's text, with the connected Hex's processed text when it heard it.
    pub(super) fn wait(
        self,
        target: TranscriptionTarget,
        audio_ms: u64,
        context: &ContextSnapshot,
        stop_phrase: Option<&str>,
    ) -> (Result<String, String>, Option<Processed>) {
        match self {
            Self::Local(transcript) => {
                (local_text(transcript, target, audio_ms, stop_phrase), None)
            }
            Self::Remote {
                connection,
                samples,
                engine,
                offline,
            } => match dictate_remotely(&connection, &samples, context) {
                Some((mut processed, raw)) => {
                    // Labs › Stop phrase: the other Hex doesn't know the words that
                    // finished this capture.
                    let strip =
                        |text| crate::dictation::stop_phrase::strip_finished(text, stop_phrase);
                    processed.text = strip(processed.text).trim().to_string();
                    (Ok(strip(raw).trim().to_string()), Some(processed))
                }
                None => (
                    engine
                        .foreground(samples, offline)
                        .map_err(|error| format!("{error:#}"))
                        .and_then(|transcript| {
                            local_text(transcript, target, audio_ms, stop_phrase)
                        }),
                    None,
                ),
            },
        }
    }
}

fn dictate_remotely(
    connection: &crate::remote::client::Connection,
    samples: &[f32],
    context: &ContextSnapshot,
) -> Option<(Processed, String)> {
    let started = Instant::now();
    match crate::remote::client::dictate(connection, samples, context.application.as_deref()) {
        Ok(dictated) => Some((
            Processed {
                text: dictated.text,
                nothing: false,
                observation: Some(ProcessingObservation {
                    profile: format!("remote: {}", connection.machine_label),
                    latency_ms: started.elapsed().as_millis() as u64,
                    model: None,
                    fallback: None,
                }),
                transformations: Vec::new(),
            },
            dictated.raw,
        )),
        Err(error) => {
            tracing::warn!(
                machine = connection.machine_label,
                error = %format!("{error:#}"),
                "remote transcription failed; using this Mac's transcript"
            );
            None
        }
    }
}

/// This Mac's transcript of a job, trimmed unless it serves the local API.
fn local_text(
    transcript: PendingTranscript,
    target: TranscriptionTarget,
    audio_ms: u64,
    stop_phrase: Option<&str>,
) -> Result<String, String> {
    transcript.wait().map(|transcribed| {
        tracing::info!(
            source = transcribed.source.label(),
            finish_ms = transcribed.finish_ms,
            audio_ms,
            "transcribed dictation"
        );
        let raw = transcribed.text;
        // Labs › Stop phrase: the words that finished this capture never
        // reach the text, and a capture that was only the phrase reads as
        // empty.
        use crate::dictation::modes::output::{PreparedTranscript, prepare_transcript};
        let corrected = match prepare_transcript(raw.clone(), stop_phrase, target) {
            PreparedTranscript::Ready(text) => text,
            PreparedTranscript::StopPhraseOnly | PreparedTranscript::Empty => String::new(),
        };
        tracing::debug!(
            raw_transcript = raw,
            corrected_transcript = corrected,
            "prepared local transcript"
        );
        corrected
    })
}
