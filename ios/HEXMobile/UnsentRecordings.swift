import Foundation

/// Recordings no machine transcribed. They stay on the phone, out of the
/// temporary folder iOS may empty, until a retry turns them into text, so a
/// long dictation is never lost to a network or server failure.
enum UnsentRecordings {
    /// Old ones go once this many are kept.
    private static let limit = 10

    static var folder: URL {
        URL.applicationSupportDirectory.appendingPathComponent("Unsent recordings", isDirectory: true)
    }

    static func keep(_ recording: URL) -> URL? {
        let fileManager = FileManager.default
        guard fileManager.fileExists(atPath: recording.path) else { return nil }
        if recording.deletingLastPathComponent().standardizedFileURL == folder.standardizedFileURL {
            return recording
        }
        do {
            try fileManager.createDirectory(at: folder, withIntermediateDirectories: true)
            let kept = folder.appendingPathComponent(recording.lastPathComponent)
            try fileManager.moveItem(at: recording, to: kept)
            for old in all().dropFirst(limit) { try? fileManager.removeItem(at: old) }
            return kept
        } catch {
            NSLog("Hex couldn't keep the unsent recording: \(error.localizedDescription)")
            return nil
        }
    }

    static func latest() -> URL? {
        all().first
    }

    /// Newest first.
    private static func all() -> [URL] {
        let files = (try? FileManager.default.contentsOfDirectory(
            at: folder,
            includingPropertiesForKeys: [.contentModificationDateKey]
        )) ?? []
        return files
            .filter { $0.pathExtension == "wav" }
            .sorted { modified($0) > modified($1) }
    }

    private static func modified(_ url: URL) -> Date {
        (try? url.resourceValues(forKeys: [.contentModificationDateKey]).contentModificationDate) ?? .distantPast
    }
}
