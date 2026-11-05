import Foundation
import Security
import UIKit

/// A desktop Hex this iPhone paired with (`hex remote pair` on that machine). It transcribes
/// each recording with its own model and Modes, so the iPhone runs no model while it answers.
struct PairedHex: Codable, Equatable, Sendable {
    let url: URL
    let machineID: String
    let name: String
}

/// The paired Hex lives in the app's defaults; its device token only in the Keychain.
enum PairedHexStore {
    private static let key = "remote.pairedHex"
    private static let fallbackKey = "remote.fallsBackToDevice"
    private static let keychainService = "com.carlassmann.hex.remote"

    /// Whether a dictation the paired Hex can't take is transcribed on this iPhone instead
    /// of failing. On unless turned off.
    static var fallsBackToDevice: Bool {
        get { UserDefaults.standard.object(forKey: fallbackKey) as? Bool ?? true }
        set { UserDefaults.standard.set(newValue, forKey: fallbackKey) }
    }

    static func load() -> (hex: PairedHex, token: String)? {
        guard let data = UserDefaults.standard.data(forKey: key),
              let hex = try? JSONDecoder().decode(PairedHex.self, from: data),
              let token = readToken(account: hex.machineID)
        else { return nil }
        return (hex, token)
    }

    static func save(_ hex: PairedHex, token: String) throws {
        try writeToken(token, account: hex.machineID)
        UserDefaults.standard.set(try JSONEncoder().encode(hex), forKey: key)
    }

    static func forget() {
        if let data = UserDefaults.standard.data(forKey: key),
           let hex = try? JSONDecoder().decode(PairedHex.self, from: data) {
            SecItemDelete(query(account: hex.machineID) as CFDictionary)
        }
        UserDefaults.standard.removeObject(forKey: key)
    }

    private static func query(account: String) -> [String: Any] {
        [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: keychainService,
            kSecAttrAccount as String: account,
        ]
    }

    private static func readToken(account: String) -> String? {
        var item: CFTypeRef?
        var search = query(account: account)
        search[kSecReturnData as String] = true
        guard SecItemCopyMatching(search as CFDictionary, &item) == errSecSuccess,
              let data = item as? Data
        else { return nil }
        return String(data: data, encoding: .utf8)
    }

    private static func writeToken(_ token: String, account: String) throws {
        SecItemDelete(query(account: account) as CFDictionary)
        var item = query(account: account)
        item[kSecValueData as String] = Data(token.utf8)
        item[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
        let status = SecItemAdd(item as CFDictionary, nil)
        guard status == errSecSuccess else {
            throw NSError(domain: NSOSStatusErrorDomain, code: Int(status))
        }
    }
}

enum RemoteHexPairing {
    enum Failure: LocalizedError {
        case unreachable
        case invalidCode
        case refused

        var errorDescription: String? {
            switch self {
            case .unreachable: "That Hex doesn't answer. Is it running, and exposed with tailscale serve?"
            case .invalidCode: "That code is wrong or expired. Run hex remote pair again."
            case .refused: "That Hex refused to pair."
            }
        }
    }

    /// `cbook.tailnet.ts.net:47831` or a full URL.
    static func url(forAddress address: String) -> URL? {
        let trimmed = address.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return nil }
        return URL(string: trimmed.contains("://") ? trimmed : "https://\(trimmed)")
    }

    /// Reads `hex-dictation://pair?host=…&code=…`, as printed by `hex remote pair`.
    static func link(_ url: URL) -> (address: String, code: String)? {
        guard url.scheme == "hex-dictation", url.host == "pair",
              let items = URLComponents(url: url, resolvingAgainstBaseURL: false)?.queryItems,
              let host = items.first(where: { $0.name == "host" })?.value,
              let code = items.first(where: { $0.name == "code" })?.value
        else { return nil }
        return (host, code)
    }

    static func pair(address: String, code: String) async throws -> PairedHex {
        guard let url = url(forAddress: address) else { throw Failure.unreachable }
        let session = URLSession(configuration: .ephemeral)

        var request = URLRequest(url: url.appendingPathComponent("pair"), timeoutInterval: 10)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        let label = await MainActor.run { UIDevice.current.name }
        request.httpBody = try JSONEncoder().encode(["code": code, "label": label])

        let data: Data
        let response: URLResponse
        do {
            (data, response) = try await session.data(for: request)
        } catch {
            throw Failure.unreachable
        }
        switch (response as? HTTPURLResponse)?.statusCode {
        case 200: break
        case 403: throw Failure.invalidCode
        default: throw Failure.refused
        }
        let paired = try JSONDecoder().decode(PairResponse.self, from: data)
        let hex = PairedHex(url: url, machineID: paired.machineId, name: paired.machineLabel)
        try PairedHexStore.save(hex, token: paired.token)
        return hex
    }

    private struct PairResponse: Decodable {
        let token: String
        let machineId: String
        let machineLabel: String
    }
}

