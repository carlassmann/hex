//! The settings a remote dictation depends on, mirrored from the paired Hex
//! (`GET /remote/settings`), so a Mac that falls back to its own model
//! processes the text as that machine would: its Modes (corrections,
//! OpenCode rewriting, transformation names), vocabulary, and boost. The
//! copy lives in `remote-settings.json` (0600) beside `remote-client.json`
//! and never touches `settings.json`; while Labs › Remote transcription is
//! on and the copy belongs to the connected machine, `AppSettings` applies
//! it in place of this Mac's own (`AppSettings::apply_runtime`). The model
//! is not mirrored: Labs says when the two machines use different ones.
//!
//! The same settings are edited there, not here, while the lab is on: the
//! window and `hex settings` / `hex vocabulary` send their commands
//! ([`edits_remote`]) to the paired Hex's `POST /remote/control`
//! ([`forward`]), then read the copy again.

use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use color_eyre::eyre::{Result, WrapErr, eyre};
use serde::{Deserialize, Serialize};

use crate::app_settings::{AppSettings, DictationProcessingSettings};
use crate::control::{ControlCommand, ControlReply, ControlRequest, ControlResponse};
use crate::dictation::vocabulary::{VocabularyBoost, VocabularyEntry};
use crate::remote::client::Connection;
use crate::transcription::models::TranscriptionSelection;

const FILE: &str = "remote-settings.json";
const FETCH_DEADLINE: Duration = Duration::from_secs(10);
/// A settings command; the paired Hex answers within its own 10 s.
const COMMAND_DEADLINE: Duration = Duration::from_secs(15);
/// How often the running Mac reads the other machine's settings again.
#[cfg(target_os = "macos")]
const REFRESH_INTERVAL: Duration = Duration::from_secs(5 * 60);

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteSettings {
    /// The machine these came from; a copy from another machine is ignored.
    pub machine_id: String,
    pub dictation_processing: DictationProcessingSettings,
    #[serde(default)]
    pub vocabulary: Vec<VocabularyEntry>,
    #[serde(default)]
    pub vocabulary_boost: VocabularyBoost,
    /// Shown beside this Mac's own model, never applied.
    pub transcription: TranscriptionSelection,
}

impl RemoteSettings {
    /// What this machine serves to its paired devices.
    pub fn of(settings: &AppSettings, machine_id: String) -> Self {
        Self {
            machine_id,
            dictation_processing: settings.dictation_processing.clone(),
            vocabulary: settings.vocabulary.clone(),
            vocabulary_boost: settings.vocabulary_boost,
            transcription: settings.transcription.clone(),
        }
    }
}

/// The Modes, vocabulary, and boost a Hex dictates with.
pub struct Applied {
    pub dictation_processing: DictationProcessingSettings,
    pub vocabulary: Vec<VocabularyEntry>,
    pub vocabulary_boost: VocabularyBoost,
}

/// The connected machine's copy while Labs › Remote transcription is on
/// (macOS), else `settings`' own.
pub fn applied(settings: &AppSettings) -> Applied {
    match settings
        .labs
        .remote_transcription
        .then(mirrored)
        .flatten()
        .filter(|_| cfg!(target_os = "macos"))
    {
        Some(remote) => Applied {
            dictation_processing: remote.dictation_processing,
            vocabulary: remote.vocabulary,
            vocabulary_boost: remote.vocabulary_boost,
        },
        None => Applied {
            dictation_processing: settings.dictation_processing.clone(),
            vocabulary: settings.vocabulary.clone(),
            vocabulary_boost: settings.vocabulary_boost,
        },
    }
}

/// The saved copy, when it belongs to the connected machine.
pub fn mirrored() -> Option<RemoteSettings> {
    let connection = crate::remote::client::connection().ok().flatten()?;
    let bytes = fs::read(path().ok()?).ok()?;
    let settings: RemoteSettings = serde_json::from_slice(&bytes)
        .inspect_err(|error| tracing::warn!(%error, "remote-settings.json is not valid"))
        .ok()?;
    (settings.machine_id == connection.machine_id).then_some(settings)
}

/// Reads the connected machine's settings and saves them. True when they
/// changed.
pub fn refresh(connection: &Connection) -> Result<bool> {
    let fetched = fetch(connection)?;
    if fetched.machine_id != connection.machine_id {
        return Err(eyre!(
            "{} answered for another machine",
            connection.machine_label
        ));
    }
    if mirrored().as_ref() == Some(&fetched) {
        return Ok(false);
    }
    crate::remote::pairing::write_private(&path()?, &serde_json::to_vec_pretty(&fetched)?)?;
    tracing::info!(
        machine = connection.machine_label,
        "mirrored the remote Hex's settings"
    );
    Ok(true)
}

/// The `hex` commands that read or change the mirrored settings, which a
/// paired device may send and a connected Mac sends instead of running.
pub fn edits_remote(command: &ControlCommand) -> bool {
    let mirrored = |key: &str| {
        key == "vocabulary_boost"
            || key == "dictation_processing"
            || key.starts_with("dictation_processing.")
    };
    match command {
        ControlCommand::SettingsGet { key: Some(key) }
        | ControlCommand::SettingsSet { key, .. } => mirrored(key),
        ControlCommand::SettingsApply { values } => {
            !values.is_empty() && values.keys().all(|key| mirrored(key))
        }
        ControlCommand::VocabularyList
        | ControlCommand::VocabularyAdd { .. }
        | ControlCommand::VocabularyRemove { .. } => true,
        _ => false,
    }
}

