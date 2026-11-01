import SwiftUI
import UIKit

struct ContentView: View {
    @Environment(\.openURL) private var openURL
    @Environment(\.scenePhase) private var scenePhase
    @Bindable var dictation: DictationController
    @Binding var openedFromKeyboard: Bool
    @Binding var showSetup: Bool
    @State private var setupMonitor = KeyboardSetupMonitor()
    @State private var practiceText = ""
    @State private var showReturnAppsEditor = false
    @FocusState private var practiceFocused: Bool

    var body: some View {
        ZStack {
            HexTheme.bgDeep
                .ignoresSafeArea()

            if openedFromKeyboard {
                keyboardHandoffView
                    .transition(.opacity)
            } else {
                standaloneView
                    .transition(.opacity)
            }
        }
        .preferredColorScheme(.dark)
        .animation(.easeOut(duration: 0.16), value: openedFromKeyboard)
    }

    // MARK: - Standalone App Screen

    private var standaloneView: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                headerRow

                if showSetup || !setupComplete {
                    KeyboardSetupCard(dictation: dictation, monitor: setupMonitor)
                }

                dictationStageCard

                if !dictation.transcript.isEmpty {
                    transcriptCard
                }

                if case .failed(let message) = dictation.phase {
                    errorBanner(message)
                }

