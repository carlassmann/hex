# Hex

Hex is local voice dictation for Mac, Windows and Linux. Hold a key, speak, and
it pastes.

> [!NOTE]
> **This is Carl Assmann's fork** of [anomalyco/hex](https://github.com/anomalyco/hex).
> Its changes live on `carl/dev`, rebased onto upstream `dev`, one commit per feature.
>
> **Remote transcription.** Any desktop Hex can transcribe for your other
> devices, so a phone or laptop uses a stronger machine's model and Modes
> (cleanup, spoken self-corrections) over your tailnet.
> - `hex remote pair` prints a QR code and a one-time code (five minutes,
>   T3 Code-style). Paired devices get their own token; `hex remote devices`,
>   `hex remote revoke <id>`, and `hex remote disable` manage them.
> - Hex serves paired devices on `127.0.0.1:47831`; expose it with
>   `tailscale serve --bg --https=47831 http://127.0.0.1:47831`.
> - Code: `src/remote_pairing.rs`, `src/local_api/remote.rs`.
>
> **Synced word list.** Paired devices read and edit the desktop's Vocabulary
> (`/remote/vocabulary`), so a word added on the phone helps every later
> dictation on that machine.
>
> **iOS app** (`ios/`):
> - Pair with a desktop Hex by scanning its QR code, and transcribe there. It
>   falls back to on-device transcription unless you turn that off; the
>   keyboard says where it transcribes.
> - Select a word in any app and tap **Add to word list** on the Hex keyboard;
>   the app syncs it to the paired Hex and lists the synced words.
> - Signed with my own team, bundle IDs, and App Group.

## How it works

- hold the shortcut, speak, then let go
- Hex transcribes on your device and pastes into whatever app you're in
- the first run downloads a speech model; after that nothing leaves your machine
- experimental features live in Labs, at the bottom of the sidebar

## Downloads

Stable is the current release. Dev is the newest build of `dev` and updates
itself from the dev channel.

| Platform | Stable | Dev |
| --- | --- | --- |
| macOS (Apple silicon) | [DMG](https://anoma.ly/hex/download/macos-arm64-dmg) | [DMG](https://anoma.ly/hex/download/dev/macos-arm64-dmg) |
| Windows x64 | [Installer](https://anoma.ly/hex/download/windows-x64-setup) | [Installer](https://anoma.ly/hex/download/dev/windows-x64-setup) |
| Windows ARM64 | [Installer](https://anoma.ly/hex/download/windows-arm64-setup) | [Installer](https://anoma.ly/hex/download/dev/windows-arm64-setup) |
| Linux x86_64 | [Executable](https://anoma.ly/hex/download/linux-x64) | [Executable](https://anoma.ly/hex/download/dev/linux-x64) |
| Linux x86_64 (Debian, Ubuntu) | [.deb](https://anoma.ly/hex/download/linux-x64-deb) | [.deb](https://anoma.ly/hex/download/dev/linux-x64-deb) |
| Linux x86_64 (Fedora, openSUSE) | [.rpm](https://anoma.ly/hex/download/linux-x64-rpm) | [.rpm](https://anoma.ly/hex/download/dev/linux-x64-rpm) |
| Linux ARM64 | [Executable](https://anoma.ly/hex/download/linux-arm64) | [Executable](https://anoma.ly/hex/download/dev/linux-arm64) |
| Linux ARM64 (Debian, Ubuntu) | [.deb](https://anoma.ly/hex/download/linux-arm64-deb) | [.deb](https://anoma.ly/hex/download/dev/linux-arm64-deb) |
| Linux ARM64 (Fedora, openSUSE) | [.rpm](https://anoma.ly/hex/download/linux-arm64-rpm) | [.rpm](https://anoma.ly/hex/download/dev/linux-arm64-rpm) |

Or install with:

- Homebrew: `brew install --cask anomalyco/tap/hex`
- AUR: `yay -S hex-bin`
- install script (Linux): `curl -fsSL https://anoma.ly/hex/install-linux.sh | sh`
- Nix: `nix profile install github:anomalyco/hex`

More docs are in [`docs/`](docs/README.md).

## Contributing

We don't take pull requests. [Open an issue](https://github.com/anomalyco/hex/issues)
describing the bug or idea, and our agents usually turn it around fast.

MIT licensed.
