import AkiManga
import SwiftUI

@MainActor @Observable
final class BrowseModel {
    enum Mode: String, CaseIterable { case popular = "Popular", latest = "Latest" }
    var items: [Card] = []
    var hasNext = false
    var loading = false
    var error: (title: String, message: String)?
    private var page: UInt32 = 0
    private var key = ""

    func load(_ engine: Engine, report: (String, Error?) -> Void, source: String, mode: Mode, term: String, choices: [Choice] = [], reset: Bool) async {
        let newKey = "\(source)|\(mode)|\(term)|\(choices.map { "\($0.id)=\($0.value)" })"
        if reset || newKey != key { items = []; page = 0; hasNext = false; key = newKey; error = nil }
        guard !loading else { return }
        loading = true
        defer { loading = false }
        // The first page of Popular and Latest shows at once from the last time, while the fresh one is fetched.
        if page == 0, items.isEmpty, term.isEmpty, choices.isEmpty, let kept = engine.rememberedListing(source: source, latest: mode == .latest) {
            items = kept.items
            hasNext = kept.hasNext
        }
        do {
            let next = page + 1
            let result: CardPage
            if !term.isEmpty || !choices.isEmpty {
                result = try await engine.search(source: source, term: term, page: next, choices: choices)
            } else if mode == .latest {
                result = try await engine.latest(source: source, page: next)
            } else {
                result = try await engine.popular(source: source, page: next)
            }
            guard key == newKey else { return }
            report(source, nil)
            page = next
            if next == 1 {
                items = result.items
            } else {
                let known = Set(items.map(\.id))
                items += result.items.filter { !known.contains($0.id) }
            }
            hasNext = result.hasNext
        } catch {
            guard key == newKey else { return }
            report(source, error)
            self.error = describe(error)
        }
    }
}

/// Discover: the sources you installed, what is popular or new in one, and search.
struct HomeView: View {
    @Environment(AppModel.self) private var model
    @State private var browse = BrowseModel()
    @State private var source = UserDefaults.standard.string(forKey: "source") ?? ""
    @State private var mode = BrowseModel.Mode.popular
    @State private var term = ""
    @State private var submitted = ""
    @State private var filters: [FilterInfo] = []
    @State private var choices: [String: String] = [:]
    @State private var filtering = false
    @State private var global = GlobalSearchModel()
    private static let everySource = "*"
    private var isGlobal: Bool { source == Self.everySource }

