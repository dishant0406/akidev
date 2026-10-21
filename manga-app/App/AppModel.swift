import AkiManga
import Foundation
import Observation

@MainActor @Observable
final class AppModel {
    let engine: Engine
    let prefs = Preferences()
    /// An address to add as a repository, waiting for the person to confirm (a pasted link or an opened one).
    var pendingRepo: PendingRepo?
    /// Everything installed; `visibleSources` is what the person's content settings let through.
    var sources: [SourceInfo] = []
    var repos: [RepoInfo] = []
    var library: [LibraryEntry] = []
    var tab: AppTab = .home
    /// Screens pushed over a tab that want the whole screen (a series page) hide the floating bar while shown.
    var barHiders = 0
    var tabBarHidden: Bool { barHiders > 0 }
    /// Bumped when a source's own settings change, so Discover asks it again.
    var settingsRevision = 0
    /// What the last start said about a demo repository, for the Sources screen.
    var note: String?

    init() {
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0].appendingPathComponent("AkiManga")
        engine = try! Engine(dataDir: base.path)
        refresh()
        AppDelegate.refresh = { [weak self] in await self?.refreshLibrary(force: true) }
        Task {
            await updateSources()
            await refreshLibrary()
        }
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
    }

    var visibleSources: [SourceInfo] {
        sources.filter { source in
            (prefs.showAdult || !source.nsfw) && (prefs.languages.isEmpty || source.languages.isEmpty || !Set(source.languages).isDisjoint(with: prefs.languages))
        }
    }

    /// Opens the add-repository confirmation for an address; a bare address or an `akimanga://add-repo?url=` link.
    func propose(repository text: String) {
        var address = text.trimmingCharacters(in: .whitespacesAndNewlines)
        if let url = URL(string: address), url.scheme == "akimanga", let found = URLComponents(url: url, resolvingAgainstBaseURL: false)?.queryItems?.first(where: { $0.name == "url" })?.value {
            address = found
        }
        guard !address.isEmpty else { return }
        pendingRepo = PendingRepo(address: address)
    }

    /// Why a source is not working right now, by source id; cleared when it answers again.
    var sourceProblems: [String: String] = [:]
    /// Series the last refresh could not check, with why.
    var updateFailures: [UpdateFailure] = []
    var refreshing = false
    private var lastRefresh: Date?

    /// Changes when a source is installed, removed or updated (not when the library changes).
    var sourcesKey: String { sources.map { "\($0.id)@\($0.version)" }.joined(separator: ",") + "#\(settingsRevision)" }

    func report(_ source: String, _ error: Error?) {
        sourceProblems[source] = error.map { describe($0).message }
    }

    /// Asks each source for the chapters of the series saved from it. Skipped when done in the last half hour
    /// unless `force` is set (a pull or a tap).
    func refreshLibrary(force: Bool = false) async {
        if refreshing || (!force && (lastRefresh.map { Date.now.timeIntervalSince($0) < 1800 } ?? false)) { return }
        refreshing = true
        defer { refreshing = false }
        let engine = engine
        let results = await withTaskGroup(of: (LibraryEntry, Error?).self) { group in
            for entry in library {
                group.addTask {
                    do { _ = try await engine.chapters(source: entry.source, id: entry.id); return (entry, nil) } catch { return (entry, error) }
                }
            }
            var all: [(LibraryEntry, Error?)] = []
            for await result in group { all.append(result) }
            return all
        }
        guard !Task.isCancelled else { return }
        updateFailures = results.compactMap { entry, error in error.map { UpdateFailure(title: entry.title, source: name(of: entry.source), message: describe($0).message) } }
        // A source is "not working" when any of its series failed this time; one that is gone loses its mark.
        let failedSources = Set(results.filter { $0.1 != nil }.map(\.0.source))
        for source in Set(results.map(\.0.source)) { sourceProblems[source] = nil }
        for (entry, error) in results where error != nil && failedSources.contains(entry.source) { report(entry.source, error) }
        sourceProblems = sourceProblems.filter { problem in sources.contains { $0.id == problem.key } }
        if results.isEmpty || results.contains(where: { $0.1 == nil }) { lastRefresh = .now }
        refresh()
    }

    /// Chapters not read, across the library: the Updates badge.
    var unreadTotal: Int { library.reduce(0) { $0 + Int($1.unread) } }

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

struct UpdateFailure: Identifiable {
    let title: String
    let source: String
    let message: String
    var id: String { "\(source)/\(title)" }
}

struct PendingRepo: Identifiable {
    let address: String
    var id: String { address }
}

/// What a screen needs to say about a failed call.
func describe(_ error: Error) -> (title: String, message: String) {
    if let error = error as? MangaError {
        switch error {
        case .Challenge(let url, _): return ("This site checks visitors first", "\(URL(string: url)?.host ?? url) asks for a browser check before it answers, and Aki Manga has no browser to pass it in. Try another source, or read this one through a Suwayomi or Komga server you run.")
        case .Offline(let url, _): return ("No connection", "Could not reach \(URL(string: url)?.host ?? url). Check your network and try again.")
        case .Failed(let message): return ("That did not work", message)
        }
    }
    return ("That did not work", error.localizedDescription)
}

/// A series being opened: the source it comes from and its id.
struct SeriesRef: Hashable {
    let source: String
    let id: String
    let title: String
    let cover: String?
}
