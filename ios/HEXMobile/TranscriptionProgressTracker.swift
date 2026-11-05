import Foundation
import Observation

/// Where the current dictation's text is at and for how long, as one status line
/// ("cshed · rewriting · 0:07") for the app, the keyboard (through the App Group),
/// and the Live Activity.
@MainActor
@Observable
final class TranscriptionProgressTracker {
    private(set) var progress: TranscriptionProgress?
    private(set) var since: Date?
    /// Bumped each second, so a status line's elapsed time redraws.
    private var tick = 0
    private var ticker: Task<Void, Never>?
    private var machine = "iPhone"
    @ObservationIgnored private let keyboardBridge: KeyboardBridge

    init(keyboardBridge: KeyboardBridge) {
        self.keyboardBridge = keyboardBridge
    }

    /// Starts the clock for a transcription on `machine`, publishing the status line
    /// each second while `isCurrent` holds.
    func begin(machine: String, isCurrent: @escaping @MainActor () -> Bool) {
        self.machine = machine
        progress = nil
        since = Date()
        ticker?.cancel()
        ticker = Task { [weak self] in
            while !Task.isCancelled, let self, isCurrent() {
                self.tick &+= 1
                self.keyboardBridge.transcriptionStatus = self.statusLine(machine: machine)
                try? await Task.sleep(for: .seconds(1))
            }
        }
    }

    /// Records `progress`; the Live Activity's new subtitle when its machine or stage changed.
    func update(_ progress: TranscriptionProgress) -> String? {
        guard since != nil else { return nil }
        let changed = progress.stage != self.progress?.stage || progress.machine != self.progress?.machine
        self.progress = progress
        keyboardBridge.transcriptionStatus = statusLine(machine: machine)
        return changed ? Self.liveActivitySubtitle(progress) : nil
    }

    func end() {
        ticker?.cancel()
        ticker = nil
        progress = nil
        since = nil
        keyboardBridge.transcriptionStatus = nil
    }

    /// `machine` names where it runs until progress says otherwise.
    func statusLine(machine: String) -> String {
        _ = tick
        var parts = [progress?.machine ?? machine]
        switch progress?.stage {
        case .uploading:
            parts.append("uploading")
        case .rewriting:
            parts.append("rewriting")
        case .transcribing, nil:
            if let left = progress?.audioLeftMs, left >= 2_000 {
                parts.append("transcribing, \(Self.clock(TimeInterval(left) / 1_000)) left")
            } else {
                parts.append("transcribing")
            }
        }
        if let since {
            parts.append(Self.clock(Date().timeIntervalSince(since)))
        }
        var line = parts.joined(separator: " · ")
        if let reason = progress?.fallbackReason {
            line += " (\(reason))"
        }
        return line
    }

    private static func liveActivitySubtitle(_ progress: TranscriptionProgress) -> String {
        switch progress.stage {
        case .uploading: "Uploading to \(progress.machine)…"
        case .transcribing: "Transcribing on \(progress.machine)…"
        case .rewriting: "Rewriting on \(progress.machine)…"
        }
    }

    private static func clock(_ seconds: TimeInterval) -> String {
        let whole = max(0, Int(seconds.rounded(.down)))
        return String(format: "%d:%02d", whole / 60, whole % 60)
    }
}
