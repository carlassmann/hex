//! Remote machines in Labs. A Mac with Labs › Remote transcription on
//! connects to another Hex here (`remote::client`): address and one-time
//! code, or the pair link pasted whole. Every Hex can serve others
//! (`remote::pairing`): Pair a device shows a one-time code with its QR for
//! Hex on an iPhone or another Mac, and Paired devices lists what it serves,
//! each with Revoke. Files are read and written only by a live window;
//! connecting and looking up the Tailscale name run off the UI thread.

use std::sync::mpsc::{Receiver, TryRecvError, channel};

use rune::{El, TextEdit, Waker, col, edit_access, el, row, text};
use rune_kit::TextInput;

use super::Ui;
use super::controls::{self, ButtonKind, CardRow, Paint, button, card};
use crate::desktop::i18n::{tr, tr_fill, tr_named};
use crate::remote::client::Connection;
use crate::remote::pairing::{self, Device};
use crate::remote::settings::RemoteSettings;

/// How often a shown Labs page reads the paired devices again, so a phone
/// that pairs while it shows appears.
const DEVICES_REFRESH_SECONDS: f64 = 3.0;
const QR_MODULE: f32 = 4.0;
const PAIR_LINK_SCHEME: &str = "hex-dictation://pair?";

#[derive(Default)]
pub(super) struct RemoteState {
    address: TextEdit,
    code: TextEdit,
    pub(super) connection: Option<Connection>,
    connecting: Option<Receiver<Result<Connection, String>>>,
    error: Option<String>,
    pub(super) devices: Vec<Device>,
    /// The connected machine's settings this Mac's fallback uses.
    pub(super) mirror: Option<RemoteSettings>,
    pub(super) pairing: Option<Pairing>,
    /// This machine's Tailscale name, looked up for the pair link.
    host_lookup: Option<Receiver<String>>,
    /// The poll clock the files were last read at; `None` before the first.
    read_at: Option<f64>,
    pub(super) edits: super::remote_edit::RemoteEdits,
}

/// A code on show for a device to pair with.
pub(super) struct Pairing {
    pub(super) code: String,
    pub(super) host: Option<String>,
    /// The listener was off, so it starts serving only after a restart.
    pub(super) needs_restart: bool,
}

impl Pairing {
    fn address(&self) -> String {
        let host = self.host.clone().unwrap_or_else(pairing::machine_label);
        format!("{host}:{}", pairing::port())
    }

    fn link(&self) -> String {
        format!(
            "{PAIR_LINK_SCHEME}host={}&code={}",
            self.address(),
            self.code
        )
    }

    fn spaced_code(&self) -> String {
        let (first, second) = self.code.split_at(self.code.len() / 2);
        format!("{first}-{second}")
    }
}

/// Reads the connection and devices while Labs shows or the lab is on (the
/// window then shows the paired Hex's steps), and takes the workers'
/// answers. True when what the window shows changed.
pub(super) fn poll(ui: &mut Ui, clock: f64) -> bool {
    let mut changed = super::remote_edit::poll(ui);
    if (ui.labs_open || ui.settings.labs.remote_transcription)
        && ui.live()
        && ui
            .remote
            .read_at
            .is_none_or(|at| clock >= at + DEVICES_REFRESH_SECONDS)
    {
        ui.remote.read_at = Some(clock);
        changed |= read_files(ui);
    }
    match ui.remote.connecting.as_ref().map(Receiver::try_recv) {
        Some(Ok(result)) => {
            ui.remote.connecting = None;
            match result {
                Ok(connection) => {
                    ui.remote.connection = Some(connection);
                    ui.remote.address = TextEdit::default();
                    ui.remote.code = TextEdit::default();
                    ui.remote.error = None;
                }
                Err(error) => ui.remote.error = Some(error),
            }
            changed = true;
        }
        Some(Err(TryRecvError::Disconnected)) => {
            ui.remote.connecting = None;
            ui.remote.error = Some(tr("Connecting stopped.").into());
            changed = true;
        }
        Some(Err(TryRecvError::Empty)) | None => {}
    }
    if let Some(Ok(host)) = ui.remote.host_lookup.as_ref().map(Receiver::try_recv) {
        ui.remote.host_lookup = None;
        if let Some(pairing) = &mut ui.remote.pairing {
            pairing.host = Some(host);
        }
        changed = true;
    }
    changed
}

