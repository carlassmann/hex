import Foundation

/// The paired Hex (this fork): pairing, where a dictation is transcribed, and
/// the keyboard's diagnostics.
extension DictationController {
    /// Why the keyboard last opened this app instead of dictating, and why the warm
    /// session before it ended; nil until the keyboard has done that once.
    var keyboardColdOpenNote: String? {
        guard let cold = keyboardBridge.lastColdOpen else { return nil }
        let time = { (at: TimeInterval) in
            Date(timeIntervalSince1970: at).formatted(date: .omitted, time: .standard)
        }
        var note = "Keyboard opened Hex at \(time(cold.at)): \(cold.why)."
        if let end = keyboardBridge.lastWarmEnd, end.at <= cold.at {
            note += " Warm session last ended at \(time(end.at)): \(end.why)."
        }
        return note
    }

    /// "cshed · transcribing · 0:12", or nil when not transcribing.
    var transcriptionStatus: String? {
        phase == .transcribing ? transcriptionTracker.statusLine(machine: engineName) : nil
    }

    /// Pairs with a desktop Hex (`hex remote pair` there), which then transcribes for this iPhone.
    func pairWithHex(address: String, code: String) async throws {
        pairedHex = try await RemoteHexPairing.pair(address: address, code: code)
        reloadTranscriberIfIdle()
        await wordList.sync()
    }

    func unpairHex() {
        PairedHexStore.forget()
        pairedHex = nil
        wordList.clear()
        reloadTranscriberIfIdle()
    }

    private func reloadTranscriberIfIdle() {
        if phase == .ready || isFailedPhase {
            prepareModel()
        }
    }

    static func defaultTranscriber(paired: (hex: PairedHex, token: String)?) async -> any LocalTranscribing {
        let onDevice = await onDeviceTranscriber()
        guard let paired else { return onDevice }
        return RemoteTranscriber(hex: paired.hex, token: paired.token, fallback: onDevice)
    }

    private static func onDeviceTranscriber() async -> any LocalTranscribing {
        if #available(iOS 26.0, *),
           AppleSpeechTranscriber.isAvailable,
           let locale = await AppleSpeechTranscriber.preferredLocale() {
            return AppleSpeechTranscriber(locale: locale)
        }
        return ParakeetTranscriber()
    }
}
