//! Text for a paired device, with progress while it waits. Both ways in answer
//! NDJSON, one line about every second while this machine works, so the
//! device can say what it is waiting on and tell a slow machine from a gone
//! one:
//!
//! - `POST /remote/streams/{id}/finish`: the stream the device fed while it
//!   recorded, so only its last window is left to transcribe.
//! - `POST /remote/dictations?progress=1`: a whole WAV, fed through a stream
//!   on the dictation engine's loaded model (never a second copy), or the
//!   transcription service when a stream can't start.
//!
//! ```text
//! {"stage":"received","durationMs":61200}
//! {"stage":"transcribing","elapsedMs":1200,"backlogMs":3400}
//! {"stage":"rewriting","elapsedMs":2600}
//! {"stage":"done","text":"…","raw":"…","durationMs":61200,"transcribeMs":1400,"rewriteMs":1800}
//! {"stage":"failed","code":"…","message":"…"}
//! ```

use std::io::Write;
use std::net::TcpStream;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::super::{HttpContext, generate_token, write_stream_headers};
use super::journal::{self, Entry, Rewrite};
use crate::transcription::service::{AudioClip, TranscriptionServiceError};
use crate::transcription::stream::{CreateError, End, PushError, SampleFormat, Segment, Stream};

const TICK: Duration = Duration::from_secs(1);
/// How long the device may wait for its text before Hex gives up on it.
const MAX_TRANSCRIBE_WAIT: Duration = Duration::from_secs(10 * 60);
/// A whole clip goes into its stream a second at a time, as a recorder would.
const FEED_SAMPLES: usize = crate::transcription::stream::SAMPLE_RATE;
const BACKLOG_RETRY: Duration = Duration::from_millis(200);

/// One NDJSON line at a time to the waiting device.
struct Progress {
    connection: TcpStream,
    started: Instant,
}

impl Progress {
    fn start(connection: &mut TcpStream) -> Option<Self> {
        write_stream_headers(connection, "application/x-ndjson").ok()?;
        Some(Self {
            connection: connection.try_clone().ok()?,
            started: Instant::now(),
        })
    }

    fn elapsed_ms(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }

    /// `false` once the device is gone.
    fn send(&mut self, mut line: Value) -> bool {
        if let Some(fields) = line.as_object_mut() {
            fields
                .entry("elapsedMs")
                .or_insert_with(|| json!(self.started.elapsed().as_millis() as u64));
        }
        let Ok(mut bytes) = serde_json::to_vec(&line) else {
            return false;
        };
        bytes.push(b'\n');
        self.connection
            .write_all(&bytes)
            .and_then(|()| self.connection.flush())
            .is_ok()
    }

    fn fail(&mut self, code: &str, message: &str) {
        self.send(json!({ "stage": "failed", "code": code, "message": message }));
    }
}

/// Why a device's text didn't come.
struct Failure {
    code: &'static str,
    message: String,
}

