import AVFAudio
import XCTest
@testable import HEX

final class RemoteStreamTests: XCTestCase {
    func testDataStartSkipsChunksBeforeTheSamples() {
        var header = Data("RIFF".utf8) + le32(0) + Data("WAVE".utf8)
        header += Data("fmt ".utf8) + le32(16) + Data(count: 16)
        header += Data("FLLR".utf8) + le32(7) + Data(count: 8) // odd size, padded
        header += Data("data".utf8) + le32(3_200)

        let layout = WAVLayout.dataStart(in: header)

        XCTAssertEqual(layout?.offset, header.count)
        XCTAssertEqual(layout?.size, 3_200)
    }

    func testDataStartWaitsForAHeaderStillBeingWritten() {
        XCTAssertNil(WAVLayout.dataStart(in: Data("RIFF".utf8)))
        XCTAssertNil(WAVLayout.dataStart(in: Data("RIFF".utf8) + le32(0) + Data("WAVE".utf8)))
    }

    func testDataStartFindsTheSamplesOfAnAVFoundationWAV() throws {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString)
            .appendingPathExtension("wav")
        defer { try? FileManager.default.removeItem(at: url) }
        let settings: [String: Any] = [
            AVFormatIDKey: kAudioFormatLinearPCM,
            AVSampleRateKey: 16_000,
            AVNumberOfChannelsKey: 1,
            AVLinearPCMBitDepthKey: 16,
            AVLinearPCMIsBigEndianKey: false,
            AVLinearPCMIsFloatKey: false,
        ]
        let frames: AVAudioFrameCount = 16_000
        do {
            let file = try AVAudioFile(
                forWriting: url,
                settings: settings,
                commonFormat: .pcmFormatInt16,
                interleaved: true
            )
            let buffer = try XCTUnwrap(AVAudioPCMBuffer(pcmFormat: file.processingFormat, frameCapacity: frames))
            buffer.frameLength = frames
            try file.write(from: buffer)
        }

        let data = try Data(contentsOf: url)
        let layout = try XCTUnwrap(WAVLayout.dataStart(in: data.prefix(16 * 1024)))

        XCTAssertEqual(layout.size, Int(frames) * 2)
        XCTAssertEqual(data.count - layout.offset, layout.size)
    }

    private func le32(_ value: UInt32) -> Data {
        withUnsafeBytes(of: value.littleEndian) { Data($0) }
    }
}
