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
    private static let keychainService = "com.carlassmann.hex.remote"

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
/// its Modes (an `iPhone` Mode there can clean up spoken self-corrections).
/// Falls back to the on-device transcriber whenever that Hex doesn't answer in time.
actor RemoteTranscriber: LocalTranscribing {
    nonisolated let displayName: String

    private let hex: PairedHex
    private let token: String
    private let fallback: any LocalTranscribing
    private let session: URLSession

    var languageName: String {
        "Set on \(hex.name)"
    }

    init(hex: PairedHex, token: String, fallback: any LocalTranscribing) {
        self.hex = hex
        self.token = token
        self.fallback = fallback
        self.displayName = hex.name
        let configuration = URLSessionConfiguration.ephemeral
        configuration.timeoutIntervalForRequest = 15
        configuration.waitsForConnectivity = false
        self.session = URLSession(configuration: configuration)
    }

    /// Only the fallback needs preparing; the paired Hex keeps its own model loaded.
    func prepare(
        progressHandler: @escaping @Sendable (ModelPreparationProgress) -> Void
    ) async throws {
        try await fallback.prepare(progressHandler: progressHandler)
    }

    func transcribe(_ audioURL: URL) async throws -> String {
        do {
            return try await transcribeRemotely(audioURL)
        } catch {
            NSLog("\(hex.name) unavailable, transcribing on device: \(error.localizedDescription)")
            return try await fallback.transcribe(audioURL)
        }
    }

    private func transcribeRemotely(_ audioURL: URL) async throws -> String {
        var request = URLRequest(url: hex.url.appendingPathComponent("remote/dictations"))
        request.httpMethod = "POST"
        request.setValue("audio/wav", forHTTPHeaderField: "Content-Type")
        request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        request.setValue(await MainActor.run { UIDevice.current.model }, forHTTPHeaderField: "X-Hex-Application")
        let (data, response) = try await session.upload(for: request, fromFile: audioURL)
        guard (response as? HTTPURLResponse)?.statusCode == 200 else {
            throw URLError(.badServerResponse)
        }
        return try JSONDecoder().decode(Dictation.self, from: data).text
    }

    private struct Dictation: Decodable {
        let text: String
    }
}
