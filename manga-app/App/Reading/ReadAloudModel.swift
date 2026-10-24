import AkiManga
import Observation
import SwiftUI

/// Reading aloud: the voice models the app can get, the ones on the device and the one in use.
///
/// Models come from a voices repository. Whether it can be reached is asked when Settings is opened (never at launch), and
/// Settings shows reading aloud only if it answered; a model already on the device reads without any network.
@MainActor @Observable
final class ReadAloudModel {
    /// Where the models are. A debug build can be pointed at another with `-voicesRepo <address>`.
    private static let repository: String = {
        #if DEBUG
        if let other = UserDefaults.standard.string(forKey: "voicesRepo") { return other }
        #endif
        return "github.com/dishant0406/akidev-voices"
    }()

    private let engine: Engine
    private let prefs: Preferences
    /// The repository answered the last time it was asked.
    private(set) var reachable = false
    /// What the repository offers.
    private(set) var offered: [VoiceModel] = []
    private(set) var installed: [VoiceModel]
    /// How far each model being downloaded has got, 0 to 1.
    private(set) var progress: [String: Double] = [:]
    var failure: (title: String, message: String)?

    init(engine: Engine, prefs: Preferences) {
        self.engine = engine
        self.prefs = prefs
        installed = engine.voicesInstalled()
    }

    /// The model that reads: the one chosen, or the only one there is.
    var current: VoiceModel? { installed.first { $0.id == prefs.voiceModel } ?? installed.first }

    func has(_ model: VoiceModel) -> Bool { installed.contains { $0.id == model.id } }

    /// Every model to show: the repository's, then any on the device the repository no longer lists.
    var models: [VoiceModel] {
        offered + installed.filter { have in !offered.contains { $0.id == have.id } }
    }

    /// Asks the repository what it offers. Called each time Settings opens.
    func check() async {
        let engine = engine
        let found = try? await engine.voicesOffered(link: Self.repository)
        reachable = found != nil
        offered = found ?? []
        installed = engine.voicesInstalled()
    }

    func install(_ model: VoiceModel) async {
        guard progress[model.id] == nil else { return }
        progress[model.id] = 0
        defer { progress[model.id] = nil }
        do {
            try await engine.installVoice(link: Self.repository, id: model.id, progress: Download { done, total in
                let fraction = total > 0 ? Double(done) / Double(total) : 0
                Task { @MainActor in if self.progress[model.id] != nil { self.progress[model.id] = fraction } }
            })
            installed = engine.voicesInstalled()
            if installed.count == 1 { prefs.voiceModel = model.id }
        } catch { failure = describe(error) }
    }

    func use(_ model: VoiceModel) { prefs.voiceModel = model.id }

    func remove(_ model: VoiceModel) {
        do {
            try engine.removeVoice(id: model.id)
            installed = engine.voicesInstalled()
        } catch { failure = describe(error) }
    }
}

/// The engine's download progress, handed to a closure.
private final class Download: VoiceProgress {
    private let report: @Sendable (UInt64, UInt64) -> Void
    init(_ report: @escaping @Sendable (UInt64, UInt64) -> Void) { self.report = report }
    func update(done: UInt64, total: UInt64) { report(done, total) }
}