fn read_files(ui: &mut Ui) -> bool {
    let connection = crate::remote::client::connection().unwrap_or_else(|error| {
        tracing::warn!(error = %format!("{error:#}"), "could not read the remote connection");
        None
    });
    let devices = pairing::devices().unwrap_or_default();
    // The copy is stale until the edits being sent are answered.
    let mirror = if ui.remote.edits.sending() {
        ui.remote.mirror.clone()
    } else {
        crate::remote::settings::mirrored()
    };
    let shown = connection.as_ref().map(|c| (&c.url, &c.device_id))
        != ui
            .remote
            .connection
            .as_ref()
            .map(|c| (&c.url, &c.device_id))
        || mirror != ui.remote.mirror;
    let changed = shown || device_ids(&devices) != device_ids(&ui.remote.devices);
    ui.remote.connection = connection;
    ui.remote.devices = devices;
    ui.remote.mirror = mirror;
    if shown {
        super::remote_edit::reshow(ui);
    }
    changed
}

/// Reads the files again at the next poll.
pub(super) fn read_soon(ui: &mut Ui) {
    ui.remote.read_at = None;
}

fn device_ids(devices: &[Device]) -> Vec<&str> {
    devices.iter().map(|device| device.id.as_str()).collect()
}

/// Under Labs › Remote transcription while it is on: the machine this Mac
/// sends to, or the fields to connect to one.
pub(super) fn connection(ui: &Ui, p: Paint) -> El {
    let state = &ui.remote;
    if let Some(connection) = &state.connection {
        let disconnect = button(
            p,
            "remote-disconnect",
            tr("Disconnect"),
            ButtonKind::Secondary,
            |ui: &mut Ui, _| disconnect(ui),
        );
        return controls::described_row(
            p,
            "lab-remote-connection",
            &tr_fill("Sends to {}", &connection.machine_label),
            Some(fallback_note(ui, p, connection)),
            disconnect,
        );
    }
    let address = TextInput::new(&state.address)
        .placeholder(tr("Address or pair link"))
        .width(Some(220.0))
        .on_edit(edit_access(|ui: &mut Ui| &mut ui.remote.address))
        .on_submit(|ui: &mut Ui, _| connect(ui))
        .build(p.k)
        .key("remote-address");
    let code = TextInput::new(&state.code)
        .placeholder(tr("Code"))
        .width(Some(130.0))
        .on_edit(edit_access(|ui: &mut Ui| &mut ui.remote.code))
        .on_submit(|ui: &mut Ui, _| connect(ui))
        .build(p.k)
        .key("remote-code");
    let action = if state.connecting.is_some() {
        text(tr("Connecting…"))
            .key("remote-connecting")
            .font(p.k.ty.ui)
            .color(p.muted())
    } else {
        button(
            p,
            "remote-connect",
            tr("Connect"),
            ButtonKind::Secondary,
            |ui: &mut Ui, _| connect(ui),
        )
    };
    let fields = row()
        .key("remote-fields")
        .gap(8.0)
        .center_y()
        .child(address)
        .child(code)
        .child(action);
    let description = match &state.error {
        Some(error) => (error.clone(), p.danger()),
        None => (
            tr("On the other Hex, open Labs and choose Pair a device").to_owned(),
            p.muted(),
        ),
    };
    controls::described_row(
        p,
        "lab-remote-connection",
        tr("Connect to"),
        Some(description),
        fields,
    )
}

/// The address, then how a fallback on this Mac compares: it uses the
/// mirrored Output steps and vocabulary, and says when the models differ.
fn fallback_note(ui: &Ui, p: Paint, connection: &Connection) -> (String, rune::Color) {
    let Some(mirror) = &ui.remote.mirror else {
        return (
            format!(
                "{}\n{}",
                connection.url,
                tr("Its Output steps aren't copied yet, so a fallback here uses this Mac's own")
            ),
            p.muted(),
        );
    };
    let same_steps =
        tr("When it can't be reached, this Mac uses a copy of its Output steps and vocabulary");
    let ours = &ui.settings.transcription;
    if mirror.transcription == *ours {
        return (format!("{}\n{same_steps}", connection.url), p.muted());
    }
    let name = |selection: &crate::transcription::models::TranscriptionSelection| {
        format!(
            "{} ({})",
            crate::transcription::models::definition(selection.model).name,
            selection.language
        )
    };
    let differs = tr_named(
        "It transcribes with {theirs}, this Mac with {ours}. Choose the same model in General for matching text",
        &[
            ("theirs", &name(&mirror.transcription)),
            ("ours", &name(ours)),
        ],
    );
    (
        format!("{}\n{same_steps}\n{differs}", connection.url),
        p.warning(),
    )
}

fn connect(ui: &mut Ui) {
    if ui.remote.connecting.is_some() || !ui.live() {
        return;
    }
    let Some((url, code)) = target(&ui.remote.address.text, &ui.remote.code.text) else {
        ui.remote.error = Some(tr("Enter the other Hex's address and code.").into());
        return;
    };
    let (sender, receiver) = channel();
    let wake = wake(ui.modes.waker.clone());
    let started = std::thread::Builder::new()
        .name("hex-remote-connect".into())
        .spawn(move || {
            let result =
                crate::remote::client::connect(&url, &code).map_err(|error| format!("{error:#}"));
            let _ = sender.send(result);
            wake();
        });
    match started {
        Ok(_) => {
            ui.remote.connecting = Some(receiver);
            ui.remote.error = None;
        }
        Err(error) => ui.remote.error = Some(error.to_string()),
    }
}

