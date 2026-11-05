import Foundation

/// The hosted keyboard's demo states for `--snap-state hosted-keyboard-*`: what the app
/// would publish to the keyboard while warm, recording, and through a whole dictation.
@MainActor
enum HostedKeyboardFixture {
    static func publishReady(to bridge: KeyboardBridge) {
        Task { @MainActor in
            while !Task.isCancelled {
                let t = Date().timeIntervalSince1970
                bridge.publish(
                    KeyboardSnapshot(
                        state: .ready,
                        heartbeat: t,
                        expiresAt: t + 14 * 60 + 42
                    )
                )
                try? await Task.sleep(for: .milliseconds(250))
            }
        }
    }

    static func publishRecording(to bridge: KeyboardBridge) {
        let startWall = Date().timeIntervalSince1970 - 4.0
        Task { @MainActor in
            var simTime: Float = 0.4
            while !Task.isCancelled {
                let t = Date().timeIntervalSince1970
                let db = -28.5 + 5.2 * sin(simTime * 19.0) + 2.4 * cos(simTime * 7.3)
                bridge.publish(
                    KeyboardSnapshot(
                        state: .recording,
                        heartbeat: t,
                        expiresAt: t + 15 * 60,
                        recordingStartedAt: startWall,
                        audioLevel: 0.78,
                        loudnessDB: db,
                        jobID: "hosted-demo-job"
                    )
                )
                simTime += 0.033
                try? await Task.sleep(for: .milliseconds(33))
            }
        }
    }

    static func playTour(to bridge: KeyboardBridge) {
        Task { @MainActor in
            let boot = Date().timeIntervalSince1970
            let freshResultID = "hosted-tour-\(UUID().uuidString)"
            var simTime: Float = 0.0
            while !Task.isCancelled {
                let t = Date().timeIntervalSince1970
                let elapsed = t - boot
                if elapsed < 2.5 {
                    bridge.publish(
                        KeyboardSnapshot(
                            state: .ready,
                            heartbeat: t,
                            expiresAt: t + 15 * 60
                        )
                    )
                } else if elapsed < 5.2 {
                    let db = DotScopeSimulation.demoLoudness(at: simTime) + 4.5
                    bridge.publish(
                        KeyboardSnapshot(
                            state: .recording,
                            heartbeat: t,
                            expiresAt: t + 15 * 60,
                            recordingStartedAt: boot + 2.5,
                            audioLevel: 0.78,
                            loudnessDB: db,
                            jobID: "hosted-tour-job"
                        )
                    )
                    simTime += 0.033
                } else if elapsed < 6.4 {
                    bridge.publish(
                        KeyboardSnapshot(
                            state: .transcribing,
                            heartbeat: t,
                            expiresAt: t + 15 * 60,
                            jobID: "hosted-tour-job"
                        )
                    )
                } else {
                    bridge.publish(
                        KeyboardSnapshot(
                            state: .ready,
                            heartbeat: t,
                            expiresAt: t + 14 * 60 + 54,
                            resultID: freshResultID,
                            resultExpiresAt: boot + 30,
                            transcript: "See you at ten."
                        )
                    )
                }
                try? await Task.sleep(for: .milliseconds(33))
            }
        }
    }
}
