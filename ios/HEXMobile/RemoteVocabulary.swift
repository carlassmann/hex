import Foundation
import Observation

/// The paired Hex's word list (`/remote/vocabulary`). It is the one list that machine's
/// transcription and cleanup use, so a word added here also helps its own dictation.
enum RemoteVocabulary {
    struct Term: Codable, Hashable, Identifiable, Sendable {
        let term: String
        let soundsLike: [String]

        var id: String { term.lowercased() }
    }

    /// The paired Hex refused this word itself (`invalid-term`): sending it again can't help.
    struct Rejected: Error {
        let status: Int
    }

    /// What became of a queue of words sent one by one.
    struct Delivery {
        /// Words the Hex took or rejected; neither stays queued.
        var settled: [String] = []
        var rejected: [String] = []
        /// The Hex's list after the last word it answered for.
        var terms: [Term]?
        /// Why sending stopped early; the words after `settled` stay queued.
        var failure: Error?
    }

    /// Sends `words` in order, skipping a word the Hex rejects and stopping at the first
    /// failure that isn't about the word (no answer, not paired).
    static func deliver(_ words: [String], add: @Sendable (String) async throws -> [Term]) async -> Delivery {
        var delivery = Delivery()
        for word in words {
            do {
                delivery.terms = try await add(word)
            } catch is Rejected {
                delivery.rejected.append(word)
            } catch {
                delivery.failure = error
                break
            }
            delivery.settled.append(word)
        }
        return delivery
    }

    static func list(on hex: PairedHex, token: String) async throws -> [Term] {
        try await send(request(hex, token, path: "remote/vocabulary", method: "GET"))
    }

    static func add(_ term: String, on hex: PairedHex, token: String) async throws -> [Term] {
        try await send(request(hex, token, path: "remote/vocabulary", body: ["term": term]))
    }

    static func remove(_ term: String, on hex: PairedHex, token: String) async throws -> [Term] {
        try await send(request(hex, token, path: "remote/vocabulary/remove", body: ["term": term]))
    }

    private struct Response: Decodable {
        let terms: [Term]
    }

    private static func request(
        _ hex: PairedHex,
        _ token: String,
        path: String,
        method: String = "POST",
        body: [String: String]? = nil
    ) throws -> URLRequest {
        var request = URLRequest(url: hex.url.appendingPathComponent(path), timeoutInterval: 10)
        request.httpMethod = method
        request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        if let body {
            request.setValue("application/json", forHTTPHeaderField: "Content-Type")
            request.httpBody = try JSONEncoder().encode(body)
        }
        return request
    }

    private static func send(_ request: URLRequest) async throws -> [Term] {
        let (data, response) = try await URLSession(configuration: .ephemeral).data(for: request)
        let status = (response as? HTTPURLResponse)?.statusCode ?? 0
        guard status == 200 else {
            // 400 and 413: the term itself (invalid, too long). 401 and the rest say nothing
            // about the word, so it waits for the next sync.
            if status == 400 || status == 413 { throw Rejected(status: status) }
            throw URLError(.badServerResponse)
        }
        return try JSONDecoder().decode(Response.self, from: data).terms
    }
}

/// The paired Hex's word list as last synced, and the words the keyboard queued for it.
@MainActor
@Observable
final class RemoteWordList {
    /// Empty while unpaired.
    private(set) var terms: [RemoteVocabulary.Term] = []
    private(set) var error: String?
    @ObservationIgnored private var syncing = false
    @ObservationIgnored private let keyboardBridge: KeyboardBridge

    init(keyboardBridge: KeyboardBridge) {
        self.keyboardBridge = keyboardBridge
    }

    /// Words still waiting for the paired Hex.
    var pending: [String] {
        keyboardBridge.pendingWords
    }

    /// Sends the words the keyboard queued to the paired Hex, then reads its list back.
    /// Queued words stay queued until the paired Hex takes them; a word it rejects is dropped,
    /// so it never holds back the words after it.
    func sync() async {
        guard !syncing, let paired = PairedHexStore.load() else { return }
        syncing = true
        defer { syncing = false }
        let queued = keyboardBridge.pendingWords
        let delivery = await RemoteVocabulary.deliver(queued) { word in
            try await RemoteVocabulary.add(word, on: paired.hex, token: paired.token)
        }
        keyboardBridge.removePendingWords(delivery.settled)
        if let terms = delivery.terms { self.terms = terms }
        guard delivery.failure == nil else {
            error = "\(paired.hex.name) didn't answer. Words you add wait until it does."
            return
        }
        do {
            if delivery.terms == nil {
                terms = try await RemoteVocabulary.list(on: paired.hex, token: paired.token)
            }
            error = delivery.rejected.isEmpty
                ? nil
                : "\(paired.hex.name) didn't take \(delivery.rejected.joined(separator: ", "))."
        } catch {
            self.error = "\(paired.hex.name) didn't answer. Words you add wait until it does."
        }
    }

    func add(_ word: String) async {
        let trimmed = word.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        keyboardBridge.queueWord(trimmed)
        await sync()
    }

    func remove(_ term: RemoteVocabulary.Term) async {
        guard let paired = PairedHexStore.load() else { return }
        do {
            terms = try await RemoteVocabulary.remove(term.term, on: paired.hex, token: paired.token)
            error = nil
        } catch {
            self.error = "\(paired.hex.name) didn't answer, so the word stays."
        }
    }

    func clear() {
        terms = []
        error = nil
    }
}
