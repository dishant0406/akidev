import AkiManga
import SwiftUI

/// Akidev is a renderer: it ships with no source. Sources are plugins that come from repositories the
/// person adds by address (a GitHub link, say); this screen is where that happens.
struct SourcesView: View {
    @Environment(AppModel.self) private var model
    @State private var adding = false
    @State private var pasting = false
    @State private var link = ""
    @State private var catalogues: [CatalogueInfo] = []
    @State private var addingCatalogue = false
    @State private var catalogueAddress = ""
    @State private var catalogueProblem: (title: String, message: String)?

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    AkiHeader(title: Copy.Home.sources, subtitle: Copy.Sources.installedCount(model.sources.count)) {
                        AkiGlassGroup(spacing: 10) {
                            HStack(spacing: 10) {
                                AkiLink(value: SettingsRoute.root) { AkiIconButton(symbol: "settings", label: Copy.Settings.title, action: {}).face }.akiGlass(.circle)
                                AkiIconButton(symbol: "add", label: Copy.Sources.addRepository, tint: Aki.primary) { adding = true }
                            }
                        }
                    }
                    if model.prefs.readerKind == .books {
                        booksSection
                        installedSection
                    } else {
                        installedSection
                        booksSection
                    }
                    repositoriesSection
                    if let note = model.note { AkiText(note, .caption, tone: .muted).onAppear { akiAnnounce(note) } }
                }
                .akiPage(narrow: true)
            }
            .akiRefreshable {
                await model.updateSources()
                model.refresh()
            }
            .underTabBar()
            .background(AkiBackdrop())
            .akiNoNavigationBar()
            .akiDestination(for: RepoInfo.self) { RepoScreen(repo: $0) }
            .akiDestination(for: CatalogueRef.self) { CatalogueView(ref: $0) }
            .akiDestination(for: String.self) { SourceSettingsView(source: $0) }
            .akiDestination(for: SettingsRoute.self) { _ in SettingsView() }
            .akiDestination(for: VoicesRoute.self) { _ in VoicesView() }
            .akiDestination(for: VoiceChoiceRoute.self) { _ in VoiceChoiceView() }
            .akiSheet(isPresented: $pasting) { SourceAddSheet() }
            .onAppear(perform: loadCatalogues)
            .akiPrompt(Copy.Books.addBookCatalogue, message: Copy.Sources.addCatalogueMessage, placeholder: Copy.Books.catalogueAddressPlaceholder, confirm: Copy.Books.addCatalogueConfirm, text: $catalogueAddress, isPresented: $addingCatalogue) {
                let address = catalogueAddress
                catalogueAddress = ""
                Task {
                    do {
                        _ = try await model.engine.addCatalogue(url: address)
                        loadCatalogues()
                    } catch { catalogueProblem = describe(error, Copy.Books.couldNotAddCatalogue) }
                }
            }
            .akiAlert(catalogueProblem?.title ?? "", message: catalogueProblem?.message, isPresented: Binding(get: { catalogueProblem != nil }, set: { if !$0 { catalogueProblem = nil } }))
            .akiPrompt(Copy.Sources.addRepository, message: Copy.Sources.addRepositoryMessage, placeholder: Copy.Welcome.AddRepository.placeholder, confirm: Copy.Sources.previewRepository, text: $link, isPresented: $adding) { model.propose(repository: link); link = "" }
        }
    }
}

extension SourcesView {
    @ViewBuilder fileprivate var installedSection: some View {
                    AkiSection(title: Copy.Sources.installed) {
                        if model.sources.isEmpty { AkiCard(padding: 16) { AkiText(Copy.Sources.noSourcesYet, tone: .muted) } }
                        ForEach(model.sources, id: \.id) { source in
                            AkiLink(value: source.id) {
                                AkiRow(title: source.name, subtitle: model.sourceProblems[source.id].map { Copy.Sources.notWorking($0) } ?? (source.nsfw ? Copy.Sources.sourceLineAdult(source.languages.joined(separator: ", ").uppercased(), "\(source.version)") : Copy.Sources.sourceLine(source.languages.joined(separator: ", ").uppercased(), "\(source.version)")), icon: model.sourceProblems[source.id] == nil ? "sources-solid" : "warning-solid", showsChevron: true)
                            }
                            .akiContextMenu([AkiMenuItem(title: Copy.Sources.removeSource, icon: "trash", destructive: true) { try? model.engine.removeSource(id: source.id); model.refresh() }])
                        }
                        AkiButton(Copy.Sources.addSource, icon: "add", fullWidth: true) { pasting = true }
                    }
    }

