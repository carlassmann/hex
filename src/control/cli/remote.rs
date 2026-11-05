//! `hex remote`: pair devices with this Hex, and connect this Hex to another.

use clap::Subcommand;

use super::EXIT_REFUSED;

#[derive(Subcommand, Debug)]
pub enum RemoteCli {
    /// Print a one-time code a device can pair with for five minutes.
    Pair,
    /// List the paired devices.
    Devices,
    /// Unpair a device.
    Revoke { id: String },
    /// Stop serving paired devices (after Hex restarts).
    Disable,
    /// Pair this Hex with another one, so Labs › Remote transcription can send it dictations.
    Connect {
        /// The address the other Hex's `hex remote pair` printed, such as host.ts.net:47831.
        address: String,
        /// Its one-time code.
        code: String,
    },
    /// Forget the Hex this one is connected to.
    Disconnect,
    /// Send a WAV to the connected Hex and print what it made of it.
    Send {
        wav: std::path::PathBuf,
        /// The app the dictation is for, so the other Hex's Modes for it apply.
        #[arg(long)]
        app: Option<String>,
    },
}

pub(super) fn run(command: RemoteCli) -> u8 {
    match remote_command(command) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("{error:#}");
            EXIT_REFUSED
        }
    }
}

fn remote_command(command: RemoteCli) -> color_eyre::Result<u8> {
    use crate::remote::pairing;
    match command {
        RemoteCli::Pair => {
            let was_enabled = pairing::enabled();
            pairing::set_enabled(true)?;
            let code = pairing::issue_pairing_code()?;
            let port = pairing::port();
            let host = pairing::tailscale_dns_name().unwrap_or_else(pairing::machine_label);
            let link = format!("hex-dictation://pair?host={host}:{port}&code={code}");
            println!(
                "Scan with Hex on your iPhone to pair with {}:",
                pairing::machine_label()
            );
            println!();
            println!("{}", pairing_qr(&link)?);
            println!("Or enter:");
            println!("  Address  {host}:{port}");
            println!("  Code     {}-{}", &code[..6], &code[6..]);
            println!();
            println!("The code works once, for five minutes.");
            println!("{link}");
            if !was_enabled {
                println!();
                println!("Restart Hex to start serving devices (hex restart).");
            }
            println!(
                "Expose it in your tailnet once with: tailscale serve --bg --https={port} http://127.0.0.1:{port}"
            );
            Ok(0)
        }
        RemoteCli::Devices => {
            let devices = pairing::devices()?;
            if devices.is_empty() {
                println!("No paired devices.");
            }
            for device in devices {
                println!("{}  {}", device.id, device.label);
            }
            Ok(0)
        }
        RemoteCli::Revoke { id } => {
            if pairing::revoke(&id)? {
                println!("Unpaired {id}.");
                Ok(0)
            } else {
                eprintln!("No paired device {id}.");
                Ok(EXIT_REFUSED)
            }
        }
        RemoteCli::Disable => {
            pairing::set_enabled(false)?;
            println!("Remote dictation turns off when Hex restarts.");
            Ok(0)
        }
        RemoteCli::Connect { address, code } => {
            let url = if address.contains("://") {
                address
            } else {
                format!("https://{address}")
            };
            let connection = crate::remote::client::connect(&url, &code)?;
            println!(
                "Connected to {} ({}).",
                connection.machine_label, connection.url
            );
            println!("Turn on Labs › Remote transcription to send it your dictations.");
            Ok(0)
        }
        RemoteCli::Disconnect => {
            if crate::remote::client::disconnect()? {
                println!("Disconnected. The other Hex lists this one until you revoke it there.");
            } else {
                println!("Not connected.");
            }
            Ok(0)
        }
        RemoteCli::Send { wav, app } => {
            let connection = crate::remote::client::connection()?
                .ok_or_else(|| color_eyre::eyre::eyre!("Not connected; run hex remote connect"))?;
            let wav = std::fs::read(&wav)?;
            let dictated = crate::remote::client::dictate_wav(&connection, &wav, app.as_deref())?;
            println!("{}", dictated.text);
            eprintln!("\nHeard: {}", dictated.raw);
            Ok(0)
        }
    }
}

/// Half-height blocks keep the code small enough for a terminal; dark modules print as
/// light so phones read it on the usual dark terminal background.
fn pairing_qr(link: &str) -> color_eyre::Result<String> {
    use qrcode::render::unicode::Dense1x2;
    Ok(qrcode::QrCode::new(link)?
        .render::<Dense1x2>()
        .dark_color(Dense1x2::Light)
        .light_color(Dense1x2::Dark)
        .quiet_zone(true)
        .build())
}
