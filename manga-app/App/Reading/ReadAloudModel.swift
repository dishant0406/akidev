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
    /// The voices being fetched now, and the one being heard (empty for the model's own).
    private(set) var fetching: Set<String> = []
    private(set) var hearing: String?
    private let preview = VoicePreview()

    init(engine: Engine, prefs: Preferences) {
        self.engine = engine
        self.prefs = prefs
        installed = engine.voicesInstalled()
    }

    /// The model that reads: the one chosen, or the only one there is.
    var current: VoiceModel? { installed.first { $0.id == prefs.voiceModel } ?? installed.first }

    func has(_ model: VoiceModel) -> Bool { installed.contains { $0.id == model.id } }

    /// The repository offers a newer version of a model that is on the phone.
    func hasUpdate(_ model: VoiceModel) -> Bool { installed.first { $0.id == model.id }.map { $0.version < model.version } ?? false }

    /// The voice that reads, when one of the model's other voices is chosen and on the phone; otherwise the model's own.
    var chosenVoice: VoiceOption? { current?.voices.first { $0.id == prefs.voiceName && $0.downloaded } }

    /// The id of the voice that is in use: empty for the model's own.
    var voiceInUse: String { chosenVoice?.id ?? "" }

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

    func use(_ model: VoiceModel) {
        prefs.voiceModel = model.id
        prefs.voiceName = ""
    }

    /// Chooses a voice of the model in use, fetching it first if it is not on the phone yet; nothing is the model's own.
    func choose(_ voice: VoiceOption?) async {
        if let voice, !(await fetch(voice)) { return }
        prefs.voiceName = voice?.id ?? ""
    }

    /// Says a sentence in a voice (the model's own when nothing), fetching the voice first if need be.
    func hear(_ voice: VoiceOption?) async {
        guard let model = current else { return }
        if let voice, !(await fetch(voice)) { return }
        let engine = engine, id = model.id, name = voice?.id
        hearing = voice?.id ?? ""
        defer { hearing = nil }
        let heard = try? await Task.detached(priority: .userInitiated) { () throws -> (samples: [Float], rate: Int) in
            let speaker = try engine.openSpeaker(id: id)
            return (try speaker.speak(text: "It is a truth universally acknowledged, that a single man in possession of a good fortune, must be in want of a wife.", voice: name), Int(speaker.sampleRate()))
        }.value
        guard let heard else { return failure = ("That did not work", "The voice could not be played.") }
        preview.play(heard.samples, rate: heard.rate)
    }

    /// Fetches one of the model's voices; whether it is on the phone after.
    private func fetch(_ voice: VoiceOption) async -> Bool {
        guard !voice.downloaded, let model = current else { return true }
        guard fetching.insert(voice.id).inserted else { return false }
        defer { fetching.remove(voice.id) }
        do {
            try await engine.getVoiceChoice(model: model.id, voice: voice.id)
            installed = engine.voicesInstalled()
            return true
        } catch {
            failure = describe(error)
            return false
        }
    }

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