    private var info: SourceInfo? { model.sources.first { $0.id == source } }
    private let columns = [GridItem(.flexible(), spacing: 14, alignment: .top), GridItem(.flexible(), spacing: 14, alignment: .top)]

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    AkiHeader(title: "Discover", subtitle: isGlobal ? "All sources" : info?.name)
                    if model.sources.isEmpty && model.repos.isEmpty {
                        WelcomeView()
                    } else if model.visibleSources.isEmpty {
                        AkiEmptyState(title: model.sources.isEmpty ? "No sources installed" : "Nothing to show", message: model.sources.isEmpty ? "Install a source from your repository in Sources to start reading." : "Your content settings hide every installed source.", action: ("Open Sources", { model.tab = .sources }))
                            .frame(minHeight: 420)
                    } else {
                        HStack(spacing: 10) {
                            AkiInput(placeholder: "Search \(isGlobal ? "every source" : info?.name ?? "")", text: $term, icon: "search", onSubmit: { submitted = term; Task { await reload() } }, onClear: { submitted = ""; Task { await reload() } })
                            if !filters.isEmpty && !isGlobal {
                                AkiIconButton(symbol: "sliders", label: "Filters", tint: choices.isEmpty ? nil : Aki.primary) { filtering = true }
                            }
                        }
                        if model.visibleSources.count > 1 { AkiChips(items: [(Self.everySource, "All")] + model.visibleSources.map { ($0.id, $0.name) }, selection: $source) }
                        if !isGlobal && info?.canLatest == true && submitted.isEmpty && choices.isEmpty {
                            AkiTabs(items: BrowseModel.Mode.allCases.map { ($0, $0.rawValue) }, selection: $mode)
                        }
                        if isGlobal { globalResults } else { results }
                    }
                }
                .padding(.horizontal, 20).padding(.top, 8)
            }
            .scrollDismissesKeyboard(.immediately)
            .akiRefreshable { await reload() }
            .underTabBar()
            .background(AkiBackdrop())
            .akiNoNavigationBar()
            .akiDestination(for: SeriesRef.self) { SeriesView(ref: $0) }
            .onChange(of: source) { UserDefaults.standard.set(source, forKey: "source"); submitted = ""; term = ""; choices = [:]; filters = (try? model.engine.sourceFilters(source: source)) ?? []; Task { await reload() } }
            .akiSheet(isPresented: $filtering) { FiltersSheet(filters: filters, choices: $choices) { Task { await reload() } } }
            .onChange(of: mode) { Task { await reload() } }
            .task(id: model.sourcesKey) {
                if !isGlobal && !model.visibleSources.contains(where: { $0.id == source }) { source = model.visibleSources.first?.id ?? "" }
                guard !source.isEmpty else { return }
                filters = (try? model.engine.sourceFilters(source: source)) ?? []
                await reload()
            }
        }
    }

    @ViewBuilder private var globalResults: some View {
        if submitted.isEmpty {
            AkiEmptyState(title: "Search every source", message: "Type a title and every installed source is asked at once; each answers when it is ready.").frame(minHeight: 320)
        } else {
            GlobalSearchResults(model: global)
        }
    }

    @ViewBuilder private var results: some View {
        if let error = browse.error, browse.items.isEmpty {
            AkiEmptyState(title: error.title, message: error.message, action: ("Try again", { Task { await reload() } })).frame(minHeight: 320)
        } else if browse.items.isEmpty && !browse.loading {
            AkiEmptyState(title: submitted.isEmpty ? "Nothing to show" : "No results", message: submitted.isEmpty ? "This source gave no titles." : "Nothing matched “\(submitted)” in \(info?.name ?? source).").frame(minHeight: 320)
        } else if browse.items.isEmpty {
            AkiSpinner().frame(maxWidth: .infinity, minHeight: 240)
        } else {
            if submitted.isEmpty && choices.isEmpty, let first = browse.items.first { featured(first) }
            LazyVGrid(columns: columns, spacing: 22) {
                ForEach(submitted.isEmpty && choices.isEmpty ? Array(browse.items.dropFirst()) : browse.items, id: \.id) { card in
                    AkiLink(value: SeriesRef(source: source, id: card.id, title: card.title, cover: card.cover)) {
                        AkiCoverCard(title: card.title, cover: card.cover, subtitle: info?.name)
                    }
                }
            }
            if browse.hasNext {
                AkiButton(browse.loading ? "Loading…" : "Show more") { Task { await load(reset: false) } }.frame(maxWidth: .infinity)
            }
        }
    }

    /// The first title, large.
    private func featured(_ card: Card) -> some View {
        AkiLink(value: SeriesRef(source: source, id: card.id, title: card.title, cover: card.cover)) {
            ZStack(alignment: .bottomLeading) {
                AkiImage(url: card.cover, placeholder: 1.1, fill: true, free: true)
                LinearGradient(colors: [.clear, Aki.scrimStrong], startPoint: .center, endPoint: .bottom)
                VStack(alignment: .leading, spacing: 8) {
                    AkiBadge(mode == .latest ? "New" : "Popular", icon: "sparkles", style: .glass)
                    AkiText(card.title, .title, tone: .onMedia, lines: 2)
                }.padding(18)
            }
            .frame(height: 320)
            .clipShape(RoundedRectangle(cornerRadius: Aki.cardRadius, style: .continuous))
            .contentShape(RoundedRectangle(cornerRadius: Aki.cardRadius, style: .continuous))
        }
    }

    private func reload() async {
        if isGlobal {
            if submitted.isEmpty { global.clear() } else { await global.search(model.engine, sources: model.visibleSources, term: submitted, report: model.report) }
        } else {
            await load(reset: true)
        }
    }

    private func load(reset: Bool) async {
        await browse.load(model.engine, report: model.report, source: source, mode: mode, term: submitted, choices: choices.map { Choice(id: $0.key, value: $0.value) }.sorted { $0.id < $1.id }, reset: reset)
    }
}

/// The ways a source lets a search be narrowed (an order, a genre, a status).
struct FiltersSheet: View {
    let filters: [FilterInfo]
    @Binding var choices: [String: String]
    let apply: () -> Void
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                AkiText("Filters", .title)
                ForEach(filters, id: \.id) { filter in
                    AkiCard(padding: 16) {
                        switch filter.kind {
                        case .select:
                            AkiSelect(title: filter.name, selection: binding(filter), options: filter.options.map { ($0.value, $0.name) })
                        case .text:
                            VStack(alignment: .leading, spacing: 8) {
                                AkiText(filter.name, .bodyStrong)
                                AkiInput(placeholder: filter.name, text: binding(filter))
                            }
                        }
                    }
                }
                AkiButton("Show results", variant: .primary, size: .large, fullWidth: true) { dismiss(); apply() }
                AkiButton("Reset", variant: .ghost, fullWidth: true) { choices = [:]; dismiss(); apply() }
            }
            .padding(24)
        }
    }

    /// A choice equal to the filter's default is not a choice.
    private func binding(_ filter: FilterInfo) -> Binding<String> {
        Binding(get: { choices[filter.id] ?? filter.default }, set: { choices[filter.id] = $0 == filter.default ? nil : $0 })
    }
}
