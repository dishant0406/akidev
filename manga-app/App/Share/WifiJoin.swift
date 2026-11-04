import NetworkExtension

/// Joins the temporary Wi-Fi an Android phone made for one send, and leaves it afterwards. iOS asks the person once ("Join Wi-Fi
/// network?"); `joinOnce` makes the join end when the app is closed, so the phone does not keep a network it has no use for.
enum WifiJoin {
    struct Failed: Error {}

    static func join(ssid: String, password: String) async throws {
        let configuration = NEHotspotConfiguration(ssid: ssid, passphrase: password, isWEP: false)
        configuration.joinOnce = true
        do {
            try await NEHotspotConfigurationManager.shared.apply(configuration)
        } catch let error as NSError {
            // Already on that network is as good as joined.
            if error.domain == NEHotspotConfigurationErrorDomain, error.code == NEHotspotConfigurationError.alreadyAssociated.rawValue { return }
            throw Failed()
        }
    }

    static func leave(ssid: String) {
        NEHotspotConfigurationManager.shared.removeConfiguration(forSSID: ssid)
    }
}
