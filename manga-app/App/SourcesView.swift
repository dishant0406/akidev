import AkiManga
import SwiftUI

/// Aki Manga is a renderer: it ships with no source. Sources are plugins that come from repositories the
/// person adds by address (a GitHub link, say); this screen is where that happens.
struct SourcesView: View {
    @Environment(AppModel.self) private var model
    @State private var adding = false
    @State private var link = ""
    @State private var busy = false
    @State private var problem: String?

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    AkiHeader(title: "Sources", subtitle: "\(model.sources.count) installed") {
                        AkiIconButton(symbol: "plus", label: "Add a repository", tint: Aki.yellow) { adding = true }
                    }
                    AkiSection(title: "Installed") {
                        if model.sources.isEmpty { AkiCard(padding: 16) { AkiText("Nothing installed yet", tone: .dim) } }
                        ForEach(model.sources, id: \.id) { source in
                            AkiLink(value: source.id) {
                                AkiRow(title: source.name, subtitle: "\(source.languages.joined(separator: ", ").uppercased()) · v\(source.version)\(source.nsfw ? " · 18+" : "")", icon: "puzzlepiece.extension.fill", showsChevron: true)
                            }
                            .akiContextMenu([AkiMenuItem(title: "Remove", icon: "trash", destructive: true) { try? model.engine.removeSource(id: source.id); model.refresh() }])
                        }
                    }
                    AkiSection(title: "Repositories", footer: "A repository is a folder of source plugins, usually on GitHub: paste its address. Aki Manga does not include, offer or endorse any. Only add repositories you trust: a plugin decides which sites the app contacts.") {
                        if model.repos.isEmpty { AkiCard(padding: 16) { AkiText("No repository added", tone: .dim) } }
                        ForEach(model.repos, id: \.base) { repo in
                            AkiLink(value: repo) {
                                AkiRow(title: repo.name, subtitle: repo.signed ? "Signed" : "Not signed", icon: "externaldrive.fill", showsChevron: true)
                            }
                            .akiContextMenu([AkiMenuItem(title: "Remove", icon: "trash", destructive: true) { try? model.engine.removeRepo(base: repo.base); model.refresh() }])
                        }
                        AkiButton("Add a repository", icon: "plus", fullWidth: true) { adding = true }
                    }
                    if let note = model.note { AkiText(note, .caption, tone: .dim) }
                }
                .padding(.horizontal, 20).padding(.top, 8)
            }
            .underTabBar()
            .background(AkiBackdrop())
            .akiNoNavigationBar()
            .akiDestination(for: RepoInfo.self) { RepoView(repo: $0) }
            .akiDestination(for: String.self) { SourceSettingsView(source: $0) }
            .akiPrompt("Add a repository", message: "The address of a repository's folder on GitHub, or any https folder with an index.json.", placeholder: "github.com/owner/repository", confirm: "Add", text: $link, isPresented: $adding) { Task { await add() } }
            .akiAlert("That did not work", message: problem, isPresented: Binding(get: { problem != nil }, set: { if !$0 { problem = nil } }))
            .overlay { if busy { AkiSpinner().padding(24).akiGlass(.card, interactive: false) } }
        }
    }

    private func add() async {
        let address = link
        link = ""
        busy = true
        defer { busy = false }
        do {
            _ = try await model.engine.addRepo(link: address)
            model.refresh()
        } catch { problem = describe(error).message }
    }
}

extension RepoInfo: @retroactive Hashable {
    public func hash(into hasher: inout Hasher) { hasher.combine(base) }
    public static func == (lhs: RepoInfo, rhs: RepoInfo) -> Bool { lhs.base == rhs.base }
}