/// The URL and code to connect with: a pasted pair link carries both;
/// a bare address gets `https://`.
fn target(address: &str, code: &str) -> Option<(String, String)> {
    let address = address.trim();
    let (address, code) = match address.strip_prefix(PAIR_LINK_SCHEME) {
        Some(query) => {
            let field = |name: &str| {
                query
                    .split('&')
                    .find_map(|pair| pair.strip_prefix(name)?.strip_prefix('='))
                    .map(str::to_owned)
            };
            (field("host")?, field("code")?)
        }
        None => (address.to_owned(), code.trim().to_owned()),
    };
    if address.is_empty() || code.is_empty() {
        return None;
    }
    let url = if address.contains("://") {
        address
    } else {
        format!("https://{address}")
    };
    Some((url, code))
}

fn disconnect(ui: &mut Ui) {
    if !ui.live() {
        return;
    }
    match crate::remote::client::disconnect() {
        Ok(_) => ui.remote.connection = None,
        Err(error) => ui.remote.error = Some(format!("{error:#}")),
    }
}

/// Output's note while Labs › Remote transcription sends dictations away:
/// the pane shows and edits the other Hex's steps (`remote_edit`).
pub(super) fn output_note(ui: &Ui, p: Paint) -> Option<El> {
    let machine = super::remote_edit::machine(ui)?;
    let note = controls::icon_row(
        p,
        "remote-output-note",
        &super::icons::WAVE,
        &tr_fill("Editing {}'s steps", machine),
        Some(tr(
            "Remote transcription is on, so dictations use the steps and vocabulary on that Hex. Changes here, Edit with agent, and hex settings on this Mac change them there",
        )),
        el(),
    );
    Some(card(p, "remote-output", vec![CardRow::from(note)]))
}

/// Remote devices: pairing a new one and the ones this Hex serves.
pub(super) fn devices_section(ui: &Ui, p: Paint) -> El {
    let state = &ui.remote;
    let mut rows: Vec<CardRow> = Vec::new();
    match &state.pairing {
        Some(pairing) => rows.push(pairing_row(p, pairing)),
        None => {
            let pair = button(
                p,
                "remote-pair",
                tr("Pair a device"),
                ButtonKind::Secondary,
                |ui: &mut Ui, _| pair(ui),
            );
            rows.push(
                controls::setting_row(
                    p,
                    "remote-pair-row",
                    tr("Transcribe for other devices"),
                    Some(tr(
                        "Hex on your iPhone or another Mac sends its dictations here, and this Hex transcribes them and runs its Output steps",
                    )),
                    pair,
                )
                .into(),
            );
        }
    }
    for device in &state.devices {
        let id = device.id.clone();
        let revoke = button(
            p,
            "revoke",
            tr("Revoke"),
            ButtonKind::Secondary,
            move |ui: &mut Ui, _| revoke(ui, &id),
        );
        rows.push(
            controls::setting_row(
                p,
                &format!("remote-device-{}", device.id),
                &device.label,
                None,
                revoke,
            )
            .into(),
        );
    }
    col()
        .key("remote-devices")
        .gap(8.0)
        .child(p.section_label(tr("Remote devices")))
        .child(card(p, "remote-devices-card", rows))
}

fn pairing_row(p: Paint, pairing: &Pairing) -> CardRow {
    let mut about = vec![
        tr_fill("Address {}", &pairing.address()),
        tr_fill("Code {}", &pairing.spaced_code()),
        tr("Scan with Hex on your iPhone, or enter both on the other Mac. The code works once, for five minutes.").to_owned(),
    ];
    if pairing.needs_restart {
        about.push(tr("Restart Hex to start serving devices.").to_owned());
    }
    let details = col().key("pairing-details").gap(4.0).grow(1.0).children(
        about.into_iter().enumerate().map(|(index, line)| {
            text(line)
                .key(format!("pairing-line-{index}"))
                .font(p.k.ty.ui)
                .color(if index < 2 { p.text() } else { p.muted() })
                .selectable(true)
                .wrap()
        }),
    );
    let done = button(
        p,
        "remote-pair-done",
        tr("Done"),
        ButtonKind::Secondary,
        |ui: &mut Ui, _| ui.remote.pairing = None,
    );
    row()
        .key("remote-pairing")
        .gap(16.0)
        .px(controls::METRICS.row_px)
        .py(12.0)
        .child(qr(p, &pairing.link()))
        .child(details)
        .child(done)
        .into()
}