impl Failure {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

pub(super) fn finish_stream(
    connection: &mut TcpStream,
    stream: &Stream,
    application: String,
    context: &HttpContext,
) {
    let Some(mut progress) = Progress::start(connection) else {
        stream.cancel();
        return;
    };
    let audio_ms = stream.received_ms();
    let backlog_ms = stream.backlog_ms();
    let mut entry = Entry {
        route: "stream",
        application: application.clone(),
        audio_ms,
        ..Entry::default()
    };
    let transcribing = Instant::now();
    let early = super::rewrite::take(stream.id);
    let transcript = wait_for_stream(&mut progress, stream, context);
    entry.audio = stream.take_kept_audio().unwrap_or_default();
    let (raw, segments) = match transcript {
        Ok(transcript) => transcript,
        Err(failure) => {
            tracing::warn!(stream = stream.id, code = failure.code, message = %failure.message, audio_ms, "remote stream failed");
            entry.error = Some(format!("{}: {}", failure.code, failure.message));
            journal::record(entry);
            return progress.fail(failure.code, &failure.message);
        }
    };
    entry.transcribe_ms = transcribing.elapsed().as_millis() as u64;
    entry.raw = raw.clone();
    entry.segments = segments.clone();
    let rewritten = match early {
        Some(early) => rewrite_rest(&mut progress, early, &raw, segments, application),
        None => rewrite(&mut progress, &raw, application),
    };
    let Some(rewritten) = rewritten else {
        entry.error = Some("device-gone: the device stopped waiting".into());
        return journal::record(entry);
    };
    tracing::info!(
        stream = stream.id,
        audio_ms,
        backlog_ms,
        transcribe_ms = entry.transcribe_ms,
        rewrite_ms = rewritten.ms,
        early_pieces = rewritten.pieces,
        kept_as_heard = rewritten.kept_as_heard,
        total_ms = progress.elapsed_ms(),
        "remote stream finished"
    );
    progress.send(done(
        &rewritten.text,
        &raw,
        audio_ms,
        entry.transcribe_ms,
        rewritten.ms,
    ));
    entry.finish(rewritten);
}

/// A whole WAV, already uploaded and decoded, transcribed with progress.
pub(super) fn dictate(
    connection: &mut TcpStream,
    clip: AudioClip,
    upload_ms: u64,
    application: String,
    context: &HttpContext,
) {
    let Some(mut progress) = Progress::start(connection) else {
        return;
    };
    let audio_ms = clip.duration_ms;
    let mut entry = Entry {
        route: "upload",
        application: application.clone(),
        audio_ms,
        audio: journal::retention()
            .map(|_| clip.samples().to_vec())
            .unwrap_or_default(),
        ..Entry::default()
    };
    progress.send(json!({ "stage": "received", "durationMs": audio_ms }));
    let transcribing = Instant::now();
    let (route, raw) = match start_stream(context) {
        Some(stream) => (
            "stream",
            feed(&mut progress, &stream, clip.samples(), context)
                .and_then(|()| wait_for_stream(&mut progress, &stream, context))
                .map(|(raw, _)| raw)
                .inspect_err(|_| stream.cancel()),
        ),
        None => ("service", wait_for_service(&mut progress, clip, context)),
    };
    let raw = match raw {
        Ok(raw) => raw,
        Err(failure) => {
            tracing::warn!(route, code = failure.code, message = %failure.message, audio_ms, "remote dictation failed");
            entry.error = Some(format!("{}: {}", failure.code, failure.message));
            journal::record(entry);
            return progress.fail(failure.code, &failure.message);
        }
    };
    entry.transcribe_ms = transcribing.elapsed().as_millis() as u64;
    entry.raw = raw.clone();
    let Some(rewritten) = rewrite(&mut progress, &raw, application) else {
        entry.error = Some("device-gone: the device stopped waiting".into());
        return journal::record(entry);
    };
    tracing::info!(
        route,
        audio_ms,
        upload_ms,
        transcribe_ms = entry.transcribe_ms,
        rewrite_ms = rewritten.ms,
        total_ms = progress.elapsed_ms(),
        "remote dictation finished"
    );
    progress.send(done(
        &rewritten.text,
        &raw,
        audio_ms,
        entry.transcribe_ms,
        rewritten.ms,
    ));
    entry.finish(rewritten);
}

/// A transcript through the Modes.
struct Rewritten {
    text: String,
    ms: u64,
    /// Pieces rewritten while the stream recorded.
    pieces: usize,
    kept_as_heard: usize,
    rewrites: Vec<Rewrite>,
}

impl Entry {
    fn finish(mut self, rewritten: Rewritten) {
        self.rewrite_ms = rewritten.ms;
        self.text = rewritten.text;
        self.rewrites = rewritten.rewrites;
        journal::record(self);
    }
}

fn done(text: &str, raw: &str, audio_ms: u64, transcribe_ms: u64, rewrite_ms: u64) -> Value {
    json!({
        "stage": "done",
        "text": text,
        "raw": raw,
        "durationMs": audio_ms,
        "transcribeMs": transcribe_ms,
        "rewriteMs": rewrite_ms,
    })
}

/// A stream on the dictation engine's loaded model, when one can start now.
fn start_stream(context: &HttpContext) -> Option<Arc<Stream>> {
    let (engine, selection) = match (context.stream_engine)() {
        Ok(engine) => engine,
        Err(code) => {
            tracing::info!(code, "remote dictation runs on the transcription service");
            return None;
        }
    };
    let token = generate_token().ok()?;
    let created = context
        .streams
        .create(engine, SampleFormat::F32le, selection, token);
    if let Err(CreateError::Busy) = created {
        tracing::info!(
            "another stream is live; remote dictation runs on the transcription service"
        );
    }
    created.ok()
}

fn feed(
    progress: &mut Progress,
    stream: &Stream,
    samples: &[f32],
    context: &HttpContext,
) -> Result<(), Failure> {
    let mut last_tick = Instant::now();
    let mut chunks = samples.chunks(FEED_SAMPLES).peekable();
    while let Some(chunk) = chunks.peek() {
        if context.shutdown.load(Ordering::Acquire) {
            return Err(Failure::new("shutting-down", "Hex is quitting"));
        }
        match stream.push(chunk) {
            Ok(()) => {
                chunks.next();
            }
            Err(PushError::BacklogFull) => {
                stream.heartbeat();
                std::thread::sleep(BACKLOG_RETRY);
            }
            Err(PushError::Ended | PushError::Finishing) => {
                return Err(Failure::new(
                    "stream-ended",
                    "the transcription stream ended",
                ));
            }
        }
        if last_tick.elapsed() >= TICK {
            last_tick = Instant::now();
            if !progress.send(transcribing(stream)) {
                return Err(Failure::new("device-gone", "the device stopped waiting"));
            }
        }
    }
    Ok(())
}

fn transcribing(stream: &Stream) -> Value {
    json!({ "stage": "transcribing", "backlogMs": stream.backlog_ms() })
}

/// The stream's text and the windows it was made of.
fn wait_for_stream(
    progress: &mut Progress,
    stream: &Stream,
    context: &HttpContext,
) -> Result<(String, Vec<Segment>), Failure> {
    let stop = || context.shutdown.load(Ordering::Acquire);
    let started = Instant::now();
    if !progress.send(transcribing(stream)) {
        stream.cancel();
        return Err(Failure::new("device-gone", "the device stopped waiting"));
    }
    loop {
        match stream.finish(TICK, &stop) {
            Some(End::Final { text, segments, .. }) => return Ok((text, segments)),
            Some(End::Failed { code, message }) => return Err(Failure::new(code, message)),
            Some(End::Cancelled) => {
                return Err(Failure::new("stream-cancelled", "the stream was cancelled"));
            }
            None if stop() => return Err(Failure::new("shutting-down", "Hex is quitting")),
            None if started.elapsed() >= MAX_TRANSCRIBE_WAIT => {
                stream.cancel();
                return Err(Failure::new(
                    "transcription-timeout",
                    "transcription took too long",
                ));
            }
            None => {
                if !progress.send(transcribing(stream)) {
                    stream.cancel();
                    return Err(Failure::new("device-gone", "the device stopped waiting"));
                }
            }
        }
    }
}

/// The transcription service's whole-clip path, for when no stream can
/// start (another one is live, or the engine holds no GGUF model).
fn wait_for_service(
    progress: &mut Progress,
    clip: AudioClip,
    context: &HttpContext,
) -> Result<String, Failure> {
    let settings = crate::app_settings::AppSettings::load()
        .map_err(|_| Failure::new("settings-unavailable", "Hex could not read its settings"))?;
    let selection = settings.transcription;
    if !super::super::model_ready(&selection) {
        return Err(Failure::new(
            "model-not-ready",
            "the speech model isn't ready",
        ));
    }
    let admission = context
        .transcription
        .admit_audio()
        .ok_or_else(|| Failure::new("queue-full", "another transcription is running"))?;
    let job = context
        .transcription
        .transcribe(selection, clip, admission)
        .map_err(service_failure)?;
    let started = Instant::now();
    loop {
        match job.wait_timeout(TICK) {
            Ok(Some(Ok(transcript))) => return Ok(transcript),
            Ok(Some(Err(error))) | Err(error) => return Err(service_failure(error)),
            Ok(None) => {
                let gone = !progress.send(json!({ "stage": "transcribing" }));
                if gone
                    || context.shutdown.load(Ordering::Acquire)
                    || started.elapsed() >= MAX_TRANSCRIBE_WAIT
                {
                    job.cancel();
                    return Err(Failure::new(
                        "transcription-timeout",
                        "transcription stopped",
                    ));
                }
            }
        }
    }
}

fn service_failure(error: TranscriptionServiceError) -> Failure {
    match error {
        TranscriptionServiceError::QueueFull => {
            Failure::new("queue-full", "another transcription is running")
        }
        TranscriptionServiceError::Unavailable => {
            Failure::new("service-gone", "the transcription service stopped")
        }
        TranscriptionServiceError::Cancelled => Failure::new("cancelled", "cancelled"),
        TranscriptionServiceError::Model(error) => Failure::new("model-load-failed", error),
        TranscriptionServiceError::Inference(error) => Failure::new("transcription-failed", error),
    }
}

/// Runs the Modes on `raw`, sending a line each second; `None` when the
/// device left meanwhile.
fn rewrite(progress: &mut Progress, raw: &str, application: String) -> Option<Rewritten> {
    let started = Instant::now();
    if !progress.send(json!({ "stage": "rewriting" })) {
        return None;
    }
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    let raw_owned = raw.to_owned();
    std::thread::Builder::new()
        .name("remote-rewrite".into())
        .spawn(move || {
            let _ = sender.send(super::process(&raw_owned, application, None));
        })
        .ok()?;
    let processed = loop {
        match receiver.recv_timeout(TICK) {
            Ok(processed) => break Some(processed),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                if !progress.send(json!({ "stage": "rewriting" })) {
                    return None;
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break None,
        }
    };
    let ms = started.elapsed().as_millis() as u64;
    let rewrites = processed
        .iter()
        .map(|processed| Rewrite::new("whole", raw, processed, ms))
        .collect();
    Some(Rewritten {
        text: processed.map_or_else(|| raw.to_owned(), |processed| processed.text),
        ms,
        pieces: 0,
        kept_as_heard: 0,
        rewrites,
    })
}

/// Finishes a stream rewritten while it recorded: only the text after its
/// last piece is left, plus any piece OpenCode still has, waited on as long
/// as the Modes' own deadline.
fn rewrite_rest(
    progress: &mut Progress,
    early: Arc<super::rewrite::EarlyRewrite>,
    raw: &str,
    segments: Vec<Segment>,
    application: String,
) -> Option<Rewritten> {
    let started = Instant::now();
    if !progress.send(json!({ "stage": "rewriting" })) {
        return None;
    }
    let wait = crate::app_settings::AppSettings::load()
        .map(|settings| {
            settings
                .dictation_processing
                .default_mode
                .post_processing
                .deadline_seconds
        })
        .map_or(Duration::from_secs(10), Duration::from_secs);
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    std::thread::Builder::new()
        .name("remote-rewrite".into())
        .spawn(move || {
            let finished = early.finish(&segments, wait, |rest| {
                super::process(rest, application, None)
            });
            let _ = sender.send(finished);
        })
        .ok()?;
    let finished = loop {
        match receiver.recv_timeout(TICK) {
            Ok(finished) => break finished,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                if !progress.send(json!({ "stage": "rewriting" })) {
                    return None;
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                break super::rewrite::Finished {
                    text: raw.to_owned(),
                    pieces: 0,
                    kept_as_heard: 0,
                    rewrites: Vec::new(),
                };
            }
        }
    };
    Some(Rewritten {
        text: finished.text,
        ms: started.elapsed().as_millis() as u64,
        pieces: finished.pieces,
        kept_as_heard: finished.kept_as_heard,
        rewrites: finished.rewrites,
    })
}
