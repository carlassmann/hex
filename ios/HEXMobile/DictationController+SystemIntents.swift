import Foundation

/// The Action Button, Control Center, and Siri: commands from `HexSystemCommandBus`.
extension DictationController {
    func wireSystemIntentBus() {
        Task {
            await HexSystemCommandBus.shared.register { [weak self] command in
                guard let self else { return nil }
                return await self.performSystemCommand(command)
            }
        }
    }

    private func performSystemCommand(_ command: HexSystemCommand) async -> String? {
        switch command {
        case .toggleDictation:
            if phase == .recording {
                finishRecording()
                if let task = transcriptionTask {
                    await task.value
                }
                return transcript
            }
            followDictationLanguageSetting()
            if phase == .modelRequired || isFailedPhase {
                prepareModel()
            }
            if phase == .preparingModel {
                try? await waitUntilReady(timeoutSeconds: 10)
            }
            guard phase == .ready else { return nil }
            startRecording(copyToPasteboard: true)
            return ""

        case .stopDictation:
            guard phase == .recording else { return transcript }
            finishRecording()
            if let task = transcriptionTask {
                await task.value
            }
            return transcript

        case .cancelDictation:
            cancelCurrentJob()
            return nil

        case .warmSession:
            if !keyboardSessionEnabled {
                if warmDuration == .off {
                    setWarmDuration(.fifteenMinutes)
                }
                requestWarmSession()
            }
            return nil
        }
    }

    private func waitUntilReady(timeoutSeconds: TimeInterval) async throws {
        let deadline = now() + timeoutSeconds
        while phase == .preparingModel && now() < deadline {
            try await Task.sleep(for: .milliseconds(100))
        }
    }
}
