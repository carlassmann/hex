//! This Hex as a paired device of another Hex (`remote::pairing` on that
//! machine): `hex remote connect` trades the other machine's one-time code
//! for a device token, kept in `remote-client.json` (0600) in the support
//! directory, never in `settings.json`. With Labs › Remote transcription on,
//! the dictation worker sends each finished clip to that machine's
//! `POST /remote/dictations`, which transcribes it with its model and runs
//! its Modes, so every device shares one model and one pipeline.

use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use color_eyre::eyre::{Result, WrapErr, eyre};
use serde::{Deserialize, Serialize};

const FILE: &str = "remote-client.json";
/// The other machine transcribes, then waits up to its Modes' deadline;
/// a long dictation gets [`DICTATION_DEADLINE_PER_MINUTE`] more per minute.
const DICTATION_DEADLINE: Duration = Duration::from_secs(30);
const DICTATION_DEADLINE_PER_MINUTE: Duration = Duration::from_secs(10);
/// 16 kHz, 16-bit mono.
const WAV_BYTES_PER_MINUTE: usize = 60 * 32_000;
const PAIR_DEADLINE: Duration = Duration::from_secs(15);

/// The machine this Hex sends its dictations to.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Connection {
    pub url: String,
    token: String,
    pub device_id: String,
    pub machine_id: String,
    pub machine_label: String,
}

impl Connection {
    pub(crate) fn token(&self) -> &str {
        &self.token
    }
}

#[derive(Serialize)]
struct PairRequest<'a> {
    code: &'a str,
    label: &'a str,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PairResponse {
    token: String,
    device_id: String,
    machine_id: String,
    machine_label: String,
}

/// What the other machine made of a dictation: the last line of its
/// progress answer (`stage: done`).
#[derive(Debug, Deserialize)]
pub struct Dictated {
    /// Processed by its Modes.
    pub text: String,
    /// As its model heard it.
    pub raw: String,
}

/// Pairs with the Hex at `url` using the code its `hex remote pair` printed.
pub fn connect(url: &str, code: &str) -> Result<Connection> {
    let url = url.trim_end_matches('/').to_owned();
    let code: String = code.chars().filter(char::is_ascii_alphanumeric).collect();
    let label = crate::remote::pairing::machine_label();
    let agent = crate::net::agent(&url, |config| {
        config
            .http_status_as_error(false)
            .timeout_global(Some(PAIR_DEADLINE))
    });
    let mut response = agent
        .post(format!("{url}/pair"))
        .header("Content-Type", "application/json")
        .send(
            &serde_json::to_vec(&PairRequest {
                code: &code,
                label: &label,
            })?[..],
        )
        .map_err(|error| eyre!(crate::net::unreachable(&url, &error)))?;
    if response.status() != 200 {
        return Err(eyre!(
            "{url} refused the code ({}): {}",
            response.status(),
            response.body_mut().read_to_string().unwrap_or_default()
        ));
    }
    let paired: PairResponse = serde_json::from_str(&response.body_mut().read_to_string()?)
        .wrap_err("the other Hex answered the pairing oddly")?;
    let connection = Connection {
        url,
        token: paired.token,
        device_id: paired.device_id,
        machine_id: paired.machine_id,
        machine_label: paired.machine_label,
    };
    crate::remote::pairing::write_private(&path()?, &serde_json::to_vec_pretty(&connection)?)?;
    // Its settings shape this Mac's fallback; a later refresh retries.
    match crate::remote::settings::refresh(&connection) {
        Ok(_) => crate::remote::settings::reapply(),
        Err(error) => {
            tracing::warn!(error = %format!("{error:#}"), "could not read the remote Hex's settings")
        }
    }
    Ok(connection)
}

/// The paired machine, if any.
pub fn connection() -> Result<Option<Connection>> {
    match fs::read(path()?) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .wrap_err("remote-client.json is not valid"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).wrap_err("could not read remote-client.json"),
    }
}

/// Forgets the paired machine; it keeps listing this device until revoked there.
pub fn disconnect() -> Result<bool> {
    crate::remote::settings::forget();
    let removed = fs::remove_file(path()?);
    crate::remote::settings::reapply();
    match removed {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error).wrap_err("could not remove remote-client.json"),
    }
}

