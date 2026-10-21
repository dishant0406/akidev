import Foundation
import Observation
import SwiftUI

enum Appearance: String, CaseIterable {
    case system, light, dark
    var title: String { rawValue.capitalized }
    var scheme: ColorScheme? { self == .light ? .light : self == .dark ? .dark : nil }
}

enum ReaderBackground: String, CaseIterable {
    case pitch = "black", slate = "grey"
    var title: String { self == .pitch ? "Black" : "Dark grey" }
    var color: Color { self == .pitch ? Aki.readerBlack : Aki.readerGrey }
}

enum OrientationLock: String, CaseIterable {
    case auto, portrait, landscape
    var title: String { rawValue.capitalized }
    var mask: UIInterfaceOrientationMask { self == .portrait ? .portrait : self == .landscape ? .landscape : .allButUpsideDown }
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
    var readerBackground: ReaderBackground { didSet { defaults.set(readerBackground.rawValue, forKey: "readerBackground") } }
    var orientation: OrientationLock { didSet { defaults.set(orientation.rawValue, forKey: "orientation") } }
    var keepAwake: Bool { didSet { defaults.set(keepAwake, forKey: "keepAwake") } }

    init() {
        let defaults = UserDefaults.standard
        appearance = Appearance(rawValue: defaults.string(forKey: "appearance") ?? "") ?? .system
        showAdult = defaults.bool(forKey: "showAdult")
        languages = defaults.stringArray(forKey: "languages") ?? []
        defaultMode = ReadingMode(rawValue: defaults.string(forKey: "defaultMode") ?? "") ?? .strip
        invertTaps = defaults.bool(forKey: "invertTaps")
        readerBackground = ReaderBackground(rawValue: defaults.string(forKey: "readerBackground") ?? "") ?? .pitch
        orientation = OrientationLock(rawValue: defaults.string(forKey: "orientation") ?? "") ?? .auto
        keepAwake = defaults.bool(forKey: "keepAwake")
    }
}
