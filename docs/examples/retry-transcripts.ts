import { defineHexConfig } from "@hex/commands"

// Retries a dictation with Cohere Transcribe when the first model got it
// wrong in one of two recognizable ways: it heard speech but wrote nothing,
// or it wrote Cyrillic for a language that isn't written in Cyrillic (Parakeet
// sometimes drifts into Russian on short German or English dictations).
//
// Needs Labs › Transcribe again in transforms, Cohere Transcribe installed,
// and this transformation selected in the dictation mode. See
// https://anoma.ly/hex/docs/transforms#hearing-a-dictation-again

const RETRY_MODEL = "cohere_transcribe"

const CYRILLIC = /\p{Script=Cyrillic}/u
const CYRILLIC_LANGUAGES = new Set(["ru", "uk", "be", "bg", "sr", "mk", "kk", "ky", "mn", "tg"])

/** Cyrillic text for a dictation whose language is known and written otherwise. */
const isWrongScript = (text: string, language: string | undefined) =>
  language !== undefined && !CYRILLIC_LANGUAGES.has(language) && CYRILLIC.test(text)

export default defineHexConfig({
  transformations: {
    "retry-transcripts": {
      name: "Retry failed transcripts",
      description: "Hear empty or wrong-script dictations again with Cohere Transcribe",
      timeout: 30000,
      transform: async (text, context) => {
        const transcript = context.transcript
        if (!transcript || transcript.model === RETRY_MODEL) return text

        const heardNothing = text.trim() === "" && transcript.heardSpeech
        if (!heardNothing && !isWrongScript(text, transcript.language)) return text

        try {
          const retried = await transcript.transcribeAgain({ model: RETRY_MODEL })
          return retried || text
        } catch (error) {
          // Cohere isn't installed or lacks the language: keep the first result.
          console.log(`retry-transcripts: ${error instanceof Error ? error.message : error}`)
          return text
        }
      },
    },
  },
})
