//! Remote machines: another device (an iPhone, another Hex) pairs with this
//! Hex once and then sends it recordings to transcribe and clean up.
//!
//! Pairing follows T3 Code: `hex remote pair` prints a one-time code that is
//! valid for five minutes; the device trades it once for a long-lived token
//! (`POST /pair` on the remote listener). Only SHA-256 hashes of codes and
//! tokens are stored, in `remote.json` (0600) in the support directory, so
//! the CLI and the running app share it. Revoking a device removes its token.

use std::fs;
use std::path::PathBuf;

use color_eyre::eyre::{Result, WrapErr, eyre};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::events::now_ms;

/// The loopback port of the remote listener; `tailscale serve` exposes it.
pub const DEFAULT_PORT: u16 = 47831;
pub const PAIRING_CODE_LIFETIME_MS: u64 = 5 * 60 * 1000;
const CODE_ALPHABET: &[u8] = b"23456789ABCDEFGHJKLMNPQRSTUVWXYZ";
const CODE_LENGTH: usize = 12;

pub fn port() -> u16 {
    std::env::var("HEX_REMOTE_PORT")
        .ok()
        .and_then(|port| port.parse().ok())
        .unwrap_or(DEFAULT_PORT)
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Store {
    machine_id: String,
    /// Whether Hex serves paired devices; `hex remote pair` turns it on.
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    devices: Vec<Device>,
    #[serde(default)]
    pairing_codes: Vec<PairingCode>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub id: String,
    pub label: String,
    token_hash: String,
    pub paired_at_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct PairingCode {
    code_hash: String,
    expires_at_ms: u64,
}

pub struct Pairing {
    pub device: Device,
    pub token: String,
}

/// This machine's identity, created on first use.
pub fn machine_id() -> Result<String> {
    update(|_| Ok(()))?;
    Ok(load()?.machine_id)
}

/// A human name for this machine, shown on the devices that pair with it.
pub fn machine_label() -> String {
    hostname()
        .map(|name| name.trim_end_matches(".local").to_owned())
        .unwrap_or_else(|| "Hex".into())
}

/// Whether the running Hex should start the remote listener.
pub fn enabled() -> bool {
    load().is_ok_and(|store| store.enabled)
}

pub fn set_enabled(enabled: bool) -> Result<()> {
    update(|store| {
        store.enabled = enabled;
        Ok(())
    })
}

/// A new one-time code, valid for `PAIRING_CODE_LIFETIME_MS`.
pub fn issue_pairing_code() -> Result<String> {
    let code = random_code()?;
    let expires_at_ms = now_ms() + PAIRING_CODE_LIFETIME_MS;
    let code_hash = sha256(&normalize_code(&code));
    update(|store| {
        store.pairing_codes.push(PairingCode {
            code_hash: code_hash.clone(),
            expires_at_ms,
        });
        Ok(())
    })?;
    Ok(code)
}

/// Spends `code` once and returns the new device's token, or `None` for an
/// unknown or expired code.
pub fn redeem_pairing_code(code: &str, label: &str) -> Result<Option<Pairing>> {
    let code_hash = sha256(&normalize_code(code));
    let token = random_token()?;
    let device = Device {
        id: random_hex(8)?,
        label: label.trim().chars().take(64).collect(),
        token_hash: sha256(&token),
        paired_at_ms: now_ms(),
    };
    let mut redeemed = false;
    update(|store| {
        let before = store.pairing_codes.len();
        store
            .pairing_codes
            .retain(|pending| pending.code_hash != code_hash);
        redeemed = store.pairing_codes.len() < before;
        if redeemed {
            store.devices.push(device.clone());
        }
        Ok(())
    })?;
    Ok(redeemed.then_some(Pairing { device, token }))
}

/// The paired device a bearer token belongs to.
pub fn device_for_token(token: &str) -> Option<Device> {
    let token_hash = sha256(token);
    load()
        .ok()?
        .devices
        .into_iter()
        .find(|device| device.token_hash == token_hash)
}

pub fn devices() -> Result<Vec<Device>> {
    Ok(load()?.devices)
}

/// Whether a device with `id` was paired and is now revoked.
pub fn revoke(id: &str) -> Result<bool> {
    let mut removed = false;
    update(|store| {
        let before = store.devices.len();
        store.devices.retain(|device| device.id != id);
        removed = store.devices.len() < before;
        Ok(())
    })?;
    Ok(removed)
}

fn path() -> Result<PathBuf> {
    Ok(crate::app_paths::support_dir()?.join("remote.json"))
}

fn load() -> Result<Store> {
    let path = path()?;
    match fs::read(&path) {
        Ok(bytes) => serde_json::from_slice(&bytes).wrap_err("remote.json is not valid"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Store::default()),
        Err(error) => Err(error).wrap_err("could not read remote.json"),
    }
}

fn update(change: impl FnOnce(&mut Store) -> Result<()>) -> Result<()> {
    let mut store = load()?;
    if store.machine_id.is_empty() {
        store.machine_id = random_hex(16)?;
    }
    let now = now_ms();
    store.pairing_codes.retain(|code| code.expires_at_ms > now);
    change(&mut store)?;
    write_private(&path()?, &serde_json::to_vec_pretty(&store)?)
}

fn write_private(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("json.tmp");
    {
        use std::io::Write as _;
        let mut options = fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    fs::rename(&temporary, path).wrap_err("could not save remote.json")
}

/// `ABCD-EFGH-JKLM` and `abcdefghjklm` are the same code.
fn normalize_code(code: &str) -> String {
    code.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|character| character.to_ascii_uppercase())
        .collect()
}

fn random_code() -> Result<String> {
    let mut code = String::with_capacity(CODE_LENGTH);
    while code.len() < CODE_LENGTH {
        let mut byte = [0_u8; 1];
        getrandom::fill(&mut byte).map_err(|error| eyre!("no randomness: {error}"))?;
        // 32 symbols divide 256 evenly, so taking the low five bits is unbiased.
        code.push(CODE_ALPHABET[usize::from(byte[0]) % CODE_ALPHABET.len()] as char);
    }
    Ok(code)
}

fn random_token() -> Result<String> {
    Ok(format!("hexr_{}", random_hex(32)?))
}

fn random_hex(bytes: usize) -> Result<String> {
    let mut random = vec![0_u8; bytes];
    getrandom::fill(&mut random).map_err(|error| eyre!("no randomness: {error}"))?;
    Ok(to_hex(&random))
}

fn sha256(value: &str) -> String {
    to_hex(&Sha256::digest(value.as_bytes()))
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// The name people gave this machine: the macOS computer name, else the host name.
fn hostname() -> Option<String> {
    let command = if cfg!(target_os = "macos") {
        ("scutil", &["--get", "ComputerName"][..])
    } else {
        ("hostname", &[][..])
    };
    let output = std::process::Command::new(command.0)
        .args(command.1)
        .output()
        .ok()?;
    let name = String::from_utf8(output.stdout).ok()?.trim().to_owned();
    (!name.is_empty()).then_some(name)
}

/// This machine's MagicDNS name, when Tailscale runs here.
pub fn tailscale_dns_name() -> Option<String> {
    let output = std::process::Command::new("tailscale")
        .args(["status", "--json"])
        .output()
        .ok()?;
    let status: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    let name = status["Self"]["DNSName"].as_str()?.trim_end_matches('.');
    (!name.is_empty()).then(|| name.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_support_dir(test: impl FnOnce()) {
        let directory = std::env::temp_dir().join(format!("hex-remote-{}", random_hex(6).unwrap()));
        // SAFETY: tests that touch the environment run this crate's tests in their own process.
        unsafe { std::env::set_var("HEX_APPLICATION_SUPPORT_DIR", &directory) };
        test();
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn a_pairing_code_works_once_and_its_device_can_be_revoked() {
        with_support_dir(|| {
            let code = issue_pairing_code().unwrap();
            assert_eq!(code.len(), CODE_LENGTH);

            let spaced = format!("{}-{}", &code[..6], code[6..].to_lowercase());
            let pairing = redeem_pairing_code(&spaced, "iPhone").unwrap().unwrap();
            assert!(redeem_pairing_code(&code, "again").unwrap().is_none());

            let device = device_for_token(&pairing.token).unwrap();
            assert_eq!(device.label, "iPhone");
            assert!(device_for_token("hexr_wrong").is_none());

            assert!(revoke(&device.id).unwrap());
            assert!(device_for_token(&pairing.token).is_none());
        });
    }
}
