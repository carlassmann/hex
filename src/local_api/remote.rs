//! The remote listener: what paired devices reach through `tailscale serve`.
//! It shares the local API's transcription service but answers only these
//! routes, authenticated by a paired device's token (`remote::pairing`), never
//! the local API token:
//!
//! - `GET /.well-known/hex/machine`: this machine's id and name, unauthenticated,
//!   so a device can check it reached the machine it paired with.
//! - `POST /pair` `{code, label}`: trades a one-time code for a device token.
//! - `POST /remote/dictations`: a WAV recording in, `{text, raw}` out, transcribed
//!   with this machine's selected model and processed by its Modes, as if it
//!   were dictated here into an app named after the device (`X-Hex-Application`,
//!   default the device's label), so a mode can target it. With `?progress=1`
//!   it answers NDJSON progress instead (`remote::progress`).
//! - `POST /remote/streams` and `/remote/streams/{id}/{audio,heartbeat,cancel}`:
//!   the `/streams` routes (`docs/streams.md`) for a device that sends audio
//!   while it records, so a long dictation is mostly transcribed by its end.
//!   `POST /remote/streams/{id}/finish` answers NDJSON progress and then the
//!   text, processed by the Modes like a dictation.
//! - `POST /remote/transcriptions`: a WAV in, the raw transcript with segment
//!   and word times out, with no Modes (`remote::timed`), for a paired Mac's
//!   meeting chunks and Annotate words.
//! - `GET /remote/vocabulary`: this machine's word list, `{terms}`.
//! - `POST /remote/vocabulary` `{term, soundsLike?}` adds a word (or spellings);
//!   `POST /remote/vocabulary/remove` `{term}` removes one. Both answer the new
//!   `{terms, changed}`, and apply from the next dictation on.

use std::net::{TcpListener, TcpStream};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, mpsc};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::control::{ControlCall, ControlCommand, ControlReply, ControlRequest};

use super::{
    HttpContext, HttpRequest, HttpResponse, RequestAction, UPLOAD_DEADLINE, read_audio,
    read_body_bounded, serve, streams, wait_for_transcript, write_response,
};

mod journal;
mod progress;
mod rewrite;
mod timed;

/// The remote listener for paired devices; Hex runs on without it.
pub(super) fn start_listener(context: Arc<HttpContext>) -> Option<JoinHandle<()>> {
    let port = crate::remote::pairing::port();
    let listener = TcpListener::bind(("127.0.0.1", port))
        .and_then(|listener| listener.set_nonblocking(true).map(|()| listener))
        .inspect_err(|error| tracing::warn!(%error, port, "remote listener is unavailable"))
        .ok()?;
    let (ready, readiness) = mpsc::sync_channel(1);
    let worker = thread::Builder::new()
        .name("remote-api".into())
        .spawn(move || serve(listener, context, handle_request, ready))
        .inspect_err(|error| tracing::warn!(%error, "could not start the remote listener"))
        .ok()?;
    if readiness.recv_timeout(Duration::from_secs(1)) != Ok(true) {
        tracing::warn!("remote listener has no HTTP workers");
        return None;
    }
    tracing::info!(port, "remote listener started");
    Some(worker)
}

const MAX_PAIR_BYTES: usize = 1024;
const MAX_VOCABULARY_EDIT_BYTES: usize = 4096;
/// A phone uploading a long dictation over a slow cellular link.
const DICTATION_UPLOAD_DEADLINE: std::time::Duration = std::time::Duration::from_secs(5 * 60);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Machine {
    machine_id: String,
    label: String,
    version: &'static str,
}

