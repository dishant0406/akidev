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
                    ScreenHeader(title: "Sources", subtitle: "\(model.sources.count) installed") {
                        GlassIconButton(symbol: "plus", tint: Aki.yellow) { adding = true }
                    }
                    section("Installed") {
                        if model.sources.isEmpty { emptyRow("Nothing installed yet") }
                        ForEach(model.sources, id: \.id) { source in
                            NavigationLink(value: source.id) {
                                row(title: source.name, subtitle: "\(source.languages.joined(separator: ", ").uppercased()) · v\(source.version)\(source.nsfw ? " · 18+" : "")", symbol: "puzzlepiece.extension.fill")
                            }
                            .buttonStyle(.plain)
                            .contextMenu { Button("Remove", systemImage: "trash", role: .destructive) { try? model.engine.removeSource(id: source.id); model.refresh() } }
                        }
                    }
                    section("Repositories") {
                        if model.repos.isEmpty { emptyRow("No repository added") }
                        ForEach(model.repos, id: \.base) { repo in
                            NavigationLink(value: repo) {
                                row(title: repo.name, subtitle: repo.signed ? "Signed" : "Not signed", symbol: "externaldrive.fill", highlight: repo.signed)
                            }
                            .buttonStyle(.plain)
                            .contextMenu { Button("Remove", systemImage: "trash", role: .destructive) { try? model.engine.removeRepo(base: repo.base); model.refresh() } }
                        }
                        Button { adding = true } label: {
                            Label("Add a repository", systemImage: "plus").font(.system(size: 16, weight: .bold, design: .rounded)).foregroundStyle(Aki.ink)
                                .frame(maxWidth: .infinity, minHeight: 54).contentShape(Capsule())
                        }
                        .buttonStyle(.plain).glassEffect(.regular.interactive(), in: .capsule)
                    }
                    Text("A repository is a folder of source plugins, usually on GitHub: paste its address. Aki Manga does not include, offer or endorse any. Only add repositories you trust: a plugin decides which sites the app contacts.")
                        .font(.system(size: 13, design: .rounded)).foregroundStyle(Aki.dim)
                    if let note = model.note { Text(note).font(.system(size: 12, design: .rounded)).foregroundStyle(Aki.dim) }
                }
                .padding(.horizontal, 20).padding(.top, 8)
            }
            .underTabBar()
            .background(AmbientBackground())
            .toolbar(.hidden, for: .navigationBar)
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
            .overlay { if busy { ProgressView().padding(24).glassEffect(.regular, in: .rect(cornerRadius: 20)) } }
        }
    }

    private func section<Content: View>(_ title: String, @ViewBuilder content: () -> Content) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(title).font(Aki.title(19)).foregroundStyle(Aki.ink)
            content()
        }
    }

    private func emptyRow(_ text: String) -> some View {
        Text(text).font(.system(size: 15, design: .rounded)).foregroundStyle(Aki.dim)
            .frame(maxWidth: .infinity, alignment: .leading).padding(16)
            .background(Aki.card, in: RoundedRectangle(cornerRadius: Aki.cardRadius, style: .continuous))
    }

    private func row(title: String, subtitle: String, symbol: String, highlight: Bool = false) -> some View {
        HStack(spacing: 14) {
            Image(systemName: symbol).font(.system(size: 18, weight: .semibold)).foregroundStyle(Aki.onYellow)
                .frame(width: 46, height: 46).background(Aki.yellow, in: RoundedRectangle(cornerRadius: Aki.thumb, style: .continuous))
            VStack(alignment: .leading, spacing: 3) {
                Text(title).font(.system(size: 17, weight: .bold, design: .rounded)).foregroundStyle(Aki.ink).lineLimit(1)
                Text(subtitle).font(.system(size: 13, weight: .medium, design: .rounded)).foregroundStyle(highlight ? Aki.ink : Aki.dim).lineLimit(1)
            }
            Spacer()
            Image(systemName: "chevron.right").font(.system(size: 13, weight: .bold)).foregroundStyle(Aki.dim)
        }
        .padding(12)
        .background(Aki.card, in: RoundedRectangle(cornerRadius: Aki.cardRadius, style: .continuous))
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
                HStack {
                    GlassIconButton(symbol: "chevron.left") { dismiss() }
                    Spacer()
                }
                ScreenHeader(title: repo.name, subtitle: signed ? "Signed repository" : "Not signed") { EmptyView() }
                if let error {
                    StateCard(title: error.title, message: error.message, action: ("Try again", { Task { await load() } })).frame(minHeight: 260)
                } else if !loaded {
                    ProgressView().frame(maxWidth: .infinity, minHeight: 200)
                } else {
                    ForEach(sources, id: \.id) { source in
                        HStack(spacing: 12) {
                            VStack(alignment: .leading, spacing: 3) {
                                Text(source.name).font(.system(size: 17, weight: .bold, design: .rounded)).foregroundStyle(Aki.ink)
                                Text("\(source.languages.joined(separator: ", ").uppercased()) · v\(source.version)\(source.nsfw ? " · 18+" : "")")
                                    .font(.system(size: 13, weight: .medium, design: .rounded)).foregroundStyle(Aki.dim)
                            }
                            Spacer()
                            action(for: source)
                        }
                        .padding(14)
                        .background(Aki.card, in: RoundedRectangle(cornerRadius: Aki.cardRadius, style: .continuous))
                    }
                    Text(signed ? "This repository signs its index, so the app notices if it ever changes hands." : "This repository is not signed. You are trusting its address.")
                        .font(.system(size: 13, design: .rounded)).foregroundStyle(Aki.dim)
                }
            }
            .padding(.horizontal, 20).padding(.top, 8)
        }
        .background(AmbientBackground())
        .toolbar(.hidden, for: .navigationBar)
        .hidesTabBar()
        .task { await load() }
    }

    @ViewBuilder private func action(for source: RepoSource) -> some View {
        if working.contains(source.id) {
            ProgressView()
        } else {
            switch source.state {
            case .notInstalled: smallButton("Install", yellow: true) { Task { await install(source) } }
            case .updateAvailable: smallButton("Update", yellow: true) { Task { await install(source) } }
            case .installed: Chip(text: "Installed", symbol: "checkmark")
            case .needsNewerApp: Chip(text: "Needs a newer app")
            }
        }
    }

    private func smallButton(_ title: String, yellow: Bool, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Text(title).font(.system(size: 14, weight: .bold, design: .rounded)).foregroundStyle(yellow ? Aki.onYellow : Aki.ink)
                .padding(.horizontal, 18).frame(height: 38).contentShape(Capsule())
        }
        .buttonStyle(.plain)
        .glassEffect(yellow ? .regular.tint(Aki.yellow).interactive() : .regular.interactive(), in: .capsule)
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

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                HStack {
                    GlassIconButton(symbol: "chevron.left") { dismiss() }
                    Spacer()
                }
                ScreenHeader(title: model.name(of: source), subtitle: "Settings") { EmptyView() }
                if settings.isEmpty {
                    Text("This source has no settings.").font(.system(size: 15, design: .rounded)).foregroundStyle(Aki.dim)
                }
                ForEach(settings, id: \.id) { setting in
                    HStack {
                        Text(setting.name).font(.system(size: 16, weight: .bold, design: .rounded)).foregroundStyle(Aki.ink)
                        Spacer()
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
                        .labelsHidden().tint(Aki.ink)
                    }
                    .padding(16)
                    .background(Aki.card, in: RoundedRectangle(cornerRadius: Aki.cardRadius, style: .continuous))
                }
                Button(role: .destructive) { try? model.engine.removeSource(id: source); model.refresh(); dismiss() } label: {
                    Label("Remove this source", systemImage: "trash").font(.system(size: 15, weight: .bold, design: .rounded))
                        .frame(maxWidth: .infinity, minHeight: 50)
                }
                .buttonStyle(.plain).foregroundStyle(Aki.danger).glassEffect(.regular.interactive(), in: .capsule)
                .padding(.top, 8)
            }
            .padding(.horizontal, 20).padding(.top, 8)
        }
        .background(AmbientBackground())
        .toolbar(.hidden, for: .navigationBar)
        .hidesTabBar()
        .onAppear { settings = (try? model.engine.sourceSettings(source: source)) ?? [] }
    }
}
