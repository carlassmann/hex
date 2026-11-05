import SwiftUI
import UIKit

@main
struct HEXMobileApp: App {
    @Environment(\.scenePhase) private var scenePhase
    @State private var dictation = DictationController.shared
    @State private var openedFromKeyboard = Self.initialOpenedFromKeyboard
    @State private var showSetup = SettingsSnapshotFixture.showsSetupSheet
    @State private var openPage: SettingsPage?
    /// Counts the keyboard's Open Hex taps from the Hex key menu: each one shows Home.
    @State private var homeRequests = 0
    /// A pair link waiting for you to confirm where your recordings would go.
    @State private var pendingPairing: (address: String, code: String)?

    init() {
        FullKeyboardSettings().applyLaunchArguments()
        TypingLogSettings().turnOffOnce()
        TypingLogSettings().applyLaunchArguments()
        HinglishSettings().applyLaunchArguments()
        GreeklishSettings().applyLaunchArguments()
        TwoLanguageDictationSettings().applyLaunchArguments()
        VoiceIsolationSettings().applyLaunchArguments()
        MultilingualParakeetSettings().applyLaunchArguments()
        KeyboardParakeetSettings().applyLaunchArguments()
        KeyboardParakeetSettings().forgetUnavailableWithoutReason()
        SettingsSnapshotFixture.applyDefaults()
        HexInbox.sweep()
    }

    private static var initialOpenedFromKeyboard: Bool {
        let args = ProcessInfo.processInfo.arguments
        guard let idx = args.firstIndex(of: "--snap-state"), idx + 1 < args.count else {
            return false
        }
        return args[idx + 1].hasPrefix("handoff-")
    }

    var body: some Scene {
        WindowGroup {
            if ProcessInfo.processInfo.environment["XCTestConfigurationFilePath"] != nil {
                Color.clear
            } else if let keyboardScene = KeyboardSnapshotStage.requestedScene {
                KeyboardSnapshotStage(scene: keyboardScene)
            } else if let fullKeyboardScene = FullKeyboardSnapshotStage.requestedScene {
                FullKeyboardSnapshotStage(scene: fullKeyboardScene)
            } else if let experimentsScene = ExperimentsSnapshotStage.requestedScene {
                ExperimentsSnapshotStage(scene: experimentsScene)
            } else if let hostedScene = HostedKeyboardConversationStage.requestedScene {
                HostedKeyboardConversationStage(scene: hostedScene, dictation: dictation)
            } else {
                ContentView(
                    dictation: dictation,
                    openedFromKeyboard: $openedFromKeyboard,
                    showSetup: $showSetup,
                    openPage: $openPage,
                    homeRequests: homeRequests
                )
                .onOpenURL { url in
                    // What's new waits for an open that no link asked for.
                    WhatsNew.openedByLink = true
                    // A Hex words file from AirDrop, Files or Open in Hex: its review sheet.
                    if HexInbox.open(url) { return }
                    // A link can come from anywhere, so it pairs only once you confirm its host.
                    if let link = RemoteHexPairing.link(url) {
                        pendingPairing = link
                        return
                    }
                    guard let request = KeyboardLaunchRequest(url: url) else { return }
                    if case .setup = request {
                        openedFromKeyboard = false
                        showSetup = true
                    } else if case .translationSettings = request {
                        openedFromKeyboard = false
                        openPage = .dictation
                    } else if case .home = request {
                        openedFromKeyboard = false
                        showSetup = false
                        homeRequests += 1
                    } else if case .pasteSettings = request {
                        openedFromKeyboard = false
                        openPage = .keyboard
                        if let settingsURL = URL(string: UIApplication.openSettingsURLString) {
                            UIApplication.shared.open(settingsURL)
                        }
                    } else {
                        openedFromKeyboard = true
                    }
                    dictation.handleKeyboardLaunch(request)
                }
                .alert(
                    pendingPairing.map { "Pair with \($0.address)?" } ?? "",
                    isPresented: Binding(get: { pendingPairing != nil }, set: { if !$0 { pendingPairing = nil } }),
                    presenting: pendingPairing
                ) { link in
                    Button("Pair") {
                        Task { try? await dictation.pairWithHex(address: link.address, code: link.code) }
                    }
                    Button("Cancel", role: .cancel) {}
                } message: { _ in
                    Text(dictation.pairedHex == nil
                         ? "Your recordings will be transcribed on that Hex."
                         : "It replaces the Hex you're paired with now, and your recordings go there instead.")
                }
                .onChange(of: scenePhase) { _, phase in
                    if phase == .background {
                        // The next open counts as plain again unless a link comes with it.
                        WhatsNew.openedByLink = false
                        openedFromKeyboard = false
                        dictation.appMovedToBackground()
                    } else if phase == .active {
                        dictation.endStaleLiveActivities()
                        dictation.appBecameActive()
                        // Settings › Dictation › Speech model in keyboard: picks its download up.
                        dictation.startKeyboardParakeetDownloadIfWanted()
                    }
                }
            }
        }
    }
}
