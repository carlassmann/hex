//! Labs › Remote transcription on the dictation worker: the connected Hex
//! hears a job's clip, and this Mac's model is the fallback.

use std::sync::Arc;
use std::time::Instant;

use super::TranscriptionTarget;
use crate::dictation::context::ContextSnapshot;
use crate::dictation::processor::{Processed, ProcessingObservation};
use crate::transcription::engine::{Offline, PendingTranscript, TranscriptionEngine};

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

impl JobTranscript {
    /// The job's text, with the connected Hex's processed text when it heard it.
    pub(super) fn wait(
        self,
        target: TranscriptionTarget,
        audio_ms: u64,
        context: &ContextSnapshot,
    ) -> (Result<String, String>, Option<Processed>) {
        match self {
            Self::Local(transcript) => (local_text(transcript, target, audio_ms), None),
            Self::Remote {
                connection,
                samples,
                engine,
                offline,
            } => match dictate_remotely(&connection, &samples, context) {
                Some((processed, raw)) => (Ok(raw.trim().to_string()), Some(processed)),
                None => (
                    engine
                        .foreground(samples, offline)
                        .map_err(|error| format!("{error:#}"))
                        .and_then(|transcript| local_text(transcript, target, audio_ms)),
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
                observation: Some(ProcessingObservation {
                    profile: format!("remote: {}", connection.machine_label),
                    latency_ms: started.elapsed().as_millis() as u64,
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
) -> Result<String, String> {
    transcript.wait().map(|transcribed| {
        tracing::info!(
            source = transcribed.source.label(),
            finish_ms = transcribed.finish_ms,
            audio_ms,
            "transcribed dictation"
        );
        let text = transcribed.text;
        let corrected = if matches!(target, TranscriptionTarget::Service) {
            text.clone()
        } else {
            text.trim().to_string()
        };
        tracing::debug!(
            raw_transcript = text,
            corrected_transcript = corrected,
            "prepared local transcript"
        );
        corrected
    })
}
