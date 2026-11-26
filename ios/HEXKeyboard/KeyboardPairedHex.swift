import Foundation

/// This fork's keyboard logic (the paired Hex, its word list, freeing memory to dictate),
/// kept out of `KeyboardViewController.swift`.
@MainActor
enum KeyboardPairedHex {
    /// A short selection (a name or term, not a sentence) can be added to the word list.
    static func wordListCandidate(_ selectedText: String?) -> String? {
        let selected = selectedText?.trimmingCharacters(in: .whitespacesAndNewlines.union(.punctuationCharacters))
        guard let selected, !selected.isEmpty, selected.count <= 64,
              selected.split(whereSeparator: \.isWhitespace).count <= 4
        else { return nil }
        return selected
    }

    /// Where the app's dictation is transcribed (the paired Hex or this iPhone), and how far.
    static func syncTranscriber(_ viewModel: KeyboardViewModel, bridge: KeyboardBridge, transcribing: Bool) {
        let transcriberName = bridge.transcriberName
        if viewModel.transcriberName != transcriberName { viewModel.transcriberName = transcriberName }
        let status = transcribing ? bridge.transcriptionStatus : nil
        if viewModel.transcriptionStatus != status { viewModel.transcriptionStatus = status }
    }

    private static var lastMemoryRelease: TimeInterval = 0

    /// Dictating in the keyboard beats opening Hex, so before giving up for lack of memory the
    /// keyboard drops what it can load again and isn't on screen (emoji, an idle recording
    /// panel). At most every 10 s, as availability is checked on every poll.
    static func memoryReleaseDue() -> Bool {
        let now = Date().timeIntervalSince1970
        guard now - lastMemoryRelease >= 10 else { return false }
        lastMemoryRelease = now
        return true
    }
}
