//! Editing the paired Hex's steps from this window. While Labs › Remote
//! transcription is on and its settings are mirrored (`remote::settings`),
//! the window shows that machine's Output steps, vocabulary, and boost in
//! place of this Mac's own, and an edit to them is sent there as `hex`
//! commands (`POST /remote/control`) instead of saved here. This Mac's own
//! values stay in `settings.json` and show again once the lab is off.
//!
//! The shown copy changes at once; one worker thread sends the edits in
//! order, then reads the copy again. A refused or unreachable send says why
//! and shows what the paired Hex still has.

use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, channel, sync_channel};

use super::Ui;
use crate::app_settings::AppSettings;
use crate::control::{ControlCommand, ControlReply};
use crate::dictation::vocabulary::VocabularyEntry;
use crate::remote::settings::RemoteSettings;

/// Edits waiting to be sent; typing saves at most every pause.
const QUEUED_EDITS: usize = 16;

#[derive(Default)]
pub(super) struct RemoteEdits {
    sender: Option<SyncSender<Vec<ControlCommand>>>,
    results: Option<Receiver<Result<(), String>>>,
    /// Sends not yet answered; the mirrored file is stale until they are.
    pending: usize,
}

impl RemoteEdits {
    pub(super) fn sending(&self) -> bool {
        self.pending > 0
    }
}

/// The mirrored settings this window shows and edits for `settings`, when
/// its lab is on.
pub(super) fn mirror<'a>(ui: &'a Ui, settings: &AppSettings) -> Option<&'a RemoteSettings> {
    let feature = crate::labs::Feature::RemoteTranscription;
    if !feature.shipped() || !settings.labs.remote_transcription || ui.remote.connection.is_none() {
        return None;
    }
    ui.remote.mirror.as_ref()
}

/// The machine whose steps the window shows, when they aren't this Mac's.
pub(super) fn machine(ui: &Ui) -> Option<&str> {
    mirror(ui, &ui.settings)?;
    Some(&ui.remote.connection.as_ref()?.machine_label)
}

/// `settings` as the window shows them: the paired Hex's steps and
/// vocabulary while the lab is on.
pub(super) fn shown(ui: &Ui, mut settings: AppSettings) -> AppSettings {
    if let Some(mirror) = mirror(ui, &settings) {
        settings.dictation_processing = mirror.dictation_processing.clone();
        settings.vocabulary = mirror.vocabulary.clone();
        settings.vocabulary_boost = mirror.vocabulary_boost;
    }
    settings
}

/// Shows the mirror again after it changed underneath.
pub(super) fn reshow(ui: &mut Ui) -> bool {
    match ui.service_settings.clone() {
        Some(saved) => ui.show(shown(ui, saved)),
        None => false,
    }
}

/// Splits a save: what `settings.json` keeps (this Mac's own steps and
/// vocabulary) and the commands that make the paired Hex match `candidate`.
pub(super) fn split(ui: &Ui, candidate: &AppSettings) -> (AppSettings, Vec<ControlCommand>) {
    let Some(mirror) = mirror(ui, &ui.settings) else {
        return (candidate.clone(), Vec::new());
    };
    let mut local = candidate.clone();
    if let Some(saved) = &ui.service_settings {
        local.dictation_processing = saved.dictation_processing.clone();
        local.vocabulary = saved.vocabulary.clone();
        local.vocabulary_boost = saved.vocabulary_boost;
    }
    (local, commands(mirror, candidate))
}

/// Shows `candidate`'s steps as the paired Hex's at once and sends them.
pub(super) fn send(ui: &mut Ui, commands: Vec<ControlCommand>, candidate: &AppSettings) {
    if commands.is_empty() {
        return;
    }
    if let Some(mirror) = ui.remote.mirror.as_mut() {
        mirror.dictation_processing = candidate.dictation_processing.clone();
        mirror.vocabulary = candidate.vocabulary.clone();
        mirror.vocabulary_boost = candidate.vocabulary_boost;
    }
    if !ui.live() {
        return;
    }
    let edits = &mut ui.remote.edits;
    let sender = edits.sender.get_or_insert_with(|| {
        let (sender, results) = start_worker();
        edits.results = Some(results);
        sender
    });
    match sender.try_send(commands) {
        Ok(()) => edits.pending += 1,
        Err(_) => {
            ui.error = Some("Too many edits waiting to reach the other Hex; try again".into())
        }
    }
}

