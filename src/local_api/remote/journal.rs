//! An opt-in record of every paired device's dictation, for checking the
//! speech model and the Modes against stronger ones later. With
//! `HEX_REMOTE_JOURNAL=<entries>` set, each dictation leaves an owner-only
//! folder in `<support>/remote-journal/`: `audio.wav` (16 kHz, 16-bit) and
//! `record.json` (the transcript, its windows, every rewrite's input and
//! output, the final text, timings, and the Modes and speech model used).
//! Only the newest `<entries>` folders are kept.

use std::fs;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::dictation::processor::{Processed, ProcessingObservation};
use crate::transcription::stream::Segment;

const RETENTION_ENV: &str = "HEX_REMOTE_JOURNAL";
const FOLDER: &str = "remote-journal";

/// How many dictations to keep, when the journal is on.
pub(super) fn retention() -> Option<usize> {
    std::env::var(RETENTION_ENV)
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|retention| *retention > 0)
}

/// One trip through the Modes.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Rewrite {
    /// `piece` (while recording), `rest` (after it), or `whole`.
    pub stage: &'static str,
    pub input: String,
    /// `None` when it never answered in time, so the input was kept.
    pub output: Option<String>,
    pub observation: Option<ProcessingObservation>,
    pub transformations: Vec<String>,
    pub latency_ms: u64,
}

impl Rewrite {
    pub(super) fn new(
        stage: &'static str,
        input: &str,
        processed: &Processed,
        latency_ms: u64,
    ) -> Self {
        Self {
            stage,
            input: input.to_owned(),
            output: Some(processed.text.clone()),
            observation: processed.observation.clone(),
            transformations: processed.transformations.clone(),
            latency_ms,
        }
    }
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Entry {
    /// `stream` (fed while recording) or `upload`.
    pub route: &'static str,
    pub application: String,
    pub audio_ms: u64,
    pub transcribe_ms: u64,
    pub rewrite_ms: u64,
    pub raw: String,
    pub segments: Vec<Segment>,
    pub rewrites: Vec<Rewrite>,
    pub text: String,
    pub error: Option<String>,
    #[serde(skip)]
    pub audio: Vec<f32>,
}

/// Writes `entry` when the journal is on; never fails the dictation.
pub(super) fn record(entry: Entry) {
    let Some(retention) = retention() else {
        return;
    };
    let folder = match crate::app_paths::support_dir() {
        Ok(support) => support.join(FOLDER),
        Err(error) => {
            tracing::warn!(%error, "remote journal has no folder");
            return;
        }
    };
    let spawned = std::thread::Builder::new()
        .name("remote-journal".into())
        .spawn(move || match write(&folder, &entry, retention) {
            Ok(path) => tracing::info!(path = %path.display(), "remote dictation journaled"),
            Err(error) => tracing::warn!(%error, "remote dictation not journaled"),
        });
    if let Err(error) = spawned {
        tracing::warn!(%error, "remote dictation not journaled");
    }
}

fn write(folder: &Path, entry: &Entry, retention: usize) -> std::io::Result<std::path::PathBuf> {
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(folder)?;
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let path = folder.join(format!("{millis}-{}", entry.route));
    fs::DirBuilder::new().mode(0o700).create(&path)?;
    let settings = crate::app_settings::AppSettings::load().ok();
    let record = serde_json::json!({
        "entry": entry,
        "version": env!("CARGO_PKG_VERSION"),
        "transcription": settings.as_ref().map(|settings| &settings.transcription),
        "processing": settings.as_ref().map(|settings| &settings.dictation_processing),
    });
    let json = serde_json::to_vec_pretty(&record).map_err(std::io::Error::other)?;
    std::io::Write::write_all(&mut owner_only(&path.join("record.json"))?, &json)?;
    if !entry.audio.is_empty() {
        write_wav(&path.join("audio.wav"), &entry.audio)?;
    }
    rotate(folder, retention)?;
    Ok(path)
}

fn owner_only(path: &Path) -> std::io::Result<fs::File> {
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
}

fn write_wav(path: &Path, samples: &[f32]) -> std::io::Result<()> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: crate::transcription::stream::SAMPLE_RATE as u32,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let file = std::io::BufWriter::new(owner_only(path)?);
    let mut writer = hound::WavWriter::new(file, spec).map_err(std::io::Error::other)?;
    for sample in samples {
        let sample = (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16;
        writer.write_sample(sample).map_err(std::io::Error::other)?;
    }
    writer.finalize().map_err(std::io::Error::other)
}

/// Removes all but the newest `retention` dictations.
fn rotate(folder: &Path, retention: usize) -> std::io::Result<()> {
    let mut entries = fs::read_dir(folder)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    entries.sort();
    let remove = entries.len().saturating_sub(retention);
    for path in entries.into_iter().take(remove) {
        fs::remove_dir_all(path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_journaled_dictation_is_owner_only_and_bounded() {
        use std::os::unix::fs::PermissionsExt;
        let folder =
            std::env::temp_dir().join(format!("hex-remote-journal-{}", std::process::id()));
        let _ = fs::remove_dir_all(&folder);
        let entry = |route| Entry {
            route,
            raw: "heard".into(),
            text: "Heard.".into(),
            audio: vec![0.25; 1_600],
            ..Entry::default()
        };
        let first = write(&folder, &entry("stream"), 1).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(2));
        let second = write(&folder, &entry("upload"), 1).unwrap();

        assert!(!first.exists());
        let record: serde_json::Value =
            serde_json::from_slice(&fs::read(second.join("record.json")).unwrap()).unwrap();
        assert_eq!(record["entry"]["raw"], "heard");
        assert_eq!(record["entry"]["text"], "Heard.");
        let audio = hound::WavReader::open(second.join("audio.wav")).unwrap();
        assert_eq!(audio.duration(), 1_600);
        let mode = |path: &Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&second), 0o700);
        assert_eq!(mode(&second.join("audio.wav")), 0o600);
        fs::remove_dir_all(&folder).unwrap();
    }
}
