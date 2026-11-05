import FluidAudio
import Foundation

struct ModelPreparationProgress: Sendable {
    let fractionCompleted: Double
    let detail: String
}

protocol LocalTranscribing: Actor {
    nonisolated var displayName: String { get }
    var languageName: String { get }

    func prepare(
        progressHandler: @escaping @Sendable (ModelPreparationProgress) -> Void
    ) async throws

    func transcribe(_ audioURL: URL) async throws -> String

    /// A recording to `audioURL` began; a remote transcriber starts sending it.
    func recordingStarted(_ audioURL: URL) async
    /// The recording was thrown away.
    func recordingCancelled() async
    /// Transcribes the finished recording, reporting where it is meanwhile.
    func transcribe(
        _ audioURL: URL,
        progress: @escaping @Sendable (TranscriptionProgress) -> Void
    ) async throws -> String
}

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

extension LocalTranscribing {
    func recordingStarted(_ audioURL: URL) async {}
    func recordingCancelled() async {}

    func transcribe(
        _ audioURL: URL,
        progress: @escaping @Sendable (TranscriptionProgress) -> Void
    ) async throws -> String {
        progress(TranscriptionProgress(machine: "iPhone", stage: .transcribing))
        return try await transcribe(audioURL)
    }
}

actor ParakeetTranscriber: LocalTranscribing {
    enum TranscriberError: LocalizedError {
        case notPrepared
        case emptyTranscript

        var errorDescription: String? {
            switch self {
            case .notPrepared:
                "Parakeet V2 is not ready."
            case .emptyTranscript:
                "Parakeet did not detect any speech."
            }
        }
    }

    private var manager: AsrManager?

    nonisolated let displayName = "Parakeet V2"
    let languageName = "English"

    nonisolated static var hasCachedModel: Bool {
        AsrModels.modelsExist(
            at: AsrModels.defaultCacheDirectory(for: .v2),
            version: .v2
        )
    }

    func prepare(
        progressHandler: @escaping @Sendable (ModelPreparationProgress) -> Void
    ) async throws {
        guard manager == nil else { return }

        let models = try await AsrModels.downloadAndLoad(
            version: .v2,
            progressHandler: { progress in
                progressHandler(
                    ModelPreparationProgress(
                        fractionCompleted: progress.fractionCompleted,
                        detail: Self.detail(for: progress.phase)
                    )
                )
            }
        )
        let manager = AsrManager(config: .default)
        try await manager.loadModels(models)
        self.manager = manager
    }

    private static func detail(for phase: DownloadPhase) -> String {
        switch phase {
        case .listing:
            "Checking model files"
        case .downloading(let completedFiles, let totalFiles):
            "Downloading file \(min(completedFiles + 1, totalFiles)) of \(totalFiles)"
        case .compiling(let modelName):
            "Compiling \(modelName)"
        }
    }

    func transcribe(_ audioURL: URL) async throws -> String {
        guard let manager else {
            throw TranscriberError.notPrepared
        }

        let decoderLayers = await manager.decoderLayerCount
        var decoderState = TdtDecoderState.make(decoderLayers: decoderLayers)
        let result = try await manager.transcribe(
            audioURL,
            decoderState: &decoderState
        )
        let text = result.text.trimmingCharacters(in: CharacterSet.whitespacesAndNewlines)
        guard !text.isEmpty else {
            throw TranscriberError.emptyTranscript
        }
        return text
    }
}