/// Takes the worker's answers. True when the window should draw again.
pub(super) fn poll(ui: &mut Ui) -> bool {
    let mut changed = false;
    loop {
        let result = match ui.remote.edits.results.as_ref().map(Receiver::try_recv) {
            Some(Ok(result)) => result,
            Some(Err(TryRecvError::Disconnected)) => {
                ui.remote.edits = RemoteEdits::default();
                Err("Sending edits stopped".into())
            }
            Some(Err(TryRecvError::Empty)) | None => return changed,
        };
        ui.remote.edits.pending = ui.remote.edits.pending.saturating_sub(1);
        if let Err(error) = result {
            ui.error = Some(error);
        }
        // Read the copy the worker refreshed, or the one a refusal left.
        super::remote::read_soon(ui);
        changed = true;
    }
}

fn start_worker() -> (
    SyncSender<Vec<ControlCommand>>,
    Receiver<Result<(), String>>,
) {
    let (sender, edits) = sync_channel::<Vec<ControlCommand>>(QUEUED_EDITS);
    let (answer, results) = channel();
    let started = std::thread::Builder::new()
        .name("hex-remote-edits".into())
        .spawn(move || {
            for commands in edits {
                if answer.send(run(commands)).is_err() {
                    return;
                }
            }
        });
    if let Err(error) = started {
        tracing::warn!(%error, "could not start sending remote edits");
    }
    (sender, results)
}

fn run(commands: Vec<ControlCommand>) -> Result<(), String> {
    let connection = crate::remote::client::active()
        .ok_or_else(|| "Remote transcription is off, so the edit wasn't sent".to_string())?;
    for command in commands {
        match crate::remote::settings::forward(&connection, command) {
            Ok(ControlReply::Error { message, .. }) => {
                return Err(format!(
                    "{} refused the edit: {message}",
                    connection.machine_label
                ));
            }
            Ok(_) => {}
            Err(error) => return Err(format!("{error:#}")),
        }
    }
    Ok(())
}

/// The `hex` commands that turn the mirrored settings into `wanted`'s.
fn commands(mirror: &RemoteSettings, wanted: &AppSettings) -> Vec<ControlCommand> {
    let set = |key: &str, value: String| ControlCommand::SettingsSet {
        key: key.into(),
        value,
    };
    let mut commands = Vec::new();
    let (had, want) = (&mirror.dictation_processing, &wanted.dictation_processing);
    if had.default_mode != want.default_mode {
        commands.push(set(
            "dictation_processing.default_mode",
            json(&want.default_mode),
        ));
    }
    if had.modes != want.modes {
        commands.push(set("dictation_processing.modes", json(&want.modes)));
    }
    if mirror.vocabulary_boost != wanted.vocabulary_boost {
        let value = serde_json::to_value(wanted.vocabulary_boost).unwrap_or_default();
        commands.push(set(
            "vocabulary_boost",
            value.as_str().unwrap_or_default().into(),
        ));
    }
    commands.extend(vocabulary_commands(&mirror.vocabulary, &wanted.vocabulary));
    commands
}

/// Removes what went or changed, then adds what is new or changed, so a
/// respelled term ends with exactly its new spellings (an add merges).
fn vocabulary_commands(had: &[VocabularyEntry], want: &[VocabularyEntry]) -> Vec<ControlCommand> {
    let kept = |entry: &VocabularyEntry, list: &[VocabularyEntry]| list.contains(entry);
    let removes = had.iter().filter(|entry| !kept(entry, want)).map(|entry| {
        ControlCommand::VocabularyRemove {
            term: entry.term.clone(),
        }
    });
    let adds = want
        .iter()
        .filter(|entry| !kept(entry, had) && !entry.term.trim().is_empty())
        .map(|entry| ControlCommand::VocabularyAdd {
            term: entry.term.clone(),
            sounds_like: entry.sounds_like.clone(),
        });
    removes.chain(adds).collect()
}

