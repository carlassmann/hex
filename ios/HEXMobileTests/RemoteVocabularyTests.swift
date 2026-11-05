import XCTest
@testable import HEX

final class RemoteVocabularyTests: XCTestCase {
    private static func terms(_ words: String...) -> [RemoteVocabulary.Term] {
        words.map { RemoteVocabulary.Term(term: $0, soundsLike: []) }
    }

    func testARejectedWordIsDroppedAndTheRestStillSync() async {
        let delivery = await RemoteVocabulary.deliver(["Hex", "bad", "Parakeet"]) { word in
            if word == "bad" { throw RemoteVocabulary.Rejected(status: 400) }
            return Self.terms(word)
        }

        XCTAssertEqual(delivery.settled, ["Hex", "bad", "Parakeet"])
        XCTAssertEqual(delivery.rejected, ["bad"])
        XCTAssertEqual(delivery.terms, Self.terms("Parakeet"))
        XCTAssertNil(delivery.failure)
    }

    func testATransientFailureKeepsTheWordAndTheOnesAfterItQueued() async {
        let delivery = await RemoteVocabulary.deliver(["Hex", "Parakeet", "Orukeet"]) { word in
            if word == "Parakeet" { throw URLError(.timedOut) }
            return Self.terms(word)
        }

        XCTAssertEqual(delivery.settled, ["Hex"])
        XCTAssertEqual(delivery.rejected, [])
        XCTAssertEqual(delivery.terms, Self.terms("Hex"))
        XCTAssertNotNil(delivery.failure)
    }
}
