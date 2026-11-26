import AVFAudio
import Foundation
import UIKit

/// This fork: while paired, a recording the keyboard makes by itself also goes to the paired
/// Hex as it is recorded (`/remote/streams`, `RemoteStream`), so only its last seconds are left
/// to transcribe when it stops. It takes the same chunks as the recording's own file
/// (`AudioSink.append` in `KeyboardLocalDictation`), resampled to the 16 kHz the stream
/// accepts, into a side file the stream reads as it grows. The recording's own file is
/// untouched, so a failed stream still uploads it whole (`KeyboardRemoteTranscription`).
@MainActor
final class KeyboardRemoteFeed {
    static let sampleRate: Double = 16_000

    private let file: KeyboardPCMFile
    private let captured: AVAudioFormat
    private let written: AVAudioFormat
    private let converter: AVAudioConverter?
    private let session: URLSession
    private let stream: RemoteStream
    private let machine: String
    private var sending = false
    private var closed = false

    /// A feed for a capture at `capturedSampleRate`, or nil when no Hex is paired.
    static func make(capturedSampleRate: Double) -> KeyboardRemoteFeed? {
        guard let paired = PairedHexStore.load() else { return nil }
        do {
            return try KeyboardRemoteFeed(hex: paired.hex, token: paired.token, capturedSampleRate: capturedSampleRate)
        } catch {
            KeyboardProbeLog.shared.write("stream: none for this dictation (\((error as NSError).localizedDescription))")
            return nil
        }
    }

    private init(hex: PairedHex, token: String, capturedSampleRate: Double) throws {
        guard let captured = AVAudioFormat(commonFormat: .pcmFormatInt16, sampleRate: capturedSampleRate, channels: 1, interleaved: true),
              let written = AVAudioFormat(commonFormat: .pcmFormatInt16, sampleRate: Self.sampleRate, channels: 1, interleaved: true)
        else { throw CocoaError(.featureUnsupported) }
        self.captured = captured
        self.written = written
        if capturedSampleRate == Self.sampleRate {
            converter = nil
        } else {
            guard let converter = AVAudioConverter(from: captured, to: written) else { throw CocoaError(.featureUnsupported) }
            self.converter = converter
        }
        let url = FileManager.default.temporaryDirectory.appendingPathComponent("keyboard-stream-\(UUID().uuidString).wav")
        file = try KeyboardPCMFile(url: url, sampleRate: Self.sampleRate)
        let configuration = URLSessionConfiguration.ephemeral
        // The longest gap between bytes; Hex sends progress every second while it works.
        configuration.timeoutIntervalForRequest = 15
        configuration.waitsForConnectivity = false
        session = URLSession(configuration: configuration)
        machine = hex.name
        stream = RemoteStream(hex: hex, token: token, audioURL: url, application: UIDevice.current.model, session: session)
    }

    func append(_ data: Data) {
        guard !closed else { return }
        guard let converter else {
            file.append(data)
            return
        }
        let count = data.count / 2
        guard count > 0, let input = AVAudioPCMBuffer(pcmFormat: captured, frameCapacity: AVAudioFrameCount(count)) else { return }
        input.frameLength = AVAudioFrameCount(count)
        data.withUnsafeBytes { raw in
            guard let base = raw.baseAddress, let target = input.int16ChannelData?[0] else { return }
            memcpy(target, base, count * 2)
        }
        let capacity = AVAudioFrameCount(Double(count) * Self.sampleRate / captured.sampleRate) + 32
        guard let output = AVAudioPCMBuffer(pcmFormat: written, frameCapacity: capacity) else { return }
        var consumed = false
        var error: NSError?
        converter.convert(to: output, error: &error) { _, status in
            if consumed {
                status.pointee = .noDataNow
                return nil
            }
            consumed = true
            status.pointee = .haveData
            return input
        }
        guard error == nil, output.frameLength > 0, let samples = output.int16ChannelData?[0] else { return }
        file.append(Data(bytes: samples, count: Int(output.frameLength) * 2))
    }

    /// Starts sending, from the file's start: only once the touch is kept, so a slide or a
    /// cancelled touch never reaches the paired Hex.
    func send() {
        guard !sending, !closed else { return }
        sending = true
        let stream = stream
        Task { await stream.start() }
        KeyboardProbeLog.shared.write("stream: sending to \(machine) while recording")
    }

    /// The recording stopped: the side file is complete.
    func close() {
        guard !closed else { return }
        closed = true
        _ = file.close()
    }

    /// The recording was thrown away: the paired Hex drops what it had.
    func cancel() {
        close()
        let stream = stream
        let session = session
        let url = file.url
        Task {
            await stream.cancel()
            session.finishTasksAndInvalidate()
            try? FileManager.default.removeItem(at: url)
        }
    }

    /// The paired Hex's text for the streamed recording, or nil when it never streamed or the
    /// stream failed, so the caller uploads the whole recording instead.
    func text() async throws -> String? {
        close()
        defer {
            session.finishTasksAndInvalidate()
            try? FileManager.default.removeItem(at: file.url)
        }
        guard sending else { return nil }
        let started = Date()
        do {
            let text = try await stream.finish(progress: { _ in })
            KeyboardProbeLog.shared.write("stream: \(machine) answered \(Int(Date().timeIntervalSince(started) * 1000)) ms after the recording stopped")
            return text
        } catch {
            if error is CancellationError || Task.isCancelled {
                let stream = stream
                Task { await stream.cancel() }
                throw CancellationError()
            }
            KeyboardProbeLog.shared.write("stream failed, uploading the whole recording: \((error as NSError).localizedDescription)")
            return nil
        }
    }
}
