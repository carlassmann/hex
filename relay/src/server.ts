/**
 * Dictation relay: the iPhone uploads a recording, this Mac transcribes it with Hex's
 * models, cleans it up, and answers with the text. Listens on loopback only;
 * `tailscale serve` puts it on the tailnet over HTTPS.
 */
import { startCleaner } from "./cleanup"
import { startHexTranscriber } from "./hex"

const PORT = Number(process.env.RELAY_PORT ?? 47810)

const transcriber = await startHexTranscriber(process.env.RELAY_HEX_MODEL ?? "cohere_transcribe")
const cleaner = await startCleaner({ providerID: "opencode", id: process.env.RELAY_CLEANUP_MODEL ?? "claude-haiku-5-5" })

async function dictate(request: Request): Promise<Response> {
  const url = new URL(request.url)
  const language = url.searchParams.get("language") ?? process.env.RELAY_LANGUAGE
  const wantsCleanup = url.searchParams.get("cleanup") !== "off"
  const wav = await request.arrayBuffer()

  const started = performance.now()
  const raw = await transcriber.transcribe(wav, language)
  const transcribedMs = Math.round(performance.now() - started)

  let text = raw
  let cleanupError: string | undefined
  if (wantsCleanup) {
    try {
      text = await cleaner.clean(raw)
    } catch (error) {
      // A failed cleanup still delivers what was said.
      cleanupError = String(error)
    }
  }
  const totalMs = Math.round(performance.now() - started)
  console.log(`[${language ?? "auto"}] ${transcribedMs}/${totalMs} ms: ${JSON.stringify(raw)} -> ${JSON.stringify(text)}${cleanupError ? ` (${cleanupError})` : ""}`)
  return Response.json({ text, raw, language, transcribedMs, totalMs, cleanupError })
}

const server = Bun.serve({
  hostname: "127.0.0.1",
  port: PORT,
  routes: {
    "/health": new Response("ok"),
    "/dictate": { POST: dictate },
  },
  error: (error) => Response.json({ error: String(error) }, { status: 500 }),
})

console.log(`Relay listening on ${server.url}`)

for (const signal of ["SIGINT", "SIGTERM"] as const) {
  process.on(signal, () => {
    transcriber.close()
    cleaner.close()
    process.exit(0)
  })
}
