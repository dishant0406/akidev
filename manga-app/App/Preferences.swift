import Foundation
import Observation
import SwiftUI

enum Appearance: String, CaseIterable {
    case system, light, dark
    var title: String { rawValue.capitalized }
    var scheme: ColorScheme? { self == .light ? .light : self == .dark ? .dark : nil }
}

/// What the person chose in Settings, kept in UserDefaults so it survives launches.
@MainActor @Observable
final class Preferences {
    private let defaults = UserDefaults.standard

    var appearance: Appearance { didSet { defaults.set(appearance.rawValue, forKey: "appearance") } }
    /// Sources marked 18+ are hidden until this is on.
    var showAdult: Bool { didSet { defaults.set(showAdult, forKey: "showAdult") } }
    /// Languages to keep (empty is all).
    var languages: [String] { didSet { defaults.set(languages, forKey: "languages") } }
    var defaultMode: ReadingMode { didSet { defaults.set(defaultMode.rawValue, forKey: "defaultMode") } }
    /// Swap which screen edge goes forward.
    var invertTaps: Bool { didSet { defaults.set(invertTaps, forKey: "invertTaps") } }
    var keepAwake: Bool { didSet { defaults.set(keepAwake, forKey: "keepAwake") } }
    var wifiOnly: Bool { didSet { defaults.set(wifiOnly, forKey: "wifiOnly") } }
    var deleteWhenRead: Bool { didSet { defaults.set(deleteWhenRead, forKey: "deleteWhenRead") } }
    var notifyUpdates: Bool { didSet { defaults.set(notifyUpdates, forKey: "notifyUpdates") } }

    init() {
        let defaults = UserDefaults.standard
        appearance = Appearance(rawValue: defaults.string(forKey: "appearance") ?? "") ?? .system
        showAdult = defaults.bool(forKey: "showAdult")
        languages = defaults.stringArray(forKey: "languages") ?? []
        defaultMode = ReadingMode(rawValue: defaults.string(forKey: "defaultMode") ?? "") ?? .strip
        invertTaps = defaults.bool(forKey: "invertTaps")
        keepAwake = defaults.bool(forKey: "keepAwake")
        wifiOnly = defaults.object(forKey: "wifiOnly") as? Bool ?? true
        deleteWhenRead = defaults.bool(forKey: "deleteWhenRead")
        notifyUpdates = defaults.bool(forKey: "notifyUpdates")
    }
}
