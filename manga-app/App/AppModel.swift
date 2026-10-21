import AkiManga
import Foundation
import Observation

enum AppTab: Hashable { case library, browse, sources }

@MainActor @Observable
final class AppModel {
    let engine: Engine
    var sources: [SourceInfo] = []
    var repos: [RepoInfo] = []
    var library: [LibraryEntry] = []
    var tab: AppTab = .library
    /// Bumped when a source is installed, removed, updated or changed, so screens that show one reload.
    var revision = 0
    /// What the last start said about a demo repository, for the Sources screen.
    var note: String?

    init() {
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0].appendingPathComponent("AkiManga")
        engine = try! Engine(dataDir: base.path)
        refresh()
        Task { await updateSources() }
        #if DEBUG
        // `-demoRepo http://127.0.0.1:8770/_demo/repo/` adds that repository and installs its `local` source at
        // start, so the app can be driven without any real site (see scripts/demo-server.py).
        if let demo = UserDefaults.standard.string(forKey: "demoRepo") {
            Task { await installDemo(from: demo) }
        }
        #endif
    }

    func refresh() {
        sources = engine.sources().sorted { $0.name.localizedCaseInsensitiveCompare($1.name) == .orderedAscending }
        repos = (try? engine.repos()) ?? []
        library = (try? engine.library()) ?? []
        revision += 1
    }

    func name(of source: String) -> String {
        sources.first { $0.id == source }?.name ?? source
    }

    /// Updates installed sources whose repository has a newer version; quiet if there is nothing or no network.
    func updateSources() async {
        if let updated = try? await engine.updateSources(), !updated.isEmpty { refresh() }
    }

    #if DEBUG
    private func installDemo(from link: String) async {
        do {
            let view = try await engine.addRepo(link: link)
            // `-demoInstall a,b` installs several; the default is the demo server's `local`.
            let wanted = (UserDefaults.standard.string(forKey: "demoInstall") ?? "local").split(separator: ",").map(String.init)
            for id in wanted where !sources.contains(where: { $0.id == id }) {
                _ = try await engine.installFromRepo(base: view.info.base, source: id)
                refresh()
            }
            note = "Demo repository added from \(link)"
        } catch {
            note = "Demo repository failed: \(describe(error).message)"
        }
        refresh()
    }
    #endif
}

/// What a screen needs to say about a failed call.
func describe(_ error: Error) -> (title: String, message: String) {
    if let error = error as? MangaError {
        switch error {
        case .Challenge(let url, _): return ("This site checks visitors first", "\(URL(string: url)?.host ?? url) asks for a browser check before it answers, and Aki Manga has no browser to pass it in. Try another source, or read this one through a Suwayomi or Komga server you run.")
        case .Failed(let message): return ("That did not work", message)
        }
    }
    return ("That did not work", error.localizedDescription)
}