#[derive(Deserialize)]
struct PairRequest {
    code: String,
    label: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PairResponse {
    token: String,
    device_id: String,
    machine_id: String,
    machine_label: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct VocabularyEdit {
    term: String,
    #[serde(default)]
    sounds_like: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct VocabularyTerm {
    term: String,
    sounds_like: Vec<String>,
}

#[derive(Serialize)]
struct VocabularyResponse {
    terms: Vec<VocabularyTerm>,
    #[serde(skip_serializing_if = "Option::is_none")]
    changed: Option<bool>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DictationResponse {
    text: String,
    raw: String,
    duration_ms: u64,
}

pub(super) fn handle_request(request: HttpRequest, context: &HttpContext) -> RequestAction {
    let (path, query) = request
        .path
        .split_once('?')
        .map_or((request.path.clone(), String::new()), |(path, query)| {
            (path.to_owned(), query.to_owned())
        });
    if let Some(rest) = path.strip_prefix("/remote")
        && (rest == "/streams" || rest.starts_with("/streams/"))
    {
        return stream_action(request, rest, context);
    }
    match (request.method.as_str(), path.as_str()) {
        ("GET", "/.well-known/hex/machine") => RequestAction::Respond(machine()),
        ("POST", "/pair") => RequestAction::Remote(Action::Pair {
            content_length: request.content_length.unwrap_or(0),
            body_prefix: request.body_prefix,
        }),
        ("POST", "/remote/dictations") => dictation_action(request, query == "progress=1"),
        ("POST", "/remote/transcriptions") => timed::action(request),
        ("GET", "/remote/vocabulary") => match paired_device(&request) {
            Some(_) => RequestAction::Respond(vocabulary(None)),
            None => RequestAction::Respond(HttpResponse::code(401, "not-paired")),
        },
        ("POST", "/remote/vocabulary") => vocabulary_action(request, false),
        ("POST", "/remote/vocabulary/remove") => vocabulary_action(request, true),
        _ => RequestAction::Respond(HttpResponse::code(404, "not-found")),
    }
}

fn machine() -> HttpResponse {
    match crate::remote::pairing::machine_id() {
        Ok(machine_id) => HttpResponse::json(
            200,
            &Machine {
                machine_id,
                label: crate::remote::pairing::machine_label(),
                version: env!("CARGO_PKG_VERSION"),
            },
        ),
        Err(error) => {
            tracing::error!(error = %format!("{error:#}"), "remote machine identity is unavailable");
            HttpResponse::code(500, "machine-unavailable")
        }
    }
}

pub(super) fn paired_device(request: &HttpRequest) -> Option<crate::remote::pairing::Device> {
    request
        .authorization
        .as_deref()
        .and_then(|value| value.strip_prefix("Bearer "))
        .and_then(crate::remote::pairing::device_for_token)
}

/// The `/streams` routes behind a paired device's token; `path` is past `/remote`.
fn stream_action(request: HttpRequest, path: &str, context: &HttpContext) -> RequestAction {
    let Some(device) = paired_device(&request) else {
        return RequestAction::Respond(HttpResponse::code(401, "not-paired"));
    };
    if request.method == "POST" && path == "/streams" {
        return streams::create_action(request);
    }
    let Some((id, action)) = streams::stream_path(path) else {
        return RequestAction::Respond(HttpResponse::code(404, "not-found"));
    };
    let application = request
        .remote_application
        .clone()
        .unwrap_or_else(|| device.label.clone());
    if request.method != "POST" || action != "finish" {
        if action == "audio"
            && let Some(stream) = request.stream_token.as_deref().and_then(|token| {
                context
                    .streams
                    .get(id)
                    .filter(|stream| stream.owned_by(token))
            })
        {
            // The whole recording is transcribed again at its finish.
            stream.keep_audio();
            rewrite::watch(stream, application);
        }
        let action = action.to_owned();
        return streams::route(request, id, &action, context);
    }
    let stream = request.stream_token.as_deref().and_then(|token| {
        context
            .streams
            .get(id)
            .filter(|stream| stream.owned_by(token))
    });
    match stream {
        Some(stream) => RequestAction::Remote(Action::StreamFinish {
            stream,
            application,
        }),
        None => RequestAction::Respond(HttpResponse::code(404, "stream-not-found")),
    }
}

fn dictation_action(request: HttpRequest, progress: bool) -> RequestAction {
    let Some(device) = paired_device(&request) else {
        return RequestAction::Respond(HttpResponse::code(401, "not-paired"));
    };
    if !request.has_content_type("audio/wav") {
        return RequestAction::Respond(HttpResponse::code(415, "unsupported-audio"));
    }
    let Some(content_length) = request.content_length else {
        return RequestAction::Respond(HttpResponse::code(411, "length-required"));
    };
    let application = request
        .remote_application
        .clone()
        .unwrap_or_else(|| device.label.clone());
    RequestAction::Remote(Action::Dictation {
        application,
        content_length,
        body_prefix: request.body_prefix,
        progress,
    })
}

/// What a paired device's request leaves to do once routed.
pub(super) enum Action {
    Pair {
        content_length: usize,
        body_prefix: Vec<u8>,
    },
    Dictation {
        application: String,
        content_length: usize,
        body_prefix: Vec<u8>,
        progress: bool,
    },
    Timed {
        content_length: usize,
        body_prefix: Vec<u8>,
    },
    StreamFinish {
        stream: std::sync::Arc<crate::transcription::stream::Stream>,
        application: String,
    },
    Vocabulary {
        remove: bool,
        content_length: usize,
        body_prefix: Vec<u8>,
    },
}

pub(super) fn handle(connection: &mut TcpStream, action: Action, context: &HttpContext) {
    match action {
        Action::Pair {
            content_length,
            body_prefix,
        } => pair(connection, content_length, body_prefix, context),
        Action::Dictation {
            application,
            content_length,
            body_prefix,
            progress,
        } => dictate(
            connection,
            application,
            content_length,
            body_prefix,
            progress,
            context,
        ),
        Action::Timed {
            content_length,
            body_prefix,
        } => timed::transcribe(connection, content_length, body_prefix, context),
        Action::StreamFinish {
            stream,
            application,
        } => progress::finish_stream(connection, &stream, application, context),
        Action::Vocabulary {
            remove,
            content_length,
            body_prefix,
        } => edit_vocabulary(connection, remove, content_length, body_prefix, context),
    }
}

fn pair(stream: &mut TcpStream, content_length: usize, mut body: Vec<u8>, context: &HttpContext) {
    let response = match read_body_bounded(
        stream,
        content_length,
        &mut body,
        &context.shutdown,
        MAX_PAIR_BYTES,
        UPLOAD_DEADLINE,
    ) {
        Err(_) => HttpResponse::code(400, "invalid-request"),
        Ok(()) => match serde_json::from_slice::<PairRequest>(&body) {
            Err(_) => HttpResponse::code(400, "invalid-request"),
            Ok(request) => redeem(&request),
        },
    };
    let _ = write_response(stream, response);
}

fn redeem(request: &PairRequest) -> HttpResponse {
    let redeemed = crate::remote::pairing::redeem_pairing_code(&request.code, &request.label)
        .and_then(|pairing| Ok((pairing, crate::remote::pairing::machine_id()?)));
    match redeemed {
        Ok((Some(pairing), machine_id)) => {
            tracing::info!(device = %pairing.device.label, "remote device paired");
            HttpResponse::json(
                200,
                &PairResponse {
                    token: pairing.token,
                    device_id: pairing.device.id,
                    machine_id,
                    machine_label: crate::remote::pairing::machine_label(),
                },
            )
        }
        Ok((None, _)) => HttpResponse::code(403, "invalid-code"),
        Err(error) => {
            tracing::error!(error = %format!("{error:#}"), "remote pairing failed");
            HttpResponse::code(500, "pairing-failed")
        }
    }
}

fn dictate(
    stream: &mut TcpStream,
    application: String,
    content_length: usize,
    body: Vec<u8>,
    progress: bool,
    context: &HttpContext,
) {
    if progress {
        let uploading = std::time::Instant::now();
        match read_audio(
            stream,
            content_length,
            body,
            &context.shutdown,
            DICTATION_UPLOAD_DEADLINE,
        ) {
            Ok(clip) => {
                let upload_ms = uploading.elapsed().as_millis() as u64;
                progress::dictate(stream, clip, upload_ms, application, context);
            }
            Err(Some(response)) => {
                let _ = write_response(stream, response);
            }
            Err(None) => {}
        }
        return;
    }
    if let Some(response) =
        transcribe_and_process(stream, application, content_length, body, context)
    {
        let _ = write_response(stream, response);
    }
}

fn transcribe_and_process(
    stream: &mut TcpStream,
    application: String,
    content_length: usize,
    body: Vec<u8>,
    context: &HttpContext,
) -> Option<HttpResponse> {
    let settings = match crate::app_settings::AppSettings::load() {
        Ok(settings) => settings,
        Err(error) => {
            tracing::error!(error = %format!("{error:#}"), "remote dictation could not read settings");
            return Some(HttpResponse::code(500, "settings-unavailable"));
        }
    };
    let selection = settings.transcription.clone();
    if !super::model_ready(&selection) {
        return Some(HttpResponse::code(409, "model-not-ready"));
    }
    let Some(admission) = context.transcription.admit_audio() else {
        return Some(HttpResponse::code(429, "queue-full"));
    };
    let clip = match read_audio(
        stream,
        content_length,
        body,
        &context.shutdown,
        DICTATION_UPLOAD_DEADLINE,
    ) {
        Ok(clip) => clip,
        Err(response) => return response,
    };
    let duration_ms = clip.duration_ms;
    let raw = match wait_for_transcript(
        stream,
        selection,
        clip,
        admission,
        &context.transcription,
        &context.shutdown,
    ) {
        Ok(raw) => raw,
        Err(response) => return response,
    };
    let text = process(&raw, application, None).text;
    tracing::info!(duration_ms, "remote dictation processed");
    Some(HttpResponse::json(
        200,
        &DictationResponse {
            text,
            raw,
            duration_ms,
        },
    ))
}

/// Runs this machine's Modes on a device's transcript, as if it were dictated
/// here into `application`.
/// Runs the Modes on `raw`; `deadline` replaces their OpenCode deadline.
pub(super) fn process(
    raw: &str,
    application: String,
    deadline: Option<std::time::Duration>,
) -> crate::dictation::processor::Processed {
    let mut settings = match crate::app_settings::AppSettings::load() {
        Ok(settings) => settings,
        Err(error) => {
            tracing::error!(error = %format!("{error:#}"), "remote dictation could not read settings");
            return crate::dictation::processor::Processed {
                text: raw.to_owned(),
                observation: None,
                transformations: Vec::new(),
            };
        }
    };
    let processing_context = crate::dictation::context::ContextSnapshot {
        application: Some(application),
        ..crate::dictation::context::ContextSnapshot::default()
    };
    if let Some(deadline) = deadline {
        let processing = &mut settings.dictation_processing;
        for mode in std::iter::once(&mut processing.default_mode).chain(&mut processing.modes) {
            mode.post_processing.deadline_seconds = deadline.as_secs();
        }
    }
    let profiles =
        crate::dictation::config::build_dictation_profiles(&settings.dictation_processing);
    let processed = profiles.process_cancellable(
        raw,
        &settings.transcription.language,
        &processing_context,
        &AtomicBool::new(false),
        crate::dictation::processor::Source::Dictation,
    );
    if let Some(fallback) = processed
        .observation
        .as_ref()
        .and_then(|observation| observation.fallback.as_ref())
    {
        tracing::warn!(
            fallback,
            "remote dictation post-processing fell back to the transcript"
        );
    }
    processed
}

fn vocabulary_action(request: HttpRequest, remove: bool) -> RequestAction {
    if paired_device(&request).is_none() {
        return RequestAction::Respond(HttpResponse::code(401, "not-paired"));
    }
    RequestAction::Remote(Action::Vocabulary {
        remove,
        content_length: request.content_length.unwrap_or(0),
        body_prefix: request.body_prefix,
    })
}

fn vocabulary(changed: Option<bool>) -> HttpResponse {
    let terms = crate::dictation::vocabulary::entries()
        .into_iter()
        .map(|entry| VocabularyTerm {
            term: entry.term,
            sounds_like: entry.sounds_like,
        })
        .collect();
    HttpResponse::json(200, &VocabularyResponse { terms, changed })
}

fn edit_vocabulary(
    stream: &mut TcpStream,
    remove: bool,
    content_length: usize,
    mut body: Vec<u8>,
    context: &HttpContext,
) {
    let response = match read_body_bounded(
        stream,
        content_length,
        &mut body,
        &context.shutdown,
        MAX_VOCABULARY_EDIT_BYTES,
        UPLOAD_DEADLINE,
    ) {
        Err(_) => HttpResponse::code(400, "invalid-request"),
        Ok(()) => match serde_json::from_slice::<VocabularyEdit>(&body) {
            Err(_) => HttpResponse::code(400, "invalid-request"),
            Ok(edit) => {
                let command = if remove {
                    ControlCommand::VocabularyRemove { term: edit.term }
                } else {
                    ControlCommand::VocabularyAdd {
                        term: edit.term,
                        sounds_like: edit.sounds_like,
                    }
                };
                match run_control(command, context) {
                    Ok(ControlReply::Done { changed, .. }) => vocabulary(Some(changed)),
                    Ok(ControlReply::Error { code, .. }) if code == "invalid-value" => {
                        HttpResponse::code(400, "invalid-term")
                    }
                    Ok(_) | Err(()) => HttpResponse::code(500, "vocabulary-unavailable"),
                }
            }
        },
    };
    let _ = write_response(stream, response);
}

/// Runs a command the way `hex` does, so a running Hex window adopts it; a
/// headless `hex service` has no control host and edits the saved
/// vocabulary itself (and nothing else).
pub(super) fn run_control(
    command: ControlCommand,
    context: &HttpContext,
) -> Result<ControlReply, ()> {
    let Some(controls) = &context.control else {
        if !matches!(
            command,
            ControlCommand::VocabularyAdd { .. } | ControlCommand::VocabularyRemove { .. }
        ) {
            return Err(());
        }
        let mut settings = crate::app_settings::AppSettings::load().map_err(|_| ())?;
        let reply = match crate::control::edit_vocabulary(&mut settings.vocabulary, &command) {
            Ok(reply) => reply,
            Err(error) => return Ok(ControlReply::from(error)),
        };
        if matches!(reply, ControlReply::Done { changed: true, .. }) {
            settings.save().map_err(|_| ())?;
        }
        return Ok(reply);
    };
    let request = ControlRequest::new(command);
    let deadline = request.command.deadline();
    let (reply, response) = std::sync::mpsc::sync_channel(1);
    controls
        .try_send(ControlCall { request, reply })
        .map_err(|_| ())?;
    response
        .recv_timeout(deadline)
        .map(|response| response.reply)
        .map_err(|_| ())
}
