//! `POST /remote/transcriptions`: a paired device's WAV in, this machine's
//! raw transcript with segment and word times out (`Transcript` as JSON),
//! with no Modes. A Mac with Labs › Remote transcription sends its meeting
//! chunks and Annotate words here, so it never loads a model for them. Each
//! clip is one foreground request on this machine's dictation engine, after
//! the dictation already sent and never inside a live stream, so it is
//! capped at [`MAX_CLIP_MS`] to keep dictation here from waiting long.

use std::net::TcpStream;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use color_eyre::eyre::{Result, eyre};

use super::super::{
    HttpContext, HttpRequest, HttpResponse, RequestAction, read_audio, write_response,
};
use super::{Action, paired_device};
use crate::transcription::engine::{Offline, TranscriptionEngine};
use crate::transcription::transcript::Transcript;

/// Meeting chunks are 12 s at most and Annotate sends 30 s; leave room.
const MAX_CLIP_MS: u64 = 60_000;
/// Longest a paired device's clip yields to this machine's own dictation.
const LOCAL_DICTATION_WAIT: Duration = Duration::from_secs(20);
const LOCAL_DICTATION_POLL: Duration = Duration::from_millis(50);
const UPLOAD_DEADLINE: std::time::Duration = std::time::Duration::from_secs(60);

pub(super) fn action(request: HttpRequest) -> RequestAction {
    if paired_device(&request).is_none() {
        return RequestAction::Respond(HttpResponse::code(401, "not-paired"));
    }
    if !request.has_content_type("audio/wav") {
        return RequestAction::Respond(HttpResponse::code(415, "unsupported-audio"));
    }
    let Some(content_length) = request.content_length else {
        return RequestAction::Respond(HttpResponse::code(411, "length-required"));
    };
    RequestAction::Remote(Action::Timed {
        content_length,
        body_prefix: request.body_prefix,
    })
}

pub(super) fn transcribe(
    stream: &mut TcpStream,
    content_length: usize,
    body: Vec<u8>,
    context: &HttpContext,
) {
    let clip = match read_audio(
        stream,
        content_length,
        body,
        &context.shutdown,
        UPLOAD_DEADLINE,
    ) {
        Ok(clip) => clip,
        Err(Some(response)) => {
            let _ = write_response(stream, response);
            return;
        }
        Err(None) => return,
    };
    let response = if clip.duration_ms > MAX_CLIP_MS {
        HttpResponse::code(413, "clip-too-long")
    } else if let Some(engine) = TranscriptionEngine::shared() {
        let started = std::time::Instant::now();
        match timed(&engine, clip.samples().to_vec()) {
            Ok(transcript) => {
                tracing::info!(
                    duration_ms = clip.duration_ms,
                    latency_ms = started.elapsed().as_millis() as u64,
                    "remote timed transcription"
                );
                HttpResponse::json(200, &transcript)
            }
            Err(error) => {
                tracing::error!(error = %format!("{error:#}"), "remote timed transcription failed");
                HttpResponse::code(500, "transcription-failed")
            }
        }
    } else {
        HttpResponse::code(503, "engine-unavailable")
    };
    let _ = write_response(stream, response);
}

pub(super) fn timed(engine: &TranscriptionEngine, samples: Vec<f32>) -> Result<Transcript> {
    timed_with_phases(engine, samples).map(|(transcript, _)| transcript)
}

/// Where a clip's time on the engine went: waiting for earlier work, then
/// the model itself.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Phases {
    pub queued_ms: u64,
    pub inference_ms: u64,
}

pub(super) fn timed_with_phases(
    engine: &TranscriptionEngine,
    samples: Vec<f32>,
) -> Result<(Transcript, Phases)> {
    if samples.is_empty() {
        return Ok((Transcript::default(), Phases::default()));
    }
    let submitted = Instant::now();
    wait_for_local_dictation();
    let (send, transcript) = mpsc::sync_channel(1);
    #[cfg(target_os = "macos")]
    let selection = crate::app_settings::transcription_selection().1;
    let offline: Offline = Box::new(move |model, samples| {
        let started = Instant::now();
        #[cfg(target_os = "macos")]
        let model = model
            .activate(&selection)
            .map_err(|error| format!("{error:#}"))?;
        let result = model
            .transcribe_segments(samples)
            .map_err(|error| format!("{error:#}"))?;
        let text = result.text.clone();
        let phases = Phases {
            queued_ms: millis(started.duration_since(submitted)),
            inference_ms: millis(started.elapsed()),
        };
        let _ = send.send((result, phases));
        Ok(text)
    });
    engine
        .foreground(samples, offline)?
        .wait()
        .map_err(|error| eyre!(error))?;
    transcript
        .try_recv()
        .map_err(|_| eyre!("the transcription returned no segments"))
}

/// Someone dictating on this machine goes first: a paired device's clip
/// waits (at most `LOCAL_DICTATION_WAIT`) rather than queue ahead of it.
fn wait_for_local_dictation() {
    let waiting = Instant::now();
    while crate::transcription::engine::dictation_busy() && waiting.elapsed() < LOCAL_DICTATION_WAIT
    {
        std::thread::sleep(LOCAL_DICTATION_POLL);
    }
}

fn millis(duration: Duration) -> u64 {
    duration.as_millis() as u64
}
