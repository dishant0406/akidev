import AkiManga
import Foundation
import Observation

@MainActor @Observable
final class AppModel {
    let engine: Engine
    var sources: [SourceInfo] = []
    var library: [LibraryEntry] = []
    /// What the last start said about a demo source, for the Sources screen.
    var note: String?

    init() {
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0].appendingPathComponent("AkiManga")
        engine = try! Engine(dataDir: base.path)
        refresh()
        #if DEBUG
        // `-demoServer http://127.0.0.1:8770` installs the theme and source that server offers, so the
        // app can be driven without any real site (see scripts/demo-server.py).
        if let demo = UserDefaults.standard.string(forKey: "demoServer") {
            Task { await installDemo(from: demo) }
        }
        #endif
    }

    func refresh() {
        sources = engine.sources().sorted { $0.name.localizedCaseInsensitiveCompare($1.name) == .orderedAscending }
        library = (try? engine.library()) ?? []
    }

    func name(of source: String) -> String {
        sources.first { $0.id == source }?.name ?? source
    }

    /// Installs a source from a link to its definition (a theme it extends is installed first when the
    /// definition lists it as `"theme_url"`).
    func install(from link: String) async throws {
        guard let url = URL(string: link), ["http", "https"].contains(url.scheme) else { throw InstallError.badLink }
        let (data, _) = try await URLSession.shared.data(from: url)
        guard let json = String(data: data, encoding: .utf8) else { throw InstallError.notText }
        _ = try engine.installSource(json: json)
        refresh()
    }

    #if DEBUG
    private func installDemo(from base: String) async {
        do {
            let theme = try await text("\(base)/_demo/madara.json")
            try engine.installTheme(name: "madara", json: theme)
            let source = try await text("\(base)/_demo/local.json")
            _ = try engine.installSource(json: source)
            note = "Demo source installed from \(base)"
        } catch {
            note = "Demo source failed: \(error.localizedDescription)"
        }
        refresh()
    }

    private func text(_ link: String) async throws -> String {
        let (data, _) = try await URLSession.shared.data(from: URL(string: link)!)
        return String(decoding: data, as: UTF8.self)
    }
    #endif
}

enum InstallError: LocalizedError {
    case badLink, notText
    var errorDescription: String? {
        switch self {
        case .badLink: "That is not a web address."
        case .notText: "That address did not return text."
        }
    }
}

/// What a screen needs to say about a failed call.
func describe(_ error: Error) -> (title: String, message: String) {
    if let error = error as? MangaError {
        switch error {
        case .Challenge(let url, _): return ("This site wants a check", "\(URL(string: url)?.host ?? url) asks for a browser check before it answers. Passing it from the app comes in a later version.")
        case .Failed(let message): return ("That did not work", message)
        }
    }
    return ("That did not work", error.localizedDescription)
}
