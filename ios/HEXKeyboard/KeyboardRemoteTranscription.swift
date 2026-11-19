import Foundation
import UIKit

/// Sends a recording the keyboard made by itself to the paired Hex (`POST /remote/dictations`),
/// which transcribes it with its own model and Modes, as the app does.
enum KeyboardRemoteTranscription {
    struct Unavailable: LocalizedError {
        let name: String
        let reason: String
        var errorDescription: String? { "\(name): \(reason)" }
    }

    private struct Answer: Decodable {
        let text: String
    }

    private struct Refusal: Decodable {
        let code: String?
    }

    static func transcribe(_ audioURL: URL, on hex: PairedHex, token: String) async throws -> String {
        var request = URLRequest(url: hex.url.appendingPathComponent("remote/dictations"))
        request.httpMethod = "POST"
        request.setValue("audio/wav", forHTTPHeaderField: "Content-Type")
        request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        request.setValue(await MainActor.run { UIDevice.current.model }, forHTTPHeaderField: "X-Hex-Application")
        let configuration = URLSessionConfiguration.ephemeral
        configuration.timeoutIntervalForRequest = 60
        configuration.waitsForConnectivity = false
        let session = URLSession(configuration: configuration)
        defer { session.finishTasksAndInvalidate() }
        let data: Data
        let response: URLResponse
        do {
            (data, response) = try await session.upload(for: request, fromFile: audioURL)
        } catch {
            if error is CancellationError || Task.isCancelled { throw CancellationError() }
            throw Unavailable(name: hex.name, reason: reason(error))
        }
        let status = (response as? HTTPURLResponse)?.statusCode ?? 0
        guard status == 200 else {
            let code = (try? JSONDecoder().decode(Refusal.self, from: data))?.code
            throw Unavailable(name: hex.name, reason: refusal(code) ?? "answered \(status)")
        }
        guard let answer = try? JSONDecoder().decode(Answer.self, from: data) else {
            throw Unavailable(name: hex.name, reason: "gave no transcript")
        }
        return answer.text
    }

    private static func reason(_ error: Error) -> String {
        guard let urlError = error as? URLError else { return error.localizedDescription }
        switch urlError.code {
        case .timedOut: return "stopped answering"
        case .notConnectedToInternet, .networkConnectionLost: return "no connection"
        case .cannotFindHost, .cannotConnectToHost, .dnsLookupFailed: return "not reachable"
        default: return urlError.localizedDescription
        }
    }

    private static func refusal(_ code: String?) -> String? {
        guard let code else { return nil }
        switch code {
        case "model-not-ready", "model-not-loaded": return "its model isn't ready"
        case "queue-full", "stream-busy": return "busy with another transcription"
        case "not-paired": return "this iPhone isn't paired anymore"
        default: return code
        }
    }
}
