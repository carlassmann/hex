import SwiftUI

/// This fork's additions to the keyboard's top bar, kept out of `KeyboardSurface.swift`:
/// adding a selected word to the paired Hex's word list, and where a dictation is transcribed.
extension KeyboardViewModel {
    /// A short selection replaces the default top bar with the word-list row.
    var showsWordList: Bool {
        activeTray == .none && hasFullAccess && (selectedWord ?? addedWord) != nil
    }
}

struct KeyboardWordListBar: View {
    @ObservedObject var model: KeyboardViewModel
    let text: Color
    let mutedText: Color
    let fill: Color
    let reduceMotion: Bool

    var body: some View {
        content.hexTrayReveal(model.showsWordList, reduceMotion: reduceMotion, offsetY: 3.5)
    }

    @ViewBuilder
    private var content: some View {
        if let word = model.selectedWord, word != model.addedWord {
            Button {
                model.onAddWord?(word)
            } label: {
                HStack(spacing: 6) {
                    Image(systemName: "text.badge.plus")
                        .font(.system(size: 11, weight: .semibold))
                    Text("Add “\(word)” to word list")
                        .font(.system(size: 12, weight: .medium))
                        .lineLimit(1)
                }
                .foregroundStyle(text)
                .padding(.horizontal, 12)
                .frame(maxWidth: .infinity, minHeight: 26)
                .background(fill, in: Capsule())
            }
            .buttonStyle(.plain)
        } else if let word = model.addedWord {
            HStack(spacing: 6) {
                Image(systemName: "checkmark")
                    .font(.system(size: 11, weight: .semibold))
                Text("“\(word)” is in your word list")
                    .font(.system(size: 12, weight: .medium))
                    .lineLimit(1)
            }
            .foregroundStyle(mutedText)
            .frame(maxWidth: .infinity, minHeight: 26)
        }
    }
}

/// "Transcribing on <paired Hex>" in the stage while a dictation is transcribed.
struct KeyboardTranscriberStatus: View {
    @ObservedObject var model: KeyboardViewModel
    let color: Color
    let reduceMotion: Bool

    var body: some View {
        Text(model.transcriptionStatus ?? "Transcribing on \(model.transcriberName ?? "iPhone")")
            .font(.system(size: 13, weight: .regular))
            .foregroundStyle(color)
            .lineLimit(1)
            .hexTrayReveal(model.hasFullAccess && model.stagePhase == .transcribing, reduceMotion: reduceMotion, offsetY: 2)
    }
}
