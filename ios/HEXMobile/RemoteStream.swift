import Foundation

/// Sends a recording to the paired Hex while it is still being recorded
/// (`/remote/streams`), so a long dictation is mostly transcribed by the time
/// it ends and only its last seconds are left. It reads the PCM the recorder
/// appends to its WAV file, so the recorder never changes.
actor RemoteStream {
    enum Failure: LocalizedError {
        case refused(String)
        case ended(String, String)
        case noAnswer

        var errorDescription: String? {
            switch self {
            case .refused(let code): "Hex refused the stream (\(code))."
            case .ended(_, let message): message
            case .noAnswer: "Hex stopped answering."
            }
        }
    }

    private struct Created: Decodable {
        let id: Int
        let ownerToken: String
    }

    /// About 6 s of 16 kHz 16-bit audio, under the route's 256 KiB.
    private static let maxChunkBytes = 192 * 1024
    private static let pollInterval: Duration = .milliseconds(500)
    private static let heartbeatInterval: TimeInterval = 4

    private let hex: PairedHex
    private let token: String
    nonisolated let audioURL: URL
    private let application: String
    private let session: URLSession

    private var created: Created?
    private var dataStart: Int?
    private var sentBytes = 0
    private var lastSentAt = Date()
    private var finishing = false
    private var pump: Task<Void, Never>?
    private(set) var failure: Error?

    init(hex: PairedHex, token: String, audioURL: URL, application: String, session: URLSession) {
        self.hex = hex
        self.token = token
        self.audioURL = audioURL
        self.application = application
        self.session = session
    }

    func start() {
        pump = Task { await self.run() }
    }

    /// Stops sending, so whatever is still recorded goes in one last request.
    private func stopPump() async {
        finishing = true
        await pump?.value
        pump = nil
    }

    func cancel() async {
        await stopPump()
        guard let created else { return }
        var request = streamRequest(created, action: "cancel")
        request.timeoutInterval = 5
        _ = try? await session.data(for: request)
    }

    /// Sends the rest of the finished recording and waits for the text,
    /// reporting the paired Hex's progress.
    func finish(
        progress: @escaping @Sendable (TranscriptionProgress) -> Void
    ) async throws -> String {
        await stopPump()
        if let failure { throw failure }
        guard let created else { throw Failure.noAnswer }
        try await sendAvailable(final: true)

        let request = streamRequest(created, action: "finish")
        return try await RemoteProgressReader.read(
            request: request,
            session: session,
            machine: hex.name,
            progress: progress
        )
    }

    private func run() async {
        do {
            created = try await create()
            while !finishing {
                try await Task.sleep(for: Self.pollInterval)
                try await sendAvailable(final: false)
                if Date().timeIntervalSince(lastSentAt) > Self.heartbeatInterval, let created {
                    _ = try await session.data(for: streamRequest(created, action: "heartbeat"))
                    lastSentAt = Date()
                }
            }
        } catch {
            NSLog("Hex stream to \(hex.name) stopped: \(error.localizedDescription)")
            failure = error
        }
    }

    private func create() async throws -> Created {
        var request = URLRequest(url: hex.url.appendingPathComponent("remote/streams"))
        request.httpMethod = "POST"
        request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = Data(#"{"sampleFormat":"s16le","sampleRate":16000}"#.utf8)
        let (data, response) = try await session.data(for: request)
        guard (response as? HTTPURLResponse)?.statusCode == 201 else {
            throw Failure.refused(Self.code(in: data) ?? "status \((response as? HTTPURLResponse)?.statusCode ?? 0)")
        }
        return try JSONDecoder().decode(Created.self, from: data)
    }

    private func streamRequest(_ created: Created, action: String) -> URLRequest {
        var request = URLRequest(
            url: hex.url.appendingPathComponent("remote/streams/\(created.id)/\(action)")
        )
        request.httpMethod = "POST"
        request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        request.setValue(created.ownerToken, forHTTPHeaderField: "X-Hex-Stream-Token")
        // Hex rewrites the text in pieces while it records, in this application's Mode.
        request.setValue(application, forHTTPHeaderField: "X-Hex-Application")
        return request
    }

    /// Sends the PCM recorded since the last call. While recording, the WAV's
    /// data chunk grows to the end of the file; once the recorder stopped, its
    /// header names the data's size.
    private func sendAvailable(final: Bool) async throws {
        guard let created else { return }
        let handle = try FileHandle(forReadingFrom: audioURL)
        defer { try? handle.close() }
        if dataStart == nil {
            guard let header = try handle.read(upToCount: 16 * 1024),
                  let start = WAVLayout.dataStart(in: header) else {
                if final { throw Failure.refused("invalid-wav") }
                return
            }
            dataStart = start.offset
        }
        guard let dataStart else { return }
        let fileEnd = Int(try handle.seekToEnd())
        var end = fileEnd
        if final {
            try handle.seek(toOffset: 0)
            if let header = try handle.read(upToCount: 16 * 1024),
               let declared = WAVLayout.dataStart(in: header)?.size, declared > 0 {
                end = min(fileEnd, dataStart + declared)
            }
        }

        while dataStart + sentBytes + 1 < end {
            let available = end - dataStart - sentBytes
            let length = min(available, Self.maxChunkBytes) & ~1
            guard length > 0 else { return }
            try handle.seek(toOffset: UInt64(dataStart + sentBytes))
            guard let chunk = try handle.read(upToCount: length), chunk.count == length else { return }

            var request = streamRequest(created, action: "audio")
            request.setValue("application/octet-stream", forHTTPHeaderField: "Content-Type")
            let (data, response) = try await session.upload(for: request, from: chunk)
            switch (response as? HTTPURLResponse)?.statusCode {
            case 204:
                sentBytes += length
                lastSentAt = Date()
            case 429:
                // Hex is behind; the audio stays in the file until it catches up.
                try await Task.sleep(for: .seconds(1))
            default:
                throw Failure.refused(Self.code(in: data) ?? "audio")
            }
            if !final && finishing { return }
        }
    }

    static func code(in data: Data) -> String? {
        (try? JSONSerialization.jsonObject(with: data) as? [String: Any])?["code"] as? String
    }
}

/// Where a WAV file's sample data starts, and the size its header declares.
enum WAVLayout {
    static func dataStart(in header: Data) -> (offset: Int, size: Int)? {
        let bytes = [UInt8](header)
        guard bytes.count >= 12,
              String(bytes: bytes[0..<4], encoding: .ascii) == "RIFF",
              String(bytes: bytes[8..<12], encoding: .ascii) == "WAVE" else {
            return nil
        }
        var offset = 12
        while offset + 8 <= bytes.count {
            let id = String(bytes: bytes[offset..<offset + 4], encoding: .ascii)
            let size = Int(bytes[offset + 4])
                | Int(bytes[offset + 5]) << 8
                | Int(bytes[offset + 6]) << 16
                | Int(bytes[offset + 7]) << 24
            if id == "data" {
                return (offset + 8, size)
            }
            offset += 8 + size + (size & 1)
        }
        return nil
    }
}

/// Reads the paired Hex's NDJSON progress (`src/local_api/remote_progress.rs`)
/// until its text. The session's request timeout is the gap between lines;
/// Hex sends one every second while it works, so a timeout means it is gone,
/// never that it is slow.
enum RemoteProgressReader {
    private struct Line: Decodable {
        let stage: String
        let text: String?
        let code: String?
        let message: String?
        let backlogMs: Int?
    }

    static func read(
        request: URLRequest,
        session: URLSession,
        machine: String,
        progress: @escaping @Sendable (TranscriptionProgress) -> Void
    ) async throws -> String {
        let (bytes, response) = try await session.bytes(for: request)
        let http = response as? HTTPURLResponse
        guard http?.statusCode == 200 else {
            var body = Data()
            for try await byte in bytes.prefix(4096) { body.append(byte) }
            throw RemoteStream.Failure.refused(RemoteStream.code(in: body) ?? "status \(http?.statusCode ?? 0)")
        }
        // A Hex from before progress answers plain JSON.
        if http?.value(forHTTPHeaderField: "Content-Type")?.hasPrefix("application/json") == true {
            var body = Data()
            for try await byte in bytes { body.append(byte) }
            guard let text = (try? JSONSerialization.jsonObject(with: body) as? [String: Any])?["text"] as? String else {
                throw RemoteStream.Failure.noAnswer
            }
            return text
        }
        for try await rawLine in bytes.lines {
            guard let line = try? JSONDecoder().decode(Line.self, from: Data(rawLine.utf8)) else { continue }
            switch line.stage {
            case "received", "transcribing":
                progress(TranscriptionProgress(
                    machine: machine,
                    stage: .transcribing,
                    audioLeftMs: line.backlogMs ?? 0
                ))
            case "rewriting":
                progress(TranscriptionProgress(machine: machine, stage: .rewriting))
            case "done":
                return line.text ?? ""
            case "failed":
                throw RemoteStream.Failure.ended(line.code ?? "failed", line.message ?? "\(machine) couldn't transcribe it.")
            default:
                continue
            }
        }
        throw RemoteStream.Failure.noAnswer
    }
}