                keyboardSettingsCard
                shortcutsCard
            }
            .frame(maxWidth: 520)
            .padding(.horizontal, 20)
            .padding(.top, 16)
            .padding(.bottom, 32)
            .frame(maxWidth: .infinity)
        }
        .scrollDismissesKeyboard(.interactively)
        .task {
            while !Task.isCancelled {
                setupMonitor.refresh()
                try? await Task.sleep(for: .seconds(1))
            }
        }
        .onChange(of: scenePhase) { _, phase in
            if phase == .active {
                setupMonitor.refresh()
            }
        }
    }

    private var setupComplete: Bool {
        setupMonitor.keyboardReady
            && setupMonitor.microphone == .granted
            && dictation.isModelReady
    }

    private var headerRow: some View {
        HStack(alignment: .center, spacing: 12) {
            Text("Hex")
                .font(.system(size: 20, weight: .semibold))
                .tracking(-0.3)
                .foregroundStyle(HexTheme.textBase)

            Spacer()

            if dictation.keyboardSessionEnabled {
                Button {
                    dictation.endWarmSession()
                } label: {
                    HStack(spacing: 6) {
                        HexStatusDot(color: HexTheme.warmMic, size: 7)
                        Text("Warm · \(HexTheme.formatRemaining(dictation.keyboardSessionRemaining))")
                            .font(.system(size: 12, weight: .medium))
                            .monospacedDigit()
                            .foregroundStyle(HexTheme.textBase)
                        Text("End")
                            .font(.system(size: 12, weight: .medium))
                            .foregroundStyle(HexTheme.textMuted)
                    }
                    .padding(.horizontal, 10)
                    .padding(.vertical, 6)
                    .background(HexTheme.layer01, in: Capsule())
                    .overlay(Capsule().stroke(HexTheme.borderWeak, lineWidth: 1))
                }
                .buttonStyle(.plain)
                .accessibilityLabel("End warm microphone session")
            } else {
                Text("\(dictation.engineName) · \(dictation.transcriptionLanguage)")
                    .font(.system(size: 12, weight: .medium))
                    .foregroundStyle(HexTheme.textFaint)
                    .lineLimit(1)
            }
        }
    }

    private var dictationStageCard: some View {
        VStack(alignment: .leading, spacing: 14) {
            if dictation.isRecording {
                HStack(spacing: 10) {
                    HexStatusDot(color: HexTheme.danger, size: 9)
                    Text(HexTheme.formatClock(dictation.recordingDuration))
                        .font(.system(size: 15, weight: .semibold))
                        .monospacedDigit()
                        .foregroundStyle(HexTheme.textBase)

                    Spacer(minLength: 8)

                    DotScopeBar(
                        level: dictation.audioLevel,
                        barCount: 18,
                        active: true,
                        height: 22
                    )

                    Spacer(minLength: 8)

                    Button("Cancel") {
                        dictation.cancelCurrentJob()
                    }
                    .font(.system(size: 13, weight: .medium))
                    .foregroundStyle(HexTheme.textMuted)
                    .padding(.horizontal, 12)
                    .frame(height: 36)
                    .background(HexTheme.layer02, in: Capsule())
                    .buttonStyle(.plain)

                    Button("Finish") {
                        dictation.finishRecording()
                    }
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundStyle(HexTheme.textContrast)
                    .padding(.horizontal, 14)
                    .frame(height: 36)
                    .background(HexTheme.bgContrast, in: Capsule())
                    .buttonStyle(.plain)
                }
            } else {
                HStack(spacing: 12) {
                    VStack(alignment: .leading, spacing: 3) {
                        Text(stageHeadline)
                            .font(.system(size: 16, weight: .medium))
                            .foregroundStyle(HexTheme.textBase)

                        if let subtitle = stageSubtitle {
                            Text(subtitle)
                                .font(.system(size: 13))
                                .foregroundStyle(HexTheme.textMuted)
                                .lineLimit(2)
                        }
                    }

                    Spacer(minLength: 12)

                    if dictation.phase.isBusy {
                        HStack(spacing: 8) {
                            ProgressView()
                                .controlSize(.small)
                                .tint(HexTheme.textMuted)
                            if dictation.phase == .transcribing {
                                Button("Cancel") {
                                    dictation.cancelCurrentJob()
                                }
                                .font(.system(size: 13, weight: .medium))
                                .foregroundStyle(HexTheme.textMuted)
                                .padding(.horizontal, 12)
                                .frame(height: 36)
                                .background(HexTheme.layer02, in: Capsule())
                                .buttonStyle(.plain)
                            }
                        }
                    } else {
                        Button(action: dictation.performPrimaryAction) {
                            HStack(spacing: 7) {
                                Image(systemName: primaryActionIcon)
                                    .font(.system(size: 13, weight: .semibold))
                                Text(primaryActionLabel)
                                    .font(.system(size: 14, weight: .semibold))
                            }
                            .foregroundStyle(HexTheme.textContrast)
                            .padding(.leading, 13)
                            .padding(.trailing, 15)
                            .frame(height: 38)
                            .background(HexTheme.bgContrast, in: Capsule())
                        }
                        .buttonStyle(.plain)
                    }
                }

                if dictation.phase == .preparingModel {
                    ProgressView(value: dictation.modelPreparation.fractionCompleted)
                        .tint(HexTheme.textBase)
                }
            }
        }
        .padding(16)
        .background(HexTheme.bgBase, in: RoundedRectangle(cornerRadius: HexTheme.cardRadius))
        .overlay(
            RoundedRectangle(cornerRadius: HexTheme.cardRadius)
                .stroke(HexTheme.borderWeak, lineWidth: 1)
        )
    }

    private var transcriptCard: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(alignment: .center) {
                sectionEyebrow("TRANSCRIPT")
                Spacer()

                HStack(spacing: 6) {
                    Button(dictation.copied ? "Copied" : "Copy") {
                        dictation.copyTranscript()
                    }
                    .font(.system(size: 12, weight: .medium))
                    .foregroundStyle(dictation.copied ? HexTheme.success : HexTheme.textBase)
                    .padding(.horizontal, 10)
                    .frame(height: 28)
                    .background(HexTheme.layer02, in: Capsule())

                    Button("Clear") {
                        dictation.clearTranscript()
                    }
                    .font(.system(size: 12, weight: .medium))
                    .foregroundStyle(HexTheme.textMuted)
                    .padding(.horizontal, 10)
                    .frame(height: 28)
                    .background(HexTheme.layer01, in: Capsule())
                }
                .buttonStyle(.plain)
            }

            Text(dictation.transcript)
                .font(.system(size: 16))
                .lineSpacing(3)
                .foregroundStyle(HexTheme.textBase)
                .textSelection(.enabled)
                .frame(maxWidth: .infinity, alignment: .leading)
        }
        .padding(16)
        .background(HexTheme.bgBase, in: RoundedRectangle(cornerRadius: HexTheme.cardRadius))
        .overlay(
            RoundedRectangle(cornerRadius: HexTheme.cardRadius)
                .stroke(HexTheme.borderWeak, lineWidth: 1)
        )
    }

    private var keyboardSettingsCard: some View {
        VStack(alignment: .leading, spacing: 16) {
            HStack(alignment: .center) {
                sectionEyebrow("KEYBOARD")
                Spacer()
                Button {
                    dictation.toggleWarmSession()
                } label: {
                    HStack(spacing: 6) {
                        HexStatusDot(
                            color: dictation.keyboardSessionEnabled ? HexTheme.warmMic : HexTheme.textFaint,
                            size: 6
                        )
                        Text(dictation.keyboardSessionEnabled ? "Warm now" : "Warm mic")
                            .font(.system(size: 12, weight: .medium))
                            .foregroundStyle(HexTheme.textBase)
                    }
                    .padding(.horizontal, 10)
                    .frame(height: 28)
                    .background(HexTheme.layer02, in: Capsule())
                }
                .buttonStyle(.plain)
            }

            // Warm session duration
            VStack(alignment: .leading, spacing: 8) {
                HStack {
                    Text("Keep microphone warm")
                        .font(.system(size: 13, weight: .medium))
                        .foregroundStyle(HexTheme.textBase)
                    Spacer()
                    Text(dictation.warmDuration.title)
                        .font(.system(size: 12))
                        .foregroundStyle(HexTheme.textMuted)
                }

                segmentedRow(
                    items: WarmSessionDuration.allCases,
                    selected: dictation.warmDuration,
                    label: \.shortLabel,
                    onSelect: dictation.setWarmDuration
                )

                Text("Keeps the orange mic indicator on so keyboard dictation starts without switching apps.")
                    .font(.system(size: 12))
                    .foregroundStyle(HexTheme.textFaint)
                    .fixedSize(horizontal: false, vertical: true)
            }

            dividerLine

            // Style preset
            VStack(alignment: .leading, spacing: 8) {
                HStack {
                    Text("Style")
                        .font(.system(size: 13, weight: .medium))
                        .foregroundStyle(HexTheme.textBase)
                    Spacer()
                    Text(dictation.style.shortHint)
                        .font(.system(size: 12))
                        .foregroundStyle(HexTheme.textMuted)
                        .lineLimit(1)
                }

                segmentedRow(
                    items: DictationStyle.allCases,
                    selected: dictation.style,
                    label: \.label,
                    onSelect: dictation.setStyle
                )
            }

            dividerLine

            // Auto-return target
            HStack(spacing: 12) {
                VStack(alignment: .leading, spacing: 2) {
                    Text("Cold-start return")
                        .font(.system(size: 13, weight: .medium))
                        .foregroundStyle(HexTheme.textBase)
                    Text("Apps without a URL scheme use the bottom-edge swipe")
                        .font(.system(size: 12))
                        .foregroundStyle(HexTheme.textFaint)
                }

                Spacer(minLength: 8)

                Menu {
                    Button {
                        dictation.setReturnAppID(nil)
                    } label: {
                        if dictation.returnAppID == nil {
                            Label("Swipe back (manual)", systemImage: "checkmark")
                        } else {
                            Text("Swipe back (manual)")
                        }
                    }

                    Divider()

                    ForEach(dictation.quickReturnApps) { app in
                        Button {
                            dictation.setReturnAppID(app.id)
                        } label: {
                            if dictation.returnAppID == app.id {
                                Label(app.name, systemImage: "checkmark")
                            } else {
                                Text(app.name)
                            }
                        }
                    }

                    Divider()

                    Button("Edit apps…") { showReturnAppsEditor = true }
                } label: {
                    HStack(spacing: 5) {
                        Text(dictation.selectedReturnApp?.name ?? "Swipe back")
                            .font(.system(size: 13, weight: .medium))
                            .foregroundStyle(HexTheme.textBase)
                        Image(systemName: "chevron.up.chevron.down")
                            .font(.system(size: 10, weight: .semibold))
                            .foregroundStyle(HexTheme.textMuted)
                    }
                    .padding(.horizontal, 11)
                    .frame(height: 32)
                    .background(HexTheme.layer02, in: RoundedRectangle(cornerRadius: 10))
                }
            }
            .sheet(isPresented: $showReturnAppsEditor) {
                ReturnAppsSheet(
                    chosen: dictation.quickReturnApps,
                    onSave: dictation.setQuickReturnApps
                )
            }

            dividerLine

            RemoteHexSection(dictation: dictation)

            dividerLine

            // Bluetooth input toggle
            Toggle(
                isOn: Binding(
                    get: { dictation.allowBluetoothMic },
                    set: { dictation.setAllowBluetoothMic($0) }
                )
            ) {
                VStack(alignment: .leading, spacing: 2) {
                    Text("Use Bluetooth headphones")
                        .font(.system(size: 13, weight: .medium))
                        .foregroundStyle(HexTheme.textBase)
                    Text("Off keeps audio on the built-in iPhone mic")
                        .font(.system(size: 12))
                        .foregroundStyle(HexTheme.textFaint)
                }
            }
            .tint(HexTheme.warmMic)

            dividerLine

            // Practice field + System Keyboard Settings
            VStack(alignment: .leading, spacing: 10) {
                HStack {
                    Text("Test keyboard")
                        .font(.system(size: 13, weight: .medium))
                        .foregroundStyle(HexTheme.textBase)
                    Spacer()
                    Button {
                        if let url = URL(string: UIApplication.openSettingsURLString) {
                            openURL(url)
                        }
                    } label: {
                        HStack(spacing: 4) {
                            Text("Keyboard settings")
                            Image(systemName: "arrow.up.right")
                                .font(.system(size: 10, weight: .semibold))
                        }
                        .font(.system(size: 12, weight: .medium))
                        .foregroundStyle(HexTheme.textMuted)
                    }
                    .buttonStyle(.plain)
                }

                TextField("Tap here, switch to 🌐 Hex Keyboard, and dictate…", text: $practiceText, axis: .vertical)
                    .font(.system(size: 14))
                    .foregroundStyle(HexTheme.textBase)
                    .focused($practiceFocused)
                    .lineLimit(2...4)
                    .padding(12)
                    .background(HexTheme.layer01, in: RoundedRectangle(cornerRadius: HexTheme.controlRadius))
                    .overlay(
                        RoundedRectangle(cornerRadius: HexTheme.controlRadius)
                            .stroke(practiceFocused ? HexTheme.borderStrong : HexTheme.borderWeak, lineWidth: 1)
                    )
            }
        }
        .padding(16)
        .background(HexTheme.bgBase, in: RoundedRectangle(cornerRadius: HexTheme.cardRadius))
        .overlay(
            RoundedRectangle(cornerRadius: HexTheme.cardRadius)
                .stroke(HexTheme.borderWeak, lineWidth: 1)
        )
    }

    private var shortcutsCard: some View {
        HStack(spacing: 12) {
            Image(systemName: "button.vertical.left.press")
                .font(.system(size: 15, weight: .medium))
                .foregroundStyle(HexTheme.textMuted)
                .frame(width: 32, height: 32)
                .background(HexTheme.layer01, in: RoundedRectangle(cornerRadius: 9))

            VStack(alignment: .leading, spacing: 2) {
                Text("Action Button & Control Center")
                    .font(.system(size: 13, weight: .medium))
                    .foregroundStyle(HexTheme.textBase)
                Text("Assign “Dictate with Hex” to record with a Live Activity and copy on stop")
                    .font(.system(size: 12))
                    .foregroundStyle(HexTheme.textFaint)
            }

            Spacer(minLength: 0)
        }
        .padding(14)
        .background(HexTheme.bgBase, in: RoundedRectangle(cornerRadius: HexTheme.cardRadius))
        .overlay(
            RoundedRectangle(cornerRadius: HexTheme.cardRadius)
                .stroke(HexTheme.borderWeak, lineWidth: 1)
        )
    }

    // MARK: - Keyboard Handoff Screen ("Hex is on")

    private var keyboardHandoffView: some View {
        VStack(spacing: 0) {
            HStack(alignment: .center) {
                Text("Hex")
                    .font(.system(size: 18, weight: .semibold))
                    .tracking(-0.3)
                    .foregroundStyle(HexTheme.textBase)

                Spacer()

                Button("Open app") {
                    openedFromKeyboard = false
                }
                .font(.system(size: 13, weight: .medium))
                .foregroundStyle(HexTheme.textMuted)
                .padding(.horizontal, 12)
                .frame(height: 30)
                .background(HexTheme.layer01, in: Capsule())
                .buttonStyle(.plain)
            }

            Spacer()

            VStack(spacing: 20) {
                handoffStatusPill

                VStack(spacing: 8) {
                    Text(handoffHeadline)
                        .font(.system(size: 24, weight: .semibold))
                        .tracking(-0.4)
                        .foregroundStyle(HexTheme.textBase)
                        .multilineTextAlignment(.center)

                    Text(handoffSubtitle)
                        .font(.system(size: 15))
                        .foregroundStyle(HexTheme.textMuted)
                        .multilineTextAlignment(.center)
                        .fixedSize(horizontal: false, vertical: true)
                }

                if dictation.phase == .preparingModel {
                    ProgressView(value: dictation.modelPreparation.fractionCompleted)
                        .tint(HexTheme.textBase)
                        .frame(maxWidth: 220)
                }

                if dictation.isRecording {
                    HStack(spacing: 8) {
                        Button("Cancel") {
                            dictation.cancelCurrentJob()
                        }
                        .font(.system(size: 14, weight: .medium))
                        .foregroundStyle(HexTheme.textMuted)
                        .padding(.horizontal, 16)
                        .frame(height: 40)
                        .background(HexTheme.layer01, in: Capsule())

                        Button("Finish here") {
                            dictation.finishRecording()
                        }
                        .font(.system(size: 14, weight: .semibold))
                        .foregroundStyle(HexTheme.textContrast)
                        .padding(.horizontal, 18)
                        .frame(height: 40)
                        .background(HexTheme.bgContrast, in: Capsule())
                    }
                    .buttonStyle(.plain)
                }

                if case .failed = dictation.phase {
                    Button("Try again") {
                        dictation.prepareModel()
                    }
                    .font(.system(size: 14, weight: .semibold))
                    .foregroundStyle(HexTheme.textContrast)
                    .padding(.horizontal, 18)
                    .frame(height: 40)
                    .background(HexTheme.bgContrast, in: Capsule())
                    .buttonStyle(.plain)
                }
            }
            .frame(maxWidth: 360)

            Spacer()

            if dictation.isRecording || dictation.keyboardSessionEnabled {
                VStack(spacing: 14) {
                    // Quick one-tap URL-scheme return chips (App Store-safe)
                    VStack(spacing: 8) {
                        Text("JUMP BACK TO")
                            .font(.system(size: 11, weight: .semibold))
                            .tracking(0.6)
                            .foregroundStyle(HexTheme.textFaint)

                        ScrollView(.horizontal, showsIndicators: false) {
                            HStack(spacing: 6) {
                                ForEach(dictation.quickReturnApps) { app in
                                    let isSelected = dictation.returnAppID == app.id
                                    Button {
                                        dictation.returnToHostApp(app, remember: true)
                                    } label: {
                                        HStack(spacing: 5) {
                                            Text(app.name)
                                                .font(.system(size: 13, weight: .medium))
                                            Image(systemName: "arrow.up.forward")
                                                .font(.system(size: 10, weight: .semibold))
                                        }
                                        .foregroundStyle(isSelected ? HexTheme.textBase : HexTheme.textMuted)
                                        .padding(.horizontal, 12)
                                        .frame(height: 34)
                                        .background(isSelected ? HexTheme.layer02 : HexTheme.layer01, in: Capsule())
                                        .overlay(
                                            Capsule().stroke(
                                                isSelected ? HexTheme.borderStrong : HexTheme.borderWeak,
                                                lineWidth: 1
                                            )
                                        )
                                    }
                                    .buttonStyle(.plain)
                                }
                            }
                            .padding(.horizontal, 4)
                        }
                    }

                    // Universal bottom-edge swipe guide
                    HStack(spacing: 8) {
                        Image(systemName: "arrow.right")
                            .font(.system(size: 13, weight: .semibold))
                            .foregroundStyle(HexTheme.textBase)
                        Text("Or swipe right along the bottom edge")
                            .font(.system(size: 14, weight: .medium))
                            .foregroundStyle(HexTheme.textBase)
                    }
                    .padding(.horizontal, 18)
                    .frame(height: 44)
                    .background(HexTheme.bgBase, in: Capsule())
                    .overlay(Capsule().stroke(HexTheme.borderStrong, lineWidth: 1))

                    if dictation.keyboardSessionEnabled {
                        Text("Stays warm for \(HexTheme.formatRemaining(dictation.keyboardSessionRemaining)) · no app switch next time")
                            .font(.system(size: 12))
                            .monospacedDigit()
                            .foregroundStyle(HexTheme.textFaint)
                    }
                }
            }
        }
        .padding(.horizontal, 20)
        .padding(.top, 16)
        .padding(.bottom, 18)
    }

    @ViewBuilder
    private var handoffStatusPill: some View {
        if dictation.isRecording {
            HStack(spacing: 10) {
                HexStatusDot(color: HexTheme.danger, size: 9)
                Text(HexTheme.formatClock(dictation.recordingDuration))
                    .font(.system(size: 14, weight: .semibold))
                    .monospacedDigit()
                    .foregroundStyle(HexTheme.textBase)
                DotScopeBar(level: dictation.audioLevel, barCount: 14, active: true, height: 18)
            }
            .padding(.horizontal, 16)
            .frame(height: 42)
            .background(HexTheme.bgBase, in: Capsule())
            .overlay(Capsule().stroke(HexTheme.borderStrong, lineWidth: 1))
        } else if dictation.keyboardSessionEnabled {
            HStack(spacing: 8) {
                HexStatusDot(color: HexTheme.warmMic, size: 8)
                Text("Warm · \(HexTheme.formatRemaining(dictation.keyboardSessionRemaining))")
                    .font(.system(size: 14, weight: .medium))
                    .monospacedDigit()
                    .foregroundStyle(HexTheme.textBase)
            }
            .padding(.horizontal, 16)
            .frame(height: 40)
            .background(HexTheme.bgBase, in: Capsule())
            .overlay(Capsule().stroke(HexTheme.borderWeak, lineWidth: 1))
        } else if case .failed = dictation.phase {
            Image(systemName: "exclamationmark.triangle.fill")
                .font(.system(size: 20, weight: .semibold))
                .foregroundStyle(HexTheme.danger)
                .frame(width: 48, height: 48)
                .background(HexTheme.dangerSurface, in: Circle())
        } else {
            ProgressView()
                .tint(HexTheme.textBase)
                .frame(width: 48, height: 48)
                .background(HexTheme.bgBase, in: Circle())
        }
    }

    // MARK: - Helpers

    private func sectionEyebrow(_ title: String) -> some View {
        Text(title)
            .font(.system(size: 11, weight: .semibold))
            .tracking(0.6)
            .foregroundStyle(HexTheme.textFaint)
    }

    private var dividerLine: some View {
        Rectangle()
            .fill(HexTheme.borderWeak)
            .frame(height: 1)
    }

    private func segmentedRow<Item: Hashable>(
        items: [Item],
        selected: Item,
        label: KeyPath<Item, String>,
        onSelect: @escaping (Item) -> Void
    ) -> some View {
        HStack(spacing: 4) {
            ForEach(items, id: \.self) { item in
                let isSelected = item == selected
                Button {
                    onSelect(item)
                } label: {
                    Text(item[keyPath: label])
                        .font(.system(size: 13, weight: isSelected ? .semibold : .medium))
                        .foregroundStyle(isSelected ? HexTheme.textBase : HexTheme.textMuted)
                        .frame(maxWidth: .infinity)
                        .frame(height: 32)
                        .background(
                            isSelected ? HexTheme.layer02 : Color.clear,
                            in: RoundedRectangle(cornerRadius: 8)
                        )
                        .contentShape(RoundedRectangle(cornerRadius: 8))
                }
                .buttonStyle(.plain)
            }
        }
        .padding(3)
        .background(HexTheme.layer01, in: RoundedRectangle(cornerRadius: 11))
    }

    private func errorBanner(_ message: String) -> some View {
        HStack(spacing: 10) {
            Image(systemName: "exclamationmark.circle.fill")
                .font(.system(size: 15))
                .foregroundStyle(HexTheme.danger)
            Text(message)
                .font(.system(size: 13))
                .foregroundStyle(HexTheme.textBase)
            Spacer(minLength: 0)
        }
        .padding(14)
        .background(HexTheme.dangerSurface, in: RoundedRectangle(cornerRadius: 14))
        .overlay(
            RoundedRectangle(cornerRadius: 14)
                .stroke(HexTheme.danger.opacity(0.25), lineWidth: 1)
        )
    }

    private var stageHeadline: String {
        switch dictation.phase {
        case .modelRequired, .failed:
            "On-device speech needs setup"
        case .preparingModel:
            "Preparing \(dictation.engineName)…"
        case .ready:
            "Ready on this iPhone"
        case .requestingPermission:
            "Opening microphone…"
        case .recording:
            "Listening"
        case .transcribing:
            "Transcribing locally…"
        }
    }

    private var stageSubtitle: String? {
        switch dictation.phase {
        case .modelRequired:
            "Downloads once and runs privately offline"
        case .preparingModel:
            dictation.modelPreparation.detail
        case .ready:
            nil
        case .requestingPermission:
            "Allow microphone access when prompted"
        case .recording:
            nil
        case .transcribing:
            nil
        case .failed:
            nil
        }
    }

    private var primaryActionLabel: String {
        switch dictation.phase {
        case .modelRequired, .failed:
            "Prepare"
        default:
            "Dictate"
        }
    }

    private var primaryActionIcon: String {
        switch dictation.phase {
        case .modelRequired, .failed:
            "arrow.down"
        default:
            "mic.fill"
        }
    }

    private var handoffHeadline: String {
        if let notice = dictation.handoffNotice {
            return notice
        }
        if case .attempting(let app) = dictation.autoReturnState {
            return "Returning to \(app.name)…"
        }
        if dictation.isRecording {
            return "Hex is listening"
        }
        if dictation.keyboardSessionEnabled {
            return "Hex is on"
        }
        switch dictation.phase {
        case .failed:
            return "Couldn’t start Hex"
        case .requestingPermission:
            return "Allow microphone access"
        default:
            return "Starting Hex…"
        }
    }

    private var handoffSubtitle: String {
        if case .attempting(let app) = dictation.autoReturnState {
            return "Recording stays active in \(app.name)."
        }
        if dictation.isRecording {
            return "Return to your app and speak — tap Done in the keyboard when finished."
        }
        if dictation.keyboardSessionEnabled {
            return "Return to your app and tap Dictate in the keyboard."
        }
        switch dictation.phase {
        case .preparingModel:
            return dictation.modelPreparation.detail
        case .requestingPermission:
            return "Needed once so the keyboard can dictate."
        case .failed(let message):
            return message
        default:
            return "Preparing on-device speech."
        }
    }
}