/// Runs one such command on the connected machine; after a change, reads
/// the copy again and applies it.
pub fn forward(connection: &Connection, command: ControlCommand) -> Result<ControlReply> {
    let reads = matches!(
        command,
        ControlCommand::SettingsGet { .. } | ControlCommand::VocabularyList
    );
    let reply = send(connection, &ControlRequest::new(command))?;
    if !reads && matches!(reply, ControlReply::Done { changed: true, .. }) {
        match refresh(connection) {
            Ok(true) => reapply(),
            Ok(false) => {}
            Err(error) => tracing::warn!(
                error = %format!("{error:#}"),
                "could not read the remote Hex's settings after a change"
            ),
        }
    }
    Ok(reply)
}

fn send(connection: &Connection, request: &ControlRequest) -> Result<ControlReply> {
    let url = format!("{}/remote/control", connection.url);
    let agent = crate::net::agent(&url, |config| {
        config
            .http_status_as_error(false)
            .timeout_global(Some(COMMAND_DEADLINE))
    });
    let mut response = agent
        .post(&url)
        .header("Authorization", format!("Bearer {}", connection.token()))
        .header("Content-Type", "application/json")
        .send(serde_json::to_vec(request)?)
        .map_err(|error| eyre!(crate::net::unreachable(&url, &error)))?;
    let status = response.status();
    let body = response.body_mut().read_to_string()?;
    match status.as_u16() {
        200 => {}
        403 | 404 => {
            return Err(eyre!(
                "{} runs a Hex that can't change these settings remotely; update it",
                connection.machine_label
            ));
        }
        _ => {
            return Err(eyre!(
                "{} answered {status}: {body}",
                connection.machine_label
            ));
        }
    }
    let response: ControlResponse = serde_json::from_str(&body).wrap_err_with(|| {
        format!(
            "{} sent a reply this Hex can't read",
            connection.machine_label
        )
    })?;
    Ok(response.reply)
}

pub fn forget() {
    if let Ok(path) = path() {
        let _ = fs::remove_file(path);
    }
}

fn fetch(connection: &Connection) -> Result<RemoteSettings> {
    let url = format!("{}/remote/settings", connection.url);
    let agent = crate::net::agent(&url, |config| {
        config
            .http_status_as_error(false)
            .timeout_global(Some(FETCH_DEADLINE))
    });
    let mut response = agent
        .get(&url)
        .header("Authorization", format!("Bearer {}", connection.token()))
        .call()
        .map_err(|error| eyre!(crate::net::unreachable(&url, &error)))?;
    let status = response.status();
    let body = response.body_mut().read_to_string()?;
    if status != 200 {
        return Err(eyre!(
            "{} answered {status}: {body}",
            connection.machine_label
        ));
    }
    serde_json::from_str(&body).wrap_err_with(|| {
        format!(
            "{} sent settings this Hex can't read",
            connection.machine_label
        )
    })
}

/// Keeps the copy current while the Mac runs: once shortly after launch,
/// then every [`REFRESH_INTERVAL`], only while the lab is on. A change
/// applies at once.
#[cfg(target_os = "macos")]
pub fn start_refreshing() {
    let started = std::thread::Builder::new()
        .name("hex-remote-settings".into())
        .spawn(|| {
            std::thread::sleep(Duration::from_secs(5));
            loop {
                if crate::desktop::process_exit::quitting() {
                    return;
                }
                if let Some(connection) = crate::remote::client::active() {
                    match refresh(&connection) {
                        Ok(true) => reapply(),
                        Ok(false) => {}
                        Err(error) => tracing::warn!(
                            error = %format!("{error:#}"),
                            "could not read the remote Hex's settings; keeping the last copy"
                        ),
                    }
                }
                std::thread::sleep(REFRESH_INTERVAL);
            }
        });
    if let Err(error) = started {
        tracing::warn!(%error, "could not start mirroring remote settings");
    }
}

/// Applies this Mac's settings again, so a new copy takes effect.
pub fn reapply() {
    if let Err(error) = AppSettings::load() {
        tracing::warn!(error = %format!("{error:#}"), "could not apply the mirrored settings");
    }
}

fn path() -> Result<PathBuf> {
    Ok(crate::app_paths::support_dir()?.join(FILE))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn served_settings_read_back_the_same() {
        let settings = AppSettings {
            vocabulary: vec![VocabularyEntry {
                term: "OpenCode".into(),
                ..VocabularyEntry::default()
            }],
            ..AppSettings::default()
        };
        let served = RemoteSettings::of(&settings, "machine".into());
        let read: RemoteSettings =
            serde_json::from_slice(&serde_json::to_vec(&served).unwrap()).unwrap();
        assert_eq!(read, served);
        assert_eq!(read.dictation_processing, settings.dictation_processing);
    }
}
