# Hex dictation relay (prototype)

A device uploads a recording, the machine running this relay transcribes it with
Hex's desktop models, applies spoken self-corrections, and sends the text back.

- Transcription: Hex's headless service (`hex service --embedded`, macOS and Linux), model `cohere_transcribe` by default.
  Recordings arrive without a language, so use a model that detects it (`qwen3_asr06_b`, `whisper_large_v3_turbo`), or set `RELAY_LANGUAGE`.
- Cleanup: OpenCode's plain text-generation endpoint (no agent, no tools), model `opencode/claude-haiku-5-5`. The relay starts its own `opencode serve` on `127.0.0.1:47811`.
- If cleanup fails, the raw transcript is returned.

## Run

```sh
bun src/server.ts
tailscale serve --bg --https=10003 http://127.0.0.1:47810   # once; persists
```

In the iPhone app, set "Transcribe remotely" to `<machine>.<tailnet>.ts.net:10003`.

## Try it

```sh
curl -X POST --data-binary @samples/de.wav "http://127.0.0.1:47810/dictate?language=de"
```

`?cleanup=off` returns the transcript as heard. Settings, all optional:
`RELAY_HEX_EXECUTABLE` (default: the macOS app), `RELAY_HEX_MODEL`,
`RELAY_LANGUAGE`, `RELAY_CLEANUP_MODEL`, `RELAY_PORT`.

## Stop exposing it

```sh
tailscale serve --https=10003 off
```