/// What a repository offers, with install and update.
struct RepoView: View {
    let repo: RepoInfo
    @Environment(AppModel.self) private var model
    @Environment(\.dismiss) private var dismiss
    @State private var sources: [RepoSource] = []
    @State private var signed = false
    @State private var loaded = false
    @State private var error: (title: String, message: String)?
    @State private var working: Set<String> = []

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                AkiBackHeader(title: repo.name, subtitle: signed ? "Signed repository" : "Not signed", back: { dismiss() })
                if let error {
                    AkiEmptyState(title: error.title, message: error.message, action: ("Try again", { Task { await load() } })).frame(minHeight: 260)
                } else if !loaded {
                    AkiSpinner().frame(maxWidth: .infinity, minHeight: 200)
                } else {
                    ForEach(sources, id: \.id) { source in
                        AkiRow(title: source.name, subtitle: "\(source.languages.joined(separator: ", ").uppercased()) · v\(source.version)\(source.nsfw ? " · 18+" : "")") { action(for: source) }
                    }
                    AkiText(signed ? "This repository signs its index, so the app notices if it ever changes hands." : "This repository is not signed. You are trusting its address.", .caption, tone: .dim)
                }
            }
            .padding(.horizontal, 20).padding(.top, 8)
        }
        .background(AkiBackdrop())
        .akiNoNavigationBar()
        .hidesTabBar()
        .task { await load() }
    }

    @ViewBuilder private func action(for source: RepoSource) -> some View {
        if working.contains(source.id) {
            AkiSpinner()
        } else {
            switch source.state {
            case .notInstalled: AkiButton("Install", variant: .primary, size: .small) { Task { await install(source) } }
            case .updateAvailable: AkiButton("Update", variant: .primary, size: .small) { Task { await install(source) } }
            case .installed: AkiBadge("Installed", icon: "checkmark")
            case .needsNewerApp: AkiBadge("Needs a newer app")
            }
        }
    }

    private func load() async {
        error = nil
        do {
            let view = try await model.engine.repoSources(base: repo.base)
            sources = view.sources
            signed = view.info.signed
            loaded = true
        } catch { self.error = describe(error) }
    }

    private func install(_ source: RepoSource) async {
        working.insert(source.id)
        defer { working.remove(source.id) }
        do {
            _ = try await model.engine.installFromRepo(base: repo.base, source: source.id)
            model.refresh()
            await load()
        } catch { self.error = describe(error) }
    }
}

/// The choices a source offers (a language, what content to show).
struct SourceSettingsView: View {
    let source: String
    @Environment(AppModel.self) private var model
    @Environment(\.dismiss) private var dismiss
    @State private var settings: [SettingInfo] = []
    @State private var removing = false

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                AkiBackHeader(title: model.name(of: source), subtitle: "Settings", back: { dismiss() })
                if settings.isEmpty { AkiText("This source has no settings.", tone: .dim) }
                ForEach(settings, id: \.id) { setting in
                    AkiCard(padding: 16) {
                        AkiSelect(title: setting.name, selection: Binding(
                            get: { settings.first { $0.id == setting.id }?.value ?? setting.value },
                            set: { value in
                                try? model.engine.setSetting(source: source, id: setting.id, value: value)
                                settings = (try? model.engine.sourceSettings(source: source)) ?? settings
                                model.revision += 1
                            }
                        ), options: setting.options.map { ($0.value, $0.name) })
                    }
                }
                AkiButton("Remove this source", icon: "trash", variant: .destructive, fullWidth: true) { removing = true }.padding(.top, 8)
            }
            .padding(.horizontal, 20).padding(.top, 8)
        }
        .background(AkiBackdrop())
        .akiNoNavigationBar()
        .hidesTabBar()
        .akiConfirm("Remove \(model.name(of: source))?", message: "Its saved series stay in your library but cannot be read until you install it again.", confirm: "Remove", isPresented: $removing) {
            try? model.engine.removeSource(id: source)
            model.refresh()
            dismiss()
        }
        .onAppear { settings = (try? model.engine.sourceSettings(source: source)) ?? [] }
    }
}