#Preview {
    ContentView(
        dictation: DictationController.shared,
        openedFromKeyboard: .constant(false),
        showSetup: .constant(false)
    )
}

/// Pairs with a desktop Hex in the tailnet that then transcribes for this iPhone.
private struct RemoteHexSection: View {
    let dictation: DictationController
    @State private var address = ""
    @State private var code = ""
    @State private var pairing = false
    @State private var scanning = false
    @State private var fallsBackToDevice = PairedHexStore.fallsBackToDevice
    @State private var showingWordList = false
    @State private var failure: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            VStack(alignment: .leading, spacing: 2) {
                Text("Transcribe on a remote Hex")
                    .font(.system(size: 13, weight: .medium))
                    .foregroundStyle(HexTheme.textBase)
                Text(caption)
                    .font(.system(size: 12))
                    .foregroundStyle(failure == nil ? HexTheme.textFaint : .red)
            }
            if dictation.pairedHex != nil {
                Toggle("Fall back to this iPhone", isOn: $fallsBackToDevice)
                    .font(.system(size: 13))
                    .onChange(of: fallsBackToDevice) { _, on in PairedHexStore.fallsBackToDevice = on }
                Button(wordListLabel) { showingWordList = true }
                    .font(.system(size: 13, weight: .medium))
                Button("Unpair") { dictation.unpairHex() }
                    .font(.system(size: 13, weight: .medium))
            } else {
                if PairingScanner.isAvailable {
                    Button("Scan QR code") { scanning = true }
                        .font(.system(size: 13, weight: .medium))
                        .disabled(pairing)
                }
                field("cbook.tailnet.ts.net:47831", text: $address, keyboard: .URL)
                HStack(spacing: 8) {
                    field("Code", text: $code, keyboard: .asciiCapable)
                    Button(pairing ? "Pairing…" : "Pair", action: pair)
                        .font(.system(size: 13, weight: .medium))
                        .disabled(pairing || address.isEmpty || code.isEmpty)
                }
            }
        }
        .task(id: dictation.pairedHex) { await dictation.syncWordList() }
        .sheet(isPresented: $showingWordList) {
            WordListSheet(dictation: dictation)
        }
        .sheet(isPresented: $scanning) {
            PairingScanner { scannedAddress, scannedCode in
                scanning = false
                address = scannedAddress
                code = scannedCode
                pair()
            }
            .ignoresSafeArea()
        }
    }

    private var wordListLabel: String {
        switch dictation.wordList.count {
        case 0: "Word list"
        case 1: "Word list · 1 word"
        case let count: "Word list · \(count) words"
        }
    }

    private var caption: String {
        if let failure { return failure }
        if let hex = dictation.pairedHex {
            return fallsBackToDevice
                ? "\(hex.name) transcribes with its model and Modes. Falls back to this iPhone when it doesn't answer."
                : "\(hex.name) transcribes with its model and Modes. When it doesn't answer, dictation fails."
        }
        return "Run hex remote pair on a Mac or PC in your tailnet, then scan its QR code or enter its address and code."
    }

    private func field(_ placeholder: String, text: Binding<String>, keyboard: UIKeyboardType) -> some View {
        TextField(placeholder, text: text)
            .font(.system(size: 13))
            .textInputAutocapitalization(.never)
            .autocorrectionDisabled()
            .keyboardType(keyboard)
            .padding(.horizontal, 11)
            .frame(height: 32)
            .background(HexTheme.layer02, in: RoundedRectangle(cornerRadius: 10))
    }

    private func pair() {
        pairing = true
        failure = nil
        Task {
            do {
                try await dictation.pairWithHex(address: address, code: code)
                code = ""
            } catch {
                failure = error.localizedDescription
            }
            pairing = false
        }
    }
}

