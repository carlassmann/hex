/** Hex's own speech models, run by the Hex app's headless service (`hex service --embedded`). */

const HEX_EXECUTABLE = process.env.RELAY_HEX_EXECUTABLE ?? "/Applications/Hex.app/Contents/MacOS/hex"

export type Transcriber = {
  /** Without a language, models that detect it do; the others need one (`RELAY_LANGUAGE`). */
  transcribe(wav: ArrayBuffer, language?: string): Promise<string>
  close(): void
}

type Endpoint = { url: string; token: string }

export async function startHexTranscriber(model: string): Promise<Transcriber> {
  // The service lives as long as its stdin stays open, so keep the pipe.
  const child = Bun.spawn([HEX_EXECUTABLE, "service", "--embedded"], { stdin: "pipe", stdout: "pipe", stderr: "ignore" })
  const endpoint = await readHandshake(child.stdout)
  const auth = { authorization: `Bearer ${endpoint.token}` }
  const preparedLanguages = new Set<string>()

  const query = (language?: string) => (language ? `language=${encodeURIComponent(language)}` : "")

  async function prepare(language = "") {
    if (preparedLanguages.has(language)) return
    const response = await fetch(`${endpoint.url}/models/${model}/prepare?${query(language)}`, { method: "POST", headers: auth })
    await response.text()
    if (!response.ok) throw new Error(`Hex could not prepare ${model} for "${language}" (HTTP ${response.status})`)
    preparedLanguages.add(language)
  }

  return {
    async transcribe(wav, language) {
      await prepare(language)
      const response = await fetch(`${endpoint.url}/transcriptions?model=${model}&${query(language)}`, {
        method: "POST",
        headers: { ...auth, "content-type": "audio/wav" },
        body: wav,
      })
      if (!response.ok) throw new Error(`Hex transcription failed (HTTP ${response.status}): ${await response.text()}`)
      const { transcript } = (await response.json()) as { transcript: string }
      return transcript
    },
    close: () => child.kill(),
  }
}

async function readHandshake(stdout: ReadableStream<Uint8Array>): Promise<Endpoint> {
  const reader = stdout.getReader()
  const decoder = new TextDecoder()
  let text = ""
  while (!text.includes("\n")) {
    const { value, done } = await reader.read()
    if (done) throw new Error("Hex service exited before it was ready")
    text += decoder.decode(value)
  }
  reader.releaseLock()
  return JSON.parse(text.slice(0, text.indexOf("\n")))
}