/// The pair link as a QR code, one box per run of dark modules, dark on a
/// light plate in either appearance so cameras read it.
fn qr(p: Paint, link: &str) -> El {
    let Ok(code) = qrcode::QrCode::new(link) else {
        return el().key("qr");
    };
    let width = code.width();
    let colors = code.to_colors();
    const QUIET: usize = 2;
    let side = (width + 2 * QUIET) as f32 * QR_MODULE;
    let mut plate = col()
        .key("qr")
        .w(side)
        .h(side)
        .fill(p.k.c.white)
        .radius(p.k.radius.sm)
        .pt(QUIET as f32 * QR_MODULE)
        .pl(QUIET as f32 * QR_MODULE);
    for (y, line) in colors.chunks(width).enumerate() {
        let mut modules = row().key(format!("qr-{y}")).h(QR_MODULE);
        let mut x = 0;
        while x < width {
            let dark = line[x] == qrcode::Color::Dark;
            let run = line[x..]
                .iter()
                .take_while(|color| (**color == qrcode::Color::Dark) == dark)
                .count();
            let mut module = el()
                .key(format!("m-{x}"))
                .w(run as f32 * QR_MODULE)
                .h(QR_MODULE);
            if dark {
                module = module.fill(p.k.c.black);
            }
            modules = modules.child(module);
            x += run;
        }
        plate = plate.child(modules);
    }
    plate
}

fn pair(ui: &mut Ui) {
    if !ui.live() {
        return;
    }
    let needs_restart = !pairing::enabled();
    let code = pairing::set_enabled(true).and_then(|()| pairing::issue_pairing_code());
    let code = match code {
        Ok(code) => code,
        Err(error) => {
            ui.error = Some(format!("{error:#}"));
            return;
        }
    };
    ui.remote.pairing = Some(Pairing {
        code,
        host: None,
        needs_restart,
    });
    let (sender, receiver) = channel();
    let wake = wake(ui.modes.waker.clone());
    let started = std::thread::Builder::new()
        .name("hex-tailscale-name".into())
        .spawn(move || {
            if let Some(host) = pairing::tailscale_dns_name() {
                let _ = sender.send(host);
                wake();
            }
        });
    if started.is_ok() {
        ui.remote.host_lookup = Some(receiver);
    }
}

fn revoke(ui: &mut Ui, id: &str) {
    if !ui.live() {
        return;
    }
    match pairing::revoke(id) {
        Ok(_) => ui.remote.devices.retain(|device| device.id != id),
        Err(error) => ui.error = Some(format!("{error:#}")),
    }
}

fn wake(waker: Option<Waker>) -> impl Fn() + Send + 'static {
    move || {
        if let Some(waker) = &waker {
            waker();
        }
    }
}

/// The `mac-labs-remote` scene: connected, two paired devices, and a code
/// on show.
pub(super) fn fixture(ui: &mut Ui) {
    ui.remote.connection = serde_json::from_value(serde_json::json!({
        "url": "https://cshed.example.ts.net:47831",
        "token": "",
        "deviceId": "4f4d8a54278941b5",
        "machineId": "0",
        "machineLabel": "cshed",
    }))
    .ok();
    ui.remote.mirror = Some(RemoteSettings {
        machine_id: "0".into(),
        dictation_processing: Default::default(),
        vocabulary: Vec::new(),
        vocabulary_boost: Default::default(),
        transcription: crate::transcription::models::TranscriptionSelection {
            model: crate::transcription::models::TranscriptionModelId::ParakeetV3,
            language: "de".into(),
        },
    });
    ui.remote.devices = [
        ("8a551cde1e2549a0", "iPhone"),
        ("4f4d8a54278941b5", "cbook"),
    ]
    .into_iter()
    .filter_map(|(id, label)| {
        serde_json::from_value(serde_json::json!({
            "id": id, "label": label, "tokenHash": "", "pairedAtMs": 0,
        }))
        .ok()
    })
    .collect();
    ui.remote.pairing = Some(Pairing {
        code: "K7M2Q9X4H8PB".into(),
        host: Some("cshed.example.ts.net".into()),
        needs_restart: false,
    });
}

#[cfg(test)]
mod tests {
    use super::target;

    #[test]
    fn a_pasted_pair_link_carries_the_address_and_code() {
        assert_eq!(
            target(
                "hex-dictation://pair?host=cshed.ts.net:47831&code=ABCDEF",
                ""
            ),
            Some(("https://cshed.ts.net:47831".into(), "ABCDEF".into()))
        );
        assert_eq!(
            target(" cshed:47831 ", " ABC-DEF "),
            Some(("https://cshed:47831".into(), "ABC-DEF".into()))
        );
        assert_eq!(target("cshed", ""), None);
    }
}