/// Sends each recording to the paired Hex, which transcribes it with its own model and applies
/// its Modes (an `iPhone` Mode there can clean up spoken self-corrections). The audio goes out
/// while it is recorded, so a long dictation is mostly done when it ends; when that stream
/// breaks, the finished recording is uploaded whole. Falls back to the on-device transcriber
/// whenever that Hex can't answer, unless that fallback is turned off.
actor RemoteTranscriber: LocalTranscribing {
    nonisolated let displayName: String

    private let hex: PairedHex
    private let token: String
    private let fallback: any LocalTranscribing
    /// The on-device model loads only when a dictation first falls back to it.
    private var fallbackPrepared = false
    private let session: URLSession
    private var stream: RemoteStream?

    var languageName: String {
        "Set on \(hex.name)"
    }

    init(hex: PairedHex, token: String, fallback: any LocalTranscribing) {
        self.hex = hex
        self.token = token
        self.fallback = fallback
        self.displayName = hex.name
        let configuration = URLSessionConfiguration.ephemeral
        // The longest gap between bytes. Hex sends a progress line every second while it
        // works, so this passes only when it is gone, however long the dictation.
        configuration.timeoutIntervalForRequest = 15
        configuration.waitsForConnectivity = false
        self.session = URLSession(configuration: configuration)
    }

    /// Nothing to load: the paired Hex keeps its own model loaded, and the on-device one
    /// loads only when a dictation falls back to it.
    func prepare(
        progressHandler: @escaping @Sendable (ModelPreparationProgress) -> Void
    ) async throws {}

    func recordingStarted(_ audioURL: URL) async {
        await stream?.cancel()
        let application = await MainActor.run { UIDevice.current.model }
        let stream = RemoteStream(
            hex: hex,
            token: token,
            audioURL: audioURL,
            application: application,
            session: session
        )
        self.stream = stream
        await stream.start()
    }

    func recordingCancelled() async {
        await stream?.cancel()
        stream = nil
    }

    func transcribe(_ audioURL: URL) async throws -> String {
        try await transcribe(audioURL, vocabulary: [], progress: { _ in })
    }

    /// The paired Hex spells with its own synced word list; only the fallback uses `vocabulary`.
    func transcribe(
        _ audioURL: URL,
        vocabulary: [String],
        progress: @escaping @Sendable (TranscriptionProgress) -> Void
    ) async throws -> String {
        // A retried recording was kept after its stream ended; only the recording being
        // finished has one.
        let stream = self.stream?.audioURL == audioURL ? self.stream : nil
        if stream != nil { self.stream = nil }
        var reason: String
        do {
            return try await transcribeRemotely(audioURL, stream: stream, progress: progress)
        } catch {
            if error is CancellationError || Task.isCancelled { throw CancellationError() }
            reason = Self.reason(error)
        }
        NSLog("\(hex.name) unavailable (\(reason))")
        guard PairedHexStore.fallsBackToDevice else { throw Unavailable(name: hex.name, reason: reason) }
        progress(TranscriptionProgress(
            machine: "iPhone",
            stage: .transcribing,
            fallbackReason: "\(hex.name): \(reason)"
        ))
        if !fallbackPrepared {
            try await fallback.prepare { _ in }
            fallbackPrepared = true
        }
        return try await fallback.transcribe(audioURL, vocabulary: vocabulary)
    }

    private func transcribeRemotely(
        _ audioURL: URL,
        stream: RemoteStream?,
        progress: @escaping @Sendable (TranscriptionProgress) -> Void
    ) async throws -> String {
        if let stream {
            do {
                return try await stream.finish(progress: progress)
            } catch {
                if error is CancellationError || Task.isCancelled { throw CancellationError() }
                NSLog("\(hex.name) stream failed, uploading the whole recording: \(Self.reason(error))")
            }
        }
        progress(TranscriptionProgress(machine: hex.name, stage: .uploading))
        var components = URLComponents(
            url: hex.url.appendingPathComponent("remote/dictations"),
            resolvingAgainstBaseURL: false
        )
        components?.queryItems = [URLQueryItem(name: "progress", value: "1")]
        guard let url = components?.url else { throw URLError(.badURL) }
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("audio/wav", forHTTPHeaderField: "Content-Type")
        request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        request.setValue(await MainActor.run { UIDevice.current.model }, forHTTPHeaderField: "X-Hex-Application")
        request.httpBody = try Data(contentsOf: audioURL, options: .mappedIfSafe)
        return try await RemoteProgressReader.read(
            request: request,
            session: session,
            machine: hex.name,
            progress: progress
        )
    }

    private static func reason(_ error: Error) -> String {
        if let urlError = error as? URLError {
            switch urlError.code {
            case .timedOut: return "stopped answering"
            case .notConnectedToInternet, .networkConnectionLost: return "no connection"
            case .cannotFindHost, .cannotConnectToHost, .dnsLookupFailed: return "not reachable"
            default: return urlError.localizedDescription
            }
        }
        if case RemoteStream.Failure.refused(let code) = error {
            switch code {
            case "model-not-ready", "model-not-loaded": return "its model isn't ready"
            case "queue-full", "stream-busy": return "busy with another transcription"
            case "not-paired": return "this iPhone isn't paired anymore"
            default: return code
            }
        }
        return error.localizedDescription
    }

    private struct Unavailable: LocalizedError {
        let name: String
        let reason: String
        var errorDescription: String? { "\(name): \(reason)." }
    }
}