/// The machine dictations go to now: Labs › Remote transcription is on and
/// this Hex is connected.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn active() -> Option<Connection> {
    if !crate::labs::remote_transcription() {
        return None;
    }
    connection().unwrap_or_else(|error| {
        tracing::warn!(error = %format!("{error:#}"), "remote transcription has no connection");
        None
    })
}

/// Sends 16 kHz mono `samples` to the paired machine as if dictated into
/// `application` there, so its Modes for that app apply.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn dictate(
    connection: &Connection,
    samples: &[f32],
    application: Option<&str>,
) -> Result<Dictated> {
    dictate_wav(connection, &wav(samples)?, application)
}

/// Like [`dictate`], for a WAV file's bytes in any format the other Hex reads.
pub fn dictate_wav(
    connection: &Connection,
    wav: &[u8],
    application: Option<&str>,
) -> Result<Dictated> {
    let started = Instant::now();
    // The progress answer runs the same pipeline as a phone's dictation,
    // journal included.
    let url = format!("{}/remote/dictations?progress=1", connection.url);
    let minutes = (wav.len() / WAV_BYTES_PER_MINUTE) as u32;
    let deadline = DICTATION_DEADLINE + DICTATION_DEADLINE_PER_MINUTE * minutes;
    let agent = crate::net::agent(&url, |config| {
        config
            .http_status_as_error(false)
            .timeout_global(Some(deadline))
    });
    let mut request = agent
        .post(&url)
        .header("Authorization", format!("Bearer {}", connection.token))
        .header("Content-Type", "audio/wav");
    if let Some(application) = application.filter(|name| is_header_safe(name)) {
        request = request.header("X-Hex-Application", application);
    }
    let mut response = request
        .send(wav)
        .map_err(|error| eyre!(crate::net::unreachable(&url, &error)))?;
    let status = response.status();
    if status != 200 {
        let body = response.body_mut().read_to_string().unwrap_or_default();
        return Err(eyre!(
            "{} answered {status}: {body}",
            connection.machine_label
        ));
    }
    let dictated = last_progress_line(&response.body_mut().read_to_string()?)
        .map_err(|reason| eyre!("{}: {reason}", connection.machine_label))?;
    tracing::info!(
        machine = connection.machine_label,
        upload_bytes = wav.len(),
        latency_ms = started.elapsed().as_millis() as u64,
        "remote dictation transcribed"
    );
    Ok(dictated)
}

/// After a timed transcription fails, meetings and Annotate stay on this Mac
/// for this long, so a call without the other Hex doesn't wait on it each chunk.
const TIMED_BACKOFF: Duration = Duration::from_secs(60);
const TIMED_DEADLINE: Duration = Duration::from_secs(30);
const TIMED_CONNECT_DEADLINE: Duration = Duration::from_secs(5);
static TIMED_FAILED_AT: std::sync::Mutex<Option<Instant>> = std::sync::Mutex::new(None);

/// The machine meeting chunks and Annotate words go to now: [`active`],
/// unless a timed transcription failed in the last [`TIMED_BACKOFF`].
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn timed_active() -> Option<Connection> {
    let failed_at = *TIMED_FAILED_AT
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    if failed_at.is_some_and(|at| at.elapsed() < TIMED_BACKOFF) {
        return None;
    }
    active()
}

/// The other machine's raw transcript of 16 kHz mono `samples`, with segment
/// and word times and none of its Modes. A failure starts [`TIMED_BACKOFF`].
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn transcribe_timed(
    connection: &Connection,
    samples: &[f32],
) -> Result<crate::transcription::transcript::Transcript> {
    let result = post_timed(connection, samples);
    if let Err(error) = &result {
        *TIMED_FAILED_AT
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(Instant::now());
        tracing::warn!(
            machine = connection.machine_label,
            error = %format!("{error:#}"),
            "remote timed transcription failed; this Mac transcribes for a minute"
        );
    }
    result
}

