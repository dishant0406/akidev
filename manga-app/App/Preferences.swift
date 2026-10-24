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
    var color: Color { self == .pitch ? Aki.stage : Aki.stageAlt }
}

enum OrientationLock: String, CaseIterable {
    case auto, portrait, landscape
    var title: String { rawValue.capitalized }
    var mask: UIInterfaceOrientationMask { self == .portrait ? .portrait : self == .landscape ? .landscape : .allButUpsideDown }
}

enum LibrarySort: String, CaseIterable {
    case recent = "Last read", added = "Date added", title = "Title", unread = "Unread"
    case chapters = "Total chapters", latest = "Latest chapter", random = "Random"
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
    /// How fast the reader's strip scrolls when set going.
    var autoScrollSpeed: AutoScrollSpeed { didSet { defaults.set(autoScrollSpeed.rawValue, forKey: "autoScrollSpeed") } }
    /// How the library is ordered, and whether that order is turned round.
    var librarySort: LibrarySort { didSet { defaults.set(librarySort.rawValue, forKey: "librarySort") } }
    var libraryReversed: Bool { didSet { defaults.set(libraryReversed, forKey: "libraryReversed") } }
    /// What saving a series does about categories: nothing (0), ask which (-1), or put it on that category (its id).
    var saveCategory: Int64 { didSet { defaults.set(saveCategory, forKey: "saveCategory") } }
    /// How books look: the type, the margins and the theme.
    var bookStyle: BookStyle { didSet { defaults.set(try? JSONEncoder().encode(bookStyle), forKey: "bookStyle") } }
    var keepAwake: Bool { didSet { defaults.set(keepAwake, forKey: "keepAwake") } }
    /// A notification when a saved series gets new chapters (the person is asked for permission when turning it on).
    var notifyUpdates: Bool { didSet { defaults.set(notifyUpdates, forKey: "notifyUpdates") } }
    /// Keep new chapters of saved series on the device without being asked.
    var autoDownload: Bool { didSet { defaults.set(autoDownload, forKey: "autoDownload") } }
    /// Auto downloads wait for Wi-Fi: on unless the person says otherwise.
    var downloadOnWifiOnly: Bool { didSet { defaults.set(downloadOnWifiOnly, forKey: "downloadOnWifiOnly") } }
    /// The voice model that reads books aloud (its id); empty until one is chosen.
    var voiceModel: String { didSet { defaults.set(voiceModel, forKey: "voiceModel") } }
    /// The voice of that model that reads (its id); empty is the model's own.
    var voiceName: String { didSet { defaults.set(voiceName, forKey: "voiceName") } }
    /// How fast a book is read aloud, as a multiple of the voice's own pace.
    var speechRate: Double { didSet { defaults.set(speechRate, forKey: "speechRate") } }

    init() {
        let defaults = UserDefaults.standard
        appearance = Appearance(rawValue: defaults.string(forKey: "appearance") ?? "") ?? .system
        showAdult = defaults.bool(forKey: "showAdult")
        languages = defaults.stringArray(forKey: "languages") ?? []
        defaultMode = ReadingMode(rawValue: defaults.string(forKey: "defaultMode") ?? "") ?? .strip
        invertTaps = defaults.bool(forKey: "invertTaps")
        readerBackground = ReaderBackground(rawValue: defaults.string(forKey: "readerBackground") ?? "") ?? .pitch
        orientation = OrientationLock(rawValue: defaults.string(forKey: "orientation") ?? "") ?? .auto
        bookStyle = defaults.data(forKey: "bookStyle").flatMap { try? JSONDecoder().decode(BookStyle.self, from: $0) } ?? BookStyle()
        keepAwake = defaults.bool(forKey: "keepAwake")
        saveCategory = Int64(defaults.integer(forKey: "saveCategory"))
        librarySort = LibrarySort(rawValue: defaults.string(forKey: "librarySort") ?? "") ?? .recent
        libraryReversed = defaults.bool(forKey: "libraryReversed")
        autoScrollSpeed = AutoScrollSpeed(rawValue: defaults.string(forKey: "autoScrollSpeed") ?? "") ?? .normal
        notifyUpdates = defaults.bool(forKey: "notifyUpdates")
        autoDownload = defaults.bool(forKey: "autoDownload")
        downloadOnWifiOnly = defaults.object(forKey: "downloadOnWifiOnly") as? Bool ?? true
        voiceModel = defaults.string(forKey: "voiceModel") ?? ""
        voiceName = defaults.string(forKey: "voiceName") ?? ""
        speechRate = defaults.object(forKey: "speechRate") as? Double ?? 1
    }
}