    @ViewBuilder fileprivate var booksSection: some View {
                    AkiSection(title: Copy.Home.books, footer: Copy.Sources.booksFooter) {
                        ForEach(catalogues, id: \.url) { catalogue in
                            AkiLink(value: CatalogueRef(url: catalogue.url, name: catalogue.name, search: nil)) {
                                AkiRow(title: catalogue.name, subtitle: catalogue.url, icon: "book-solid", showsChevron: true)
                            }
                            .akiContextMenu([AkiMenuItem(title: Copy.Books.removeCatalogue, icon: "trash", destructive: true) { try? model.engine.removeCatalogue(url: catalogue.url); loadCatalogues() }])
                        }
                        AkiButton(Copy.Sources.addEpub, icon: "book", fullWidth: true) { model.tab = .books; model.addingBook = true }
                        AkiButton(Copy.Books.addBookCatalogue, icon: "add", fullWidth: true) { addingCatalogue = true }
                    }
    }

    @ViewBuilder fileprivate var repositoriesSection: some View {
                    AkiSection(title: Copy.Sources.repositories, footer: Copy.Sources.repositoriesFooter) {
                        if model.repos.isEmpty { AkiCard(padding: 16) { AkiText(Copy.Sources.noRepositoriesYet, tone: .muted) } }
                        ForEach(model.repos, id: \.base) { repo in
                            AkiLink(value: repo) {
                                AkiRow(title: repo.name, subtitle: repo.signed ? Copy.Sources.signed : Copy.Sources.notSigned, icon: "server-solid", showsChevron: true)
                            }
                            .akiContextMenu([AkiMenuItem(title: Copy.Sources.removeRepository, icon: "trash", destructive: true) { try? model.engine.removeRepo(base: repo.base); model.refresh() }])
                        }
                        AkiButton(Copy.Sources.addRepository, icon: "add", fullWidth: true) { adding = true }
                    }
    }
}

extension SourcesView {
    fileprivate func loadCatalogues() { catalogues = (try? model.engine.catalogues()) ?? [] }
}

/// What a repository offers, with install and update.
struct RepoScreen: View {
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
                AkiBackHeader(title: repo.name, subtitle: signed ? Copy.Sources.signedRepository : Copy.Sources.notSigned, back: { dismiss() })
                if let error {
                    AkiEmptyState(title: error.title, message: error.message, action: (Copy.Home.tryAgain, { Task { await load() } })).frame(minHeight: 260)
                } else if !loaded {
                    AkiSpinner().frame(maxWidth: .infinity, minHeight: 200)
                } else {
                    if sources.isEmpty { AkiText(Copy.Sources.noSourcesInRepository, tone: .muted) }
                    ForEach(sources, id: \.id) { source in
                        AkiRow(title: source.name, subtitle: (source.nsfw ? Copy.Sources.sourceLineAdult(source.languages.joined(separator: ", ").uppercased(), "\(source.version)") : Copy.Sources.sourceLine(source.languages.joined(separator: ", ").uppercased(), "\(source.version)"))) { action(for: source) }
                    }
                    AkiText(signed ? Copy.Sources.repositorySigned : Copy.Sources.repositoryNotSigned, .caption, tone: .muted)
                }
            }
            .akiPage(narrow: true)
        }
        .akiRefreshable { await load() }
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
            case .notInstalled: AkiButton(Copy.Sources.install, variant: .primary, size: .small) { Task { await install(source) } }.accessibilityLabel(Copy.Sources.installSourceNamed(source.name))
            case .updateAvailable: AkiButton(Copy.Listen.update, variant: .primary, size: .small) { Task { await install(source) } }.accessibilityLabel(Copy.Sources.updateSourceNamed(source.name))
            case .installed: AkiBadge(Copy.Sources.installed, icon: "tick")
            case .needsNewerApp: AkiBadge(Copy.Sources.needsNewerApp)
            }
        }
    }

    private func load() async {
        error = nil
        do {
            let view = try await model.engine.repoSources(base: repo.base)
            sources = view.sources.filter { !$0.nsfw }
            signed = view.info.signed
            loaded = true
        } catch { self.error = describe(error, Copy.Sources.couldNotLoadRepository) }
    }

    private func install(_ source: RepoSource) async {
        working.insert(source.id)
        defer { working.remove(source.id) }
        do {
            _ = try await model.engine.installFromRepo(base: repo.base, source: source.id)
            model.refresh()
            await load()
        } catch { self.error = describe(error, Copy.Sources.couldNotAddSource) }
    }
}

