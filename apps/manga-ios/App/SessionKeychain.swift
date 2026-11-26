import AkiManga
import Foundation
import Security

/// Keeps each source's session (what its site answered at sign-in) in the Keychain, on this iPhone only. The password
/// never gets here: the engine gives only the session, and takes it back when a source loads.
final class SessionKeychain: SessionKeeper {
    private let service = "live.akidev.manga.session"

    private func query(_ source: String) -> [String: Any] {
        [kSecClass as String: kSecClassGenericPassword, kSecAttrService as String: service, kSecAttrAccount as String: source]
    }

    func load(source: String) -> String? {
        var found: AnyObject?
        let status = SecItemCopyMatching(query(source).merging([kSecReturnData as String: true, kSecMatchLimit as String: kSecMatchLimitOne]) { $1 } as CFDictionary, &found)
        guard status == errSecSuccess, let data = found as? Data else { return nil }
        return String(data: data, encoding: .utf8)
    }

    func save(source: String, session: String?) {
        SecItemDelete(query(source) as CFDictionary)
        guard let session, let data = session.data(using: .utf8) else { return }
        // Readable after the first unlock so a background refresh can use it, and never copied to another device.
        let item = query(source).merging([kSecValueData as String: data, kSecAttrAccessible as String: kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly]) { $1 }
        SecItemAdd(item as CFDictionary, nil)
    }
}
