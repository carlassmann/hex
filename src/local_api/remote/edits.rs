//! `POST /remote/control`: a paired device's `hex` command, limited to the
//! settings a remote dictation depends on (`remote::settings::edits_remote`):
//! reading and changing the Output steps, vocabulary, and boost. A Mac with
//! Labs › Remote transcription forwards these here, from its window and its
//! `hex settings` / `hex vocabulary`, so they change this machine's steps.
//! Each runs through this app's control host exactly as a local `hex`
//! command would, and answers its `ControlResponse`.

use std::net::TcpStream;

use crate::control::{ControlCommand, ControlError, ControlReply, ControlRequest, ControlResponse};

use super::super::{
    HttpContext, HttpRequest, HttpResponse, RequestAction, UPLOAD_DEADLINE, read_body_bounded,
    write_response,
};
use super::{Action, paired_device, run_control};

/// A whole `dictation_processing.modes` value fits with room to spare.
const MAX_EDIT_BYTES: usize = 256 * 1024;

pub(super) fn action(request: HttpRequest) -> RequestAction {
    if paired_device(&request).is_none() {
        return RequestAction::Respond(HttpResponse::code(401, "not-paired"));
    }
    RequestAction::Remote(Action::Control {
        content_length: request.content_length.unwrap_or(0),
        body_prefix: request.body_prefix,
    })
}

pub(super) fn run(
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
        MAX_EDIT_BYTES,
        UPLOAD_DEADLINE,
    ) {
        Err(_) => HttpResponse::code(400, "invalid-request"),
        Ok(()) => match serde_json::from_slice::<ControlRequest>(&body) {
            Err(_) => HttpResponse::code(400, "invalid-control-command"),
            Ok(request) => respond(request, context),
        },
    };
    let _ = write_response(stream, response);
}

fn respond(request: ControlRequest, context: &HttpContext) -> HttpResponse {
    if let Err(error) = crate::control::check_version(&request) {
        return HttpResponse::json(200, &ControlResponse::from(ControlReply::from(error)));
    }
    if !crate::remote::settings::edits_remote(&request.command) {
        return HttpResponse::code(403, "not-allowed");
    }
    match run_control(request.command, context) {
        Ok(reply) => {
            tracing::info!(status = reply_status(&reply), "remote settings command");
            HttpResponse::json(200, &ControlResponse::from(reply))
        }
        Err(()) => HttpResponse::code(503, "control-unavailable"),
    }
}

fn reply_status(reply: &ControlReply) -> &'static str {
    match reply {
        ControlReply::Done { changed: true, .. } => "changed",
        ControlReply::Done { .. } => "unchanged",
        ControlReply::Error { .. } => "refused",
        _ => "answered",
    }
}

/// With Labs › Remote transcription on, the steps and vocabulary are the
/// paired Hex's, so this Mac's `hex settings` and `hex vocabulary` change
/// them there. `None` runs the command here.
pub(in crate::local_api) fn forwarded(command: &ControlCommand) -> Option<ControlReply> {
    if !crate::remote::settings::edits_remote(command) {
        return None;
    }
    let connection = crate::remote::client::active()?;
    Some(
        match crate::remote::settings::forward(&connection, command.clone()) {
            Ok(ControlReply::Done { message, changed }) => ControlReply::Done {
                message: format!("{message} (on {})", connection.machine_label),
                changed,
            },
            Ok(reply) => reply,
            Err(error) => ControlReply::from(ControlError::new(
                "remote-unreachable",
                format!("{error:#}. Remote transcription is on, so these settings live there"),
            )),
        },
    )
}