/// The choices a source offers (a language, what content to show).
struct SourceSettingsView: View {
    let source: String
    @Environment(AppModel.self) private var model
    @Environment(\.dismiss) private var dismiss
    @State private var settings: [SettingInfo] = []
    @State private var removing = false
    @State private var signingIn = false
    @State private var signedInAs: String?

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                AkiBackHeader(title: model.name(of: source), subtitle: Copy.Settings.title, back: { dismiss() })
                if let info = model.sources.first(where: { $0.id == source }), info.hasSignIn { accountRow(signedIn: info.signedIn) }
                if settings.isEmpty && !hasAccount { AkiText(Copy.Sources.noSettingsForSource, tone: .muted) }
                ForEach(settings, id: \.id) { setting in
                    AkiCard(padding: 16) {
                        AkiSelect(title: setting.name, selection: Binding(
                            get: { settings.first { $0.id == setting.id }?.value ?? setting.value },
                            set: { value in
                                try? model.engine.setSetting(source: source, id: setting.id, value: value)
                                settings = (try? model.engine.sourceSettings(source: source)) ?? settings
                                model.settingsRevision += 1
                            }
                        ), options: setting.options.map { ($0.value, $0.name) })
                    }
                }
                AkiButton(Copy.Sources.removeSource, icon: "trash", variant: .destructive, fullWidth: true) { removing = true }.padding(.top, 8)
            }
            .akiPage(narrow: true)
        }
        .background(AkiBackdrop())
        .akiNoNavigationBar()
        .hidesTabBar()
        .akiConfirm(Copy.Sources.removeSourceQuestion(model.name(of: source)), message: Copy.Sources.removeSourceMessage, confirm: Copy.Sources.removeSource, isPresented: $removing) {
            try? model.engine.removeSource(id: source)
            model.refresh()
            dismiss()
        }
        .akiSheet(isPresented: $signingIn) { SignInSheet(source: source) }
        .onAppear { settings = (try? model.engine.sourceSettings(source: source)) ?? [] }
        .task(id: model.sources.first { $0.id == source }?.signedIn) { signedInAs = try? model.engine.signedInAs(source: source) }
    }

    private var hasAccount: Bool { model.sources.first { $0.id == source }?.hasSignIn ?? false }

    @ViewBuilder private func accountRow(signedIn: Bool) -> some View {
        if signedIn {
            AkiRow(title: signedInAs ?? Copy.Sources.account, subtitle: Copy.Sources.signedIn) {
                AkiButton(Copy.Sources.signOut, size: .small) { Task { try? await model.engine.signOut(source: source); model.refresh() } }
            }
        } else {
            AkiRow(title: Copy.Sources.account, subtitle: Copy.Sources.notSignedIn) {
                AkiButton(Copy.Sources.signIn, variant: .primary, size: .small) { signingIn = true }
            }
        }
    }
}
