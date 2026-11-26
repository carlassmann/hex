import Foundation

/// Where a dictation's text is at, for the app, keyboard, and Live Activity.
struct TranscriptionProgress: Sendable, Equatable {
    enum Stage: Sendable, Equatable {
        case uploading
        case transcribing
        /// Running the paired Hex's Modes (such as an OpenCode rewrite).
        case rewriting
    }

    /// The paired Hex's name, or "iPhone".
    let machine: String
    let stage: Stage
    /// Recorded audio the machine has not transcribed yet.
    var audioLeftMs: Int = 0
    /// Why this iPhone took over from the paired Hex.
    var fallbackReason: String?
}
