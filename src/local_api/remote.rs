//! The remote listener: what paired devices reach through `tailscale serve`.
//! It shares the local API's transcription service but answers only these
//! routes, authenticated by a paired device's token (`remote_pairing`), never
//! the local API token:
//!
//! - `GET /.well-known/hex/machine`: this machine's id and name, unauthenticated,
//!   so a device can check it reached the machine it paired with.
//! - `POST /pair` `{code, label}`: trades a one-time code for a device token.
//! - `POST /remote/dictations`: a WAV recording in, `{text, raw}` out, transcribed
//!   with this machine's selected model and processed by its Modes, as if it
//!   were dictated here into an app named after the device (`X-Hex-Application`,
//!   default the device's label), so a mode can target it.
//! - `GET /remote/vocabulary`: this machine's word list, `{terms}`.
//! - `POST /remote/vocabulary` `{term, soundsLike?}` adds a word (or spellings);
//!   `POST /remote/vocabulary/remove` `{term}` removes one. Both answer the new
//!   `{terms, changed}`, and apply from the next dictation on.

use std::net::TcpStream;
use std::sync::atomic::AtomicBool;

use serde::{Deserialize, Serialize};

use crate::control::{ControlCall, ControlCommand, ControlReply, ControlRequest};

use super::{
    HttpContext, HttpRequest, HttpResponse, RequestAction, UPLOAD_DEADLINE, read_audio,
    read_body_bounded, wait_for_transcript, write_response,
};

const MAX_PAIR_BYTES: usize = 1024;
const MAX_VOCABULARY_EDIT_BYTES: usize = 4096;

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

pub(super) fn handle_request(request: HttpRequest, _context: &HttpContext) -> RequestAction {
    let path = request.path.split('?').next().unwrap_or("/").to_owned();
    match (request.method.as_str(), path.as_str()) {
        ("GET", "/.well-known/hex/machine") => RequestAction::Respond(machine()),
        ("POST", "/pair") => RequestAction::RemotePair {
            content_length: request.content_length.unwrap_or(0),
            body_prefix: request.body_prefix,
        },
        ("POST", "/remote/dictations") => dictation_action(request),
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
    match crate::remote_pairing::machine_id() {
        Ok(machine_id) => HttpResponse::json(
            200,
            &Machine {
                machine_id,
                label: crate::remote_pairing::machine_label(),
                version: env!("CARGO_PKG_VERSION"),
            },
        ),
        Err(error) => {
            tracing::error!(error = %format!("{error:#}"), "remote machine identity is unavailable");
            HttpResponse::code(500, "machine-unavailable")
        }
    }
}

fn paired_device(request: &HttpRequest) -> Option<crate::remote_pairing::Device> {
    request
        .authorization
        .as_deref()
        .and_then(|value| value.strip_prefix("Bearer "))
        .and_then(crate::remote_pairing::device_for_token)
}

fn dictation_action(request: HttpRequest) -> RequestAction {
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
    RequestAction::RemoteDictation {
        application,
        content_length,
        body_prefix: request.body_prefix,
    }
}

pub(super) fn pair(
    stream: &mut TcpStream,
    content_length: usize,
    mut body: Vec<u8>,
    context: &HttpContext,
) {
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
    let redeemed = crate::remote_pairing::redeem_pairing_code(&request.code, &request.label)
        .and_then(|pairing| Ok((pairing, crate::remote_pairing::machine_id()?)));
    match redeemed {
        Ok((Some(pairing), machine_id)) => {
            tracing::info!(device = %pairing.device.label, "remote device paired");
            HttpResponse::json(
                200,
                &PairResponse {
                    token: pairing.token,
                    device_id: pairing.device.id,
                    machine_id,
                    machine_label: crate::remote_pairing::machine_label(),
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

pub(super) fn dictate(
    stream: &mut TcpStream,
    application: String,
    content_length: usize,
    body: Vec<u8>,
    context: &HttpContext,
) {
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
    let clip = match read_audio(stream, content_length, body, &context.shutdown) {
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

    let processing_context = crate::context::ContextSnapshot {
        application: Some(application),
        ..crate::context::ContextSnapshot::default()
    };
    let profiles = crate::config::build_dictation_profiles(&settings.dictation_processing);
    let processed =
        profiles.process_cancellable(&raw, &processing_context, &AtomicBool::new(false));
    if let Some(fallback) = processed
        .observation
        .as_ref()
        .and_then(|observation| observation.fallback.as_ref())
    {
        tracing::warn!(fallback, "remote dictation post-processing fell back to the transcript");
    }
    tracing::info!(duration_ms, "remote dictation processed");
    Some(HttpResponse::json(
        200,
        &DictationResponse {
            text: processed.text,
            raw,
            duration_ms,
        },
    ))
}

fn vocabulary_action(request: HttpRequest, remove: bool) -> RequestAction {
    if paired_device(&request).is_none() {
        return RequestAction::Respond(HttpResponse::code(401, "not-paired"));
    }
    RequestAction::RemoteVocabulary {
        remove,
        content_length: request.content_length.unwrap_or(0),
        body_prefix: request.body_prefix,
    }
}

fn vocabulary(changed: Option<bool>) -> HttpResponse {
    let terms = crate::vocabulary::entries()
        .into_iter()
        .map(|entry| VocabularyTerm {
            term: entry.term,
            sounds_like: entry.sounds_like,
        })
        .collect();
    HttpResponse::json(200, &VocabularyResponse { terms, changed })
}

pub(super) fn edit_vocabulary(
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
                match run_vocabulary_edit(command, context) {
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

/// Edits the list the way `hex vocabulary add|remove` does, so a running Hex
/// window adopts it; a headless `hex service` has no control host and edits
/// the saved settings itself.
fn run_vocabulary_edit(command: ControlCommand, context: &HttpContext) -> Result<ControlReply, ()> {
    let Some(controls) = &context.control else {
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
