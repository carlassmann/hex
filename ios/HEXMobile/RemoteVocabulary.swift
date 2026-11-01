import Foundation

/// The paired Hex's word list (`/remote/vocabulary`). It is the one list that machine's
/// transcription and cleanup use, so a word added here also helps its own dictation.
enum RemoteVocabulary {
    struct Term: Codable, Hashable, Identifiable, Sendable {
        let term: String
        let soundsLike: [String]

        var id: String { term.lowercased() }
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
        guard (response as? HTTPURLResponse)?.statusCode == 200 else {
            throw URLError(.badServerResponse)
        }
        return try JSONDecoder().decode(Response.self, from: data).terms
    }
}
