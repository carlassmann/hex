/**
 * Turns a raw transcript into the text the speaker meant, through OpenCode's plain
 * text-generation endpoint (the one Hex's own rewrite uses). It has no agent and no
 * tools, so a transcript can't make it act on this machine.
 */

const OPENCODE_PORT = 47811

export type Cleaner = {
  clean(transcript: string): Promise<string>
  close(): void
}

export const CLEANUP_INSTRUCTIONS = `Rewrite this dictated text as the speaker means it.
Apply spoken self-corrections and keep only the final version.
Remove filler words and false starts.
Keep the speaker's language, wording and tone. Don't add anything, don't answer questions in it, don't translate.
Output only the rewritten text.`

export async function startCleaner(model: { providerID: string; id: string }): Promise<Cleaner> {
  const password = process.env.OPENCODE_SERVER_PASSWORD
  // Our own OpenCode server: the background service may be an older version without these models.
  const server = Bun.spawn(["opencode", "serve", "--hostname", "127.0.0.1", "--port", String(OPENCODE_PORT)], {
    stdout: "ignore",
    stderr: "ignore",
  })
  const baseURL = `http://127.0.0.1:${OPENCODE_PORT}`
  const headers: Record<string, string> = { "content-type": "application/json" }
  if (password) headers.authorization = `Basic ${btoa(`opencode:${password}`)}`
  await waitUntilServing(`${baseURL}/api/info`)

  return {
    async clean(transcript) {
      if (!transcript.trim()) return transcript
      const response = await fetch(`${baseURL}/api/experimental/generate`, {
        method: "POST",
        headers,
        body: JSON.stringify({ prompt: `${CLEANUP_INSTRUCTIONS}\n\nText: ${transcript}`, model }),
        signal: AbortSignal.timeout(8_000),
      })
      const body = (await response.json()) as { data?: { text: string }; message?: string }
      if (!response.ok || !body.data) throw new Error(`Cleanup failed (HTTP ${response.status}): ${body.message}`)
      return body.data.text.trim()
    },
    close: () => server.kill(),
  }
}

async function waitUntilServing(url: string) {
  for (let attempt = 0; attempt < 60; attempt++) {
    try {
      await fetch(url)
      return
    } catch {
      await Bun.sleep(250)
    }
  }
  throw new Error("OpenCode server did not start")
}