fn json(value: &impl serde::Serialize) -> String {
    serde_json::to_string(value).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(term: &str, sounds_like: &[&str]) -> VocabularyEntry {
        VocabularyEntry {
            term: term.into(),
            sounds_like: sounds_like.iter().map(|s| (*s).into()).collect(),
        }
    }

    fn mirror(vocabulary: Vec<VocabularyEntry>) -> RemoteSettings {
        RemoteSettings {
            machine_id: "m".into(),
            dictation_processing: Default::default(),
            vocabulary,
            vocabulary_boost: Default::default(),
            transcription: AppSettings::default().transcription,
        }
    }

    #[test]
    fn the_window_edits_the_mirror_and_saves_this_macs_own() {
        let mut local = AppSettings::default();
        local.labs.remote_transcription = true;
        local.vocabulary = vec![entry("Mac only", &[])];
        let mut ui = Ui::with(local.clone(), super::super::Service::Fixture(Vec::new()));
        super::super::remote::fixture(&mut ui);
        ui.remote.mirror.as_mut().unwrap().vocabulary = vec![entry("Rune", &[])];
        ui.adopt(local.clone());
        assert_eq!(ui.settings.vocabulary, vec![entry("Rune", &[])]);
        assert_eq!(machine(&ui), Some("cshed"));

        let mut candidate = ui.settings.clone();
        candidate.vocabulary.push(entry("Tilly", &["tilley"]));
        let (saved, sent) = split(&ui, &candidate);
        assert_eq!(saved.vocabulary, local.vocabulary);
        assert_eq!(
            sent,
            vec![ControlCommand::VocabularyAdd {
                term: "Tilly".into(),
                sounds_like: vec!["tilley".into()]
            }]
        );
        assert!(ui.update(|settings| settings.vocabulary = candidate.vocabulary.clone()));
        assert_eq!(
            ui.remote.mirror.as_ref().unwrap().vocabulary,
            candidate.vocabulary
        );

        // Off again, the window shows this Mac's own list.
        local.labs.remote_transcription = false;
        ui.adopt(local.clone());
        assert_eq!(ui.settings.vocabulary, local.vocabulary);
        assert_eq!(machine(&ui), None);
    }

    #[test]
    fn an_unchanged_copy_sends_nothing() {
        let wanted = AppSettings {
            vocabulary: vec![entry("Rune", &[])],
            ..AppSettings::default()
        };
        let mut had = mirror(wanted.vocabulary.clone());
        had.dictation_processing = wanted.dictation_processing.clone();
        had.vocabulary_boost = wanted.vocabulary_boost;
        assert!(commands(&had, &wanted).is_empty());
    }

    #[test]
    fn vocabulary_edits_become_removes_then_adds() {
        let had = vec![
            entry("Rune", &[]),
            entry("Hex", &["hacks"]),
            entry("cshed", &[]),
        ];
        let want = vec![
            entry("Rune", &[]),
            entry("Hex", &["hecks"]),
            entry("Tilly", &[]),
        ];
        assert_eq!(
            vocabulary_commands(&had, &want),
            vec![
                ControlCommand::VocabularyRemove { term: "Hex".into() },
                ControlCommand::VocabularyRemove {
                    term: "cshed".into()
                },
                ControlCommand::VocabularyAdd {
                    term: "Hex".into(),
                    sounds_like: vec!["hecks".into()]
                },
                ControlCommand::VocabularyAdd {
                    term: "Tilly".into(),
                    sounds_like: vec![]
                },
            ]
        );
    }

    #[test]
    fn changed_steps_are_set_whole() {
        let mut wanted = AppSettings::default();
        wanted.dictation_processing.default_mode.transformations = vec!["lowercase".into()];
        let commands = commands(&mirror(Vec::new()), &wanted);
        let Some(ControlCommand::SettingsSet { key, value }) = commands.first() else {
            panic!("expected a set, got {commands:?}");
        };
        assert_eq!(key, "dictation_processing.default_mode");
        let mode: crate::app_settings::DictationMode = serde_json::from_str(value).unwrap();
        assert_eq!(mode, wanted.dictation_processing.default_mode);
        assert!(crate::remote::settings::edits_remote(&commands[0]));
    }
}
