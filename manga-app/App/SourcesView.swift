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
            List {
                Section {
                    if model.sources.isEmpty {
                        Text("Nothing installed yet").foregroundStyle(Aki.dim)
                    }
                    ForEach(model.sources, id: \.id) { source in
                        NavigationLink(value: source.id) {
                            VStack(alignment: .leading, spacing: 2) {
                                Text(source.name).foregroundStyle(Aki.text)
                                Text("\(source.languages.joined(separator: ", ").uppercased()) · v\(source.version)\(source.nsfw ? " · 18+" : "")")
                                    .font(.caption).foregroundStyle(Aki.faint)
                            }
                        }
                        .swipeActions {
                            Button("Remove", role: .destructive) { try? model.engine.removeSource(id: source.id); model.refresh() }
                        }
                    }
                } header: { Text("Installed") }

                Section {
                    ForEach(model.repos, id: \.base) { repo in
                        NavigationLink(value: repo) {
                            VStack(alignment: .leading, spacing: 2) {
                                Text(repo.name).foregroundStyle(Aki.text)
                                Text(repo.signed ? "Signed" : "Not signed").font(.caption).foregroundStyle(repo.signed ? Aki.accent : Aki.faint)
                            }
                        }
                        .swipeActions {
                            Button("Remove", role: .destructive) { try? model.engine.removeRepo(base: repo.base); model.refresh() }
                        }
                    }
                    Button { adding = true } label: { Label("Add a repository", systemImage: "plus") }
                } header: { Text("Repositories") } footer: {
                    Text("A repository is a folder of source plugins, usually on GitHub: paste its address. Aki Manga does not include, offer or endorse any. Only add repositories you trust: a plugin decides which sites the app contacts.")
                }

                if let note = model.note { Section { Text(note).font(.footnote).foregroundStyle(Aki.dim) } }
            }
            .scrollContentBackground(.hidden)
            .background(Aki.canvas)
            .navigationTitle("Sources")
            .navigationDestination(for: RepoInfo.self) { RepoView(repo: $0) }
            .navigationDestination(for: String.self) { SourceSettingsView(source: $0) }
            .alert("Add a repository", isPresented: $adding) {
                TextField("github.com/owner/repository", text: $link).textInputAutocapitalization(.never).autocorrectionDisabled().keyboardType(.URL)
                Button("Add") { Task { await add() } }
                Button("Cancel", role: .cancel) { link = "" }
            } message: { Text("The address of a repository's folder on GitHub, or any https folder with an index.json.") }
            .alert("That did not work", isPresented: Binding(get: { problem != nil }, set: { if !$0 { problem = nil } })) {
                Button("OK", role: .cancel) {}
            } message: { Text(problem ?? "") }
            .overlay { if busy { ProgressView().padding(24).glassEffect(.regular, in: .rect(cornerRadius: Aki.radius[2])) } }
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
    @State private var view: RepoView_?
    @State private var error: (title: String, message: String)?
    @State private var working: Set<String> = []

    private struct RepoView_ { let sources: [RepoSource]; let signed: Bool }

    var body: some View {
        List {
            if let view {
                Section {
                    ForEach(view.sources, id: \.id) { source in
                        HStack {
                            VStack(alignment: .leading, spacing: 2) {
                                Text(source.name).foregroundStyle(Aki.text)
                                Text("\(source.languages.joined(separator: ", ").uppercased()) · v\(source.version)\(source.nsfw ? " · 18+" : "")")
                                    .font(.caption).foregroundStyle(Aki.faint)
                            }
                            Spacer()
                            action(for: source)
                        }
                    }
                } footer: {
                    Text(view.signed ? "This repository signs its index, so the app notices if it ever changes hands." : "This repository is not signed. You are trusting its address.")
                }
            } else if let error {
                StateCard(title: error.title, message: error.message, action: ("Try again", { Task { await load() } }))
            } else {
                ProgressView().frame(maxWidth: .infinity)
            }
        }
        .scrollContentBackground(.hidden)
        .background(Aki.canvas)
        .navigationTitle(repo.name)
        .navigationBarTitleDisplayMode(.inline)
        .task { await load() }
    }

    @ViewBuilder private func action(for source: RepoSource) -> some View {
        if working.contains(source.id) {
            ProgressView()
        } else {
            switch source.state {
            case .notInstalled: Button("Install") { Task { await install(source) } }.buttonStyle(.glassProminent)
            case .updateAvailable: Button("Update") { Task { await install(source) } }.buttonStyle(.glass)
            case .installed: Text("Installed").font(.footnote).foregroundStyle(Aki.faint)
            case .needsNewerApp: Text("Needs a newer app").font(.footnote).foregroundStyle(Aki.faint)
            }
        }
    }

    private func load() async {
        error = nil
        do {
            let loaded = try await model.engine.repoSources(base: repo.base)
            view = RepoView_(sources: loaded.sources, signed: loaded.info.signed)
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
    @State private var settings: [SettingInfo] = []

    var body: some View {
        Form {
            if settings.isEmpty {
                Text("This source has no settings.").foregroundStyle(Aki.dim)
            }
            ForEach(settings, id: \.id) { setting in
                Picker(setting.name, selection: Binding(
                    get: { settings.first { $0.id == setting.id }?.value ?? setting.value },
                    set: { value in
                        try? model.engine.setSetting(source: source, id: setting.id, value: value)
                        settings = (try? model.engine.sourceSettings(source: source)) ?? settings
                        model.revision += 1
                    }
                )) {
                    ForEach(setting.options, id: \.value) { Text($0.name).tag($0.value) }
                }
            }
        }
        .scrollContentBackground(.hidden)
        .background(Aki.canvas)
        .navigationTitle(model.name(of: source))
        .navigationBarTitleDisplayMode(.inline)
        .onAppear { settings = (try? model.engine.sourceSettings(source: source)) ?? [] }
    }
}