/// The paired Hex's word list: names and terms its transcription and cleanup spell right.
/// Select a word in any app and tap "Add to word list" on the Hex keyboard, or add it here.
private struct WordListSheet: View {
    let dictation: DictationController
    @State private var newWord = ""
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            List {
                Section {
                    HStack {
                        TextField("Add a word or name", text: $newWord)
                            .textInputAutocapitalization(.never)
                            .autocorrectionDisabled()
                            .onSubmit(add)
                        Button("Add", action: add)
                            .disabled(newWord.trimmingCharacters(in: .whitespaces).isEmpty)
                    }
                } footer: {
                    Text(dictation.wordListError
                        ?? "Synced with \(dictation.pairedHex?.name ?? "your Hex"). In any app, select a word and tap Add to word list on the Hex keyboard.")
                }
                if !dictation.pendingWords.isEmpty {
                    Section("Waiting to sync") {
                        ForEach(dictation.pendingWords, id: \.self) { Text($0).foregroundStyle(.secondary) }
                    }
                }
                Section {
                    ForEach(dictation.wordList) { term in
                        VStack(alignment: .leading, spacing: 2) {
                            Text(term.term)
                            if !term.soundsLike.isEmpty {
                                Text("Sounds like \(term.soundsLike.joined(separator: ", "))")
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                            }
                        }
                    }
                    .onDelete { offsets in
                        let terms = offsets.map { dictation.wordList[$0] }
                        Task { for term in terms { await dictation.removeWord(term) } }
                    }
                }
            }
            .navigationTitle("Word list")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } }
            }
            .refreshable { await dictation.syncWordList() }
            .task { await dictation.syncWordList() }
        }
        .preferredColorScheme(.dark)
    }

    private func add() {
        let word = newWord
        newWord = ""
        Task { await dictation.addWord(word) }
    }
}
