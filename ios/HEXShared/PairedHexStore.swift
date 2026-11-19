import Foundation
import Security

/// A desktop Hex this iPhone paired with (`hex remote pair` on that machine). It transcribes
/// each recording with its own model and Modes, so the iPhone runs no model while it answers.
struct PairedHex: Codable, Equatable, Sendable {
    let url: URL
    let machineID: String
    let name: String
}

/// The paired Hex lives in the App Group's defaults and its device token in the App Group's
/// Keychain access group, so the keyboard transcribes there too when it records by itself.
enum PairedHexStore {
    private static let key = "remote.pairedHex"
    private static let fallbackKey = "remote.fallsBackToDevice"
    private static let keychainService = "com.carlassmann.hex.remote"
    private static var defaults: UserDefaults {
        UserDefaults(suiteName: KeyboardBridge.appGroup) ?? .standard
    }

    /// Whether a dictation the paired Hex can't take is transcribed on this iPhone instead
    /// of failing. On unless turned off.
    static var fallsBackToDevice: Bool {
        get { defaults.object(forKey: fallbackKey) as? Bool ?? true }
        set { defaults.set(newValue, forKey: fallbackKey) }
    }

    static func load() -> (hex: PairedHex, token: String)? {
        if let data = defaults.data(forKey: key),
           let hex = try? JSONDecoder().decode(PairedHex.self, from: data),
           let token = readToken(account: hex.machineID, shared: true) {
            return (hex, token)
        }
        return migrateFromAppOnlyStore()
    }

    static func save(_ hex: PairedHex, token: String) throws {
        try writeToken(token, account: hex.machineID)
        defaults.set(try JSONEncoder().encode(hex), forKey: key)
    }

    static func forget() {
        if let data = defaults.data(forKey: key),
           let hex = try? JSONDecoder().decode(PairedHex.self, from: data) {
            SecItemDelete(query(account: hex.machineID, shared: true) as CFDictionary)
        }
        defaults.removeObject(forKey: key)
    }

    /// Pairings made before the keyboard could read them lived in the app's own defaults and
    /// Keychain; the app moves them over once (the keyboard can't read the old place).
    private static func migrateFromAppOnlyStore() -> (hex: PairedHex, token: String)? {
        guard let data = UserDefaults.standard.data(forKey: key),
              let hex = try? JSONDecoder().decode(PairedHex.self, from: data),
              let token = readToken(account: hex.machineID, shared: false)
        else { return nil }
        if let fallsBack = UserDefaults.standard.object(forKey: fallbackKey) as? Bool {
            fallsBackToDevice = fallsBack
        }
        guard (try? save(hex, token: token)) != nil else { return (hex, token) }
        SecItemDelete(query(account: hex.machineID, shared: false) as CFDictionary)
        UserDefaults.standard.removeObject(forKey: key)
        UserDefaults.standard.removeObject(forKey: fallbackKey)
        return (hex, token)
    }

    private static func query(account: String, shared: Bool) -> [String: Any] {
        var query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: keychainService,
            kSecAttrAccount as String: account,
        ]
        if shared { query[kSecAttrAccessGroup as String] = KeyboardBridge.appGroup }
        return query
    }

    private static func readToken(account: String, shared: Bool) -> String? {
        var item: CFTypeRef?
        var search = query(account: account, shared: shared)
        search[kSecReturnData as String] = true
        guard SecItemCopyMatching(search as CFDictionary, &item) == errSecSuccess,
              let data = item as? Data
        else { return nil }
        return String(data: data, encoding: .utf8)
    }

    private static func writeToken(_ token: String, account: String) throws {
        SecItemDelete(query(account: account, shared: true) as CFDictionary)
        var item = query(account: account, shared: true)
        item[kSecValueData as String] = Data(token.utf8)
        item[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
        let status = SecItemAdd(item as CFDictionary, nil)
        guard status == errSecSuccess else {
            throw NSError(domain: NSOSStatusErrorDomain, code: Int(status))
        }
    }
}