fn post_timed(
    connection: &Connection,
    samples: &[f32],
) -> Result<crate::transcription::transcript::Transcript> {
    let started = Instant::now();
    let url = format!("{}/remote/transcriptions", connection.url);
    let agent = crate::net::agent(&url, |config| {
        config
            .http_status_as_error(false)
            .timeout_connect(Some(TIMED_CONNECT_DEADLINE))
            .timeout_global(Some(TIMED_DEADLINE))
    });
    let wav = wav(samples)?;
    let mut response = agent
        .post(&url)
        .header("Authorization", format!("Bearer {}", connection.token))
        .header("Content-Type", "audio/wav")
        .send(&wav[..])
        .map_err(|error| eyre!(crate::net::unreachable(&url, &error)))?;
    let status = response.status();
    let body = response.body_mut().read_to_string()?;
    if status != 200 {
        return Err(eyre!(
            "{} answered {status}: {body}",
            connection.machine_label
        ));
    }
    let transcript = serde_json::from_str(&body).wrap_err_with(|| {
        format!(
            "{} answered the transcription oddly",
            connection.machine_label
        )
    })?;
    tracing::debug!(
        machine = connection.machine_label,
        audio_ms = samples.len() as u64 * 1000 / u64::from(SAMPLE_RATE),
        latency_ms = started.elapsed().as_millis() as u64,
        "remote timed transcription"
    );
    Ok(transcript)
}

/// The `done` line's text, or why the other Hex failed the dictation.
fn last_progress_line(body: &str) -> std::result::Result<Dictated, String> {
    let line = body
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .ok_or("the answer ended before the text")?;
    let value: serde_json::Value =
        serde_json::from_str(line).map_err(|_| "the answer is not progress lines")?;
    match value.get("stage").and_then(serde_json::Value::as_str) {
        Some("done") => serde_json::from_value(value).map_err(|_| "the text is missing".into()),
        Some("failed") => Err(value
            .get("message")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("it failed the dictation")
            .to_owned()),
        _ => Err("the answer ended before the text".into()),
    }
}

const SAMPLE_RATE: u32 = 16_000;

#[cfg_attr(not(any(target_os = "macos", test)), allow(dead_code))]
fn wav(samples: &[f32]) -> Result<Vec<u8>> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut bytes = std::io::Cursor::new(Vec::with_capacity(44 + samples.len() * 2));
    let mut writer = hound::WavWriter::new(&mut bytes, spec)?;
    for sample in samples {
        writer.write_sample((sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16)?;
    }
    writer.finalize()?;
    Ok(bytes.into_inner())
}

/// An app name that can travel as an HTTP header value.
fn is_header_safe(name: &str) -> bool {
    !name.is_empty() && name.len() <= 256 && name.chars().all(|c| c == ' ' || c.is_ascii_graphic())
}

fn path() -> Result<PathBuf> {
    Ok(crate::app_paths::support_dir()?.join(FILE))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_clip_becomes_a_16_khz_mono_wav() {
        let wav = wav(&[0.0, 0.5, -1.0, 2.0]).unwrap();
        let reader = hound::WavReader::new(std::io::Cursor::new(wav)).unwrap();
        assert_eq!(reader.spec().sample_rate, 16_000);
        assert_eq!(reader.spec().channels, 1);
        let samples: Vec<i16> = reader.into_samples().map(Result::unwrap).collect();
        assert_eq!(samples, vec![0, 16383, -32767, 32767]);
    }

    #[test]
    fn the_done_line_carries_the_text() {
        let body = concat!(
            "{\"stage\":\"received\",\"durationMs\":3000}\n",
            "{\"stage\":\"rewriting\",\"elapsedMs\":900}\n",
            "{\"stage\":\"done\",\"text\":\"Hello.\",\"raw\":\"hello\",\"durationMs\":3000}\n",
        );
        let dictated = last_progress_line(body).unwrap();
        assert_eq!(
            (dictated.text.as_str(), dictated.raw.as_str()),
            ("Hello.", "hello")
        );
    }

    #[test]
    fn a_failed_or_cut_off_answer_is_an_error() {
        let failed =
            "{\"stage\":\"failed\",\"code\":\"no-speech\",\"message\":\"heard no speech\"}\n";
        assert_eq!(last_progress_line(failed).unwrap_err(), "heard no speech");
        let cut = "{\"stage\":\"rewriting\",\"elapsedMs\":900}\n";
        assert!(last_progress_line(cut).is_err());
        assert!(last_progress_line("").is_err());
    }

    #[test]
    fn only_plain_app_names_travel_as_a_header() {
        assert!(is_header_safe("Slack"));
        assert!(is_header_safe("Visual Studio Code"));
        assert!(!is_header_safe("Café"));
        assert!(!is_header_safe("bad\r\nheader"));
        assert!(!is_header_safe(""));
    }
}
