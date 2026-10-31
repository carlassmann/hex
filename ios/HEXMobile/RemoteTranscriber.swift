import Foundation

/// Sends each recording to a Hex relay on another machine in the tailnet, which transcribes it
/// with Hex's desktop models (detecting the language) and applies spoken self-corrections.
/// Falls back to the on-device transcriber whenever the relay doesn't answer in time.
actor RemoteTranscriber: LocalTranscribing {
    nonisolated let displayName = "Remote"

    private let relayURL: URL
    private let fallback: any LocalTranscribing
    private let session: URLSession

    var languageName: String {
        "Detected"
    }

    init(relayURL: URL, fallback: any LocalTranscribing) {
        self.relayURL = relayURL
        self.fallback = fallback
        let configuration = URLSessionConfiguration.ephemeral
        configuration.timeoutIntervalForRequest = 10
        configuration.waitsForConnectivity = false
        self.session = URLSession(configuration: configuration)
    }

    /// Only the fallback needs preparing; the relay keeps its own models loaded.
    func prepare(
        progressHandler: @escaping @Sendable (ModelPreparationProgress) -> Void
    ) async throws {
        try await fallback.prepare(progressHandler: progressHandler)
    }

    func transcribe(_ audioURL: URL) async throws -> String {
        do {
            return try await transcribeOnMac(audioURL)
        } catch {
            NSLog("Hex relay unavailable, transcribing on device: \(error.localizedDescription)")
            return try await fallback.transcribe(audioURL)
        }
    }

    private func transcribeOnMac(_ audioURL: URL) async throws -> String {
        var request = URLRequest(url: relayURL.appendingPathComponent("dictate"))
        request.httpMethod = "POST"
        request.setValue("audio/wav", forHTTPHeaderField: "Content-Type")
        let (data, response) = try await session.upload(for: request, fromFile: audioURL)
        guard (response as? HTTPURLResponse)?.statusCode == 200 else {
            throw URLError(.badServerResponse)
        }
        return try JSONDecoder().decode(RelayResult.self, from: data).text
    }

    private struct RelayResult: Decodable {
        let text: String
    }
}
