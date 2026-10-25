import AkiManga
import SwiftUI

@MainActor @Observable
final class BrowseModel {
    var items: [Card] = []
    var hasNext = false
    var loading = false
    var error: (title: String, message: String)?
    private var page: UInt32 = 0
    private var key = ""

    func load(_ engine: Engine, report: (String, Error?) -> Void, source: SourceInfo, term: String, choices: [Choice] = [], reset: Bool) async {
        let newKey = "\(source.id)|\(term)|\(choices.map { "\($0.id)=\($0.value)" })"
        if reset || newKey != key { items = []; page = 0; hasNext = false; key = newKey; error = nil }
        guard !loading else { return }
        loading = true
        defer { loading = false }
        // The first page of Popular and Latest shows at once from the last time, while the fresh one is fetched.
        if page == 0, items.isEmpty, term.isEmpty, choices.isEmpty, source.canPopular, let kept = engine.rememberedListing(source: source.id, latest: false) {
            items = kept.items
            hasNext = kept.hasNext
        }
        do {
            let next = page + 1
            let result: CardPage
            if !term.isEmpty || !choices.isEmpty {
                result = try await engine.search(source: source.id, term: term, page: next, choices: choices)
            } else if source.canPopular {
                result = try await engine.popular(source: source.id, page: next)
            } else if source.canLatest {
                // No popular list: fall back to the newest titles so the feed is not empty.
                result = try await engine.latest(source: source.id, page: next)
            } else {
                items = []; hasNext = false; return
            }
            guard key == newKey else { return }
            report(source.id, nil)
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
            report(source.id, error)
            self.error = describe(error)
        }
    }
}

/// Discover: the sources you installed, what is popular or new in one, and search.
struct HomeView: View {
    @Environment(AppModel.self) private var model
    @State private var browse = BrowseModel()
    @State private var source = UserDefaults.standard.string(forKey: "source") ?? ""
    @State private var feed = FeedModel()
    @State private var reading: [HistoryEntry] = []
    @State private var term = ""
    @State private var submitted = ""
    @State private var filters: [FilterInfo] = []
    @State private var choices: [String: String] = [:]
    @State private var filtering = false
    @State private var global = GlobalSearchModel()
    /// The sources the lists on screen were asked from, so coming back to this tab does not ask again.
    @State private var loadedKey = ""
    private static let everySource = "*"
    private var isGlobal: Bool { source == Self.everySource }

    private var info: SourceInfo? { model.sources.first { $0.id == source } }
    /// Two columns on a phone; as many as fit on a wider screen.
    private let columns = [GridItem(.adaptive(minimum: 150, maximum: 175), spacing: 14, alignment: .top)]

    var body: some View {
        MangaStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    AkiHeader(title: "Discover")
                    MangaPartPills()
                    if model.sources.isEmpty && model.repos.isEmpty {
                        WelcomeView()
                    } else if model.visibleSources.isEmpty {
                        AkiEmptyState(title: model.sources.isEmpty ? "No sources installed" : "Nothing to show", message: model.sources.isEmpty ? "Install a source from your repository in Sources to start reading." : "Your content settings hide every installed source.", action: ("Open Sources", { model.tab = .sources }))
                            .frame(minHeight: 420)
                    } else {
                        HStack(spacing: 10) {
                            AkiInput(placeholder: "Search \(isGlobal ? "every source" : info?.name ?? "")", text: $term, icon: "search", onSubmit: { submitted = term; Task { await reload() } }, onClear: { submitted = ""; Task { await reload() } })
                            if !otherFilters.isEmpty && !isGlobal {
                                AkiIconButton(symbol: "sliders", label: "Filters", tint: choices.isEmpty ? nil : Aki.primary) { filtering = true }
                            }
                        }
                        HStack(spacing: 10) {
                            if model.visibleSources.count > 1 {
                                AkiMenuPill(icon: "sources", selection: $source, options: [(Self.everySource, "All sources")] + model.visibleSources.map { ($0.id, $0.name) })
                            }
                            if let category = category, !isGlobal { categories(category) }
                        }
                        if isGlobal { globalResults } else { results }
                    }
                }
                .akiPage()
            }
            .scrollDismissesKeyboard(.immediately)
            .akiRefreshable { await reload() }
            .underTabBar()
            .background(AkiBackdrop())
            .akiNoNavigationBar()
            .akiDestination(for: SeriesRef.self) { SeriesView(ref: $0) }
            .onChange(of: source) { UserDefaults.standard.set(source, forKey: "source"); submitted = ""; term = ""; choices = [:]; filters = (try? model.engine.sourceFilters(source: source)) ?? []; Task { await reload() } }
            .akiSheet(isPresented: $filtering) { FiltersSheet(filters: otherFilters, choices: $choices) { Task { await reload() } } }
            .task(id: model.sourcesKey) {
                if !isGlobal && !model.visibleSources.contains(where: { $0.id == source }) { source = model.visibleSources.first?.id ?? "" }
                guard !source.isEmpty else { return }
                filters = (try? model.engine.sourceFilters(source: source)) ?? []
                // Coming back from a series runs this again: keep the titles, the pages asked for and the scroll place.
                if loadedKey == model.sourcesKey && (isGlobal ? !feed.shelves.isEmpty : !browse.items.isEmpty) {
                    reading = (try? model.engine.history(limit: 24)) ?? []
                    return
                }
                loadedKey = model.sourcesKey
                await reload()
            }
        }
    }

    @ViewBuilder private var globalResults: some View {
        if submitted.isEmpty {
            allSourcesFeed
        } else {
            GlobalSearchResults(model: global)
        }
    }

    // MARK: the feed

    /// The first select filter a source defines (usually its genres), shown as a row of chips.
    private var category: FilterInfo? { filters.first { $0.kind == .select && $0.options.count > 1 } }
    /// Everything else a source lets a search be narrowed by, behind the one filter button; the category has its own pill.
    private var otherFilters: [FilterInfo] { filters.filter { $0.id != category?.id } }

    private func categories(_ filter: FilterInfo) -> some View {
        AkiMenuPill(icon: "filter", placeholder: filter.name, selection: Binding(
            get: { choices[filter.id] ?? "" },
            set: { value in
                choices[filter.id] = value.isEmpty ? nil : value
                Task { await reload() }
            }
        ), options: filter.options.contains { $0.value.isEmpty } ? filter.options.map { ($0.value, $0.name) } : [("", "All")] + filter.options.map { ($0.value, $0.name) })
    }

    /// One card per series: the history has a row for every chapter read, newest first, so the same series appears many
    /// times and only its latest row counts. Two cards with one id also break the carousel's layout.
    private var continueReading: [HistoryEntry] {
        var seen = Set<String>()
        return reading.filter { seen.insert($0.key).inserted }.filter { !$0.finished }
    }

    @ViewBuilder private var continueShelf: some View {
        if !continueReading.isEmpty {
            AkiShelf(title: "Continue reading") {
                ForEach(continueReading, id: \.key) { entry in
                    AkiLink(value: SeriesRef(source: entry.source, id: entry.series, title: entry.title, cover: entry.cover)) {
                        AkiPoster(title: entry.title, cover: entry.cover, caption: chapterName(number: entry.chapterNumber, title: entry.chapterTitle))
                    }
                }
            }
        }
    }

    /// Every source at once: one big title, what you were reading, then a popular and a new shelf per source.
    @ViewBuilder private var allSourcesFeed: some View {
        if feed.shelves.isEmpty && feed.loading {
            AkiSpinner().frame(maxWidth: .infinity, minHeight: 280)
        } else if feed.shelves.isEmpty {
            AkiEmptyState(title: "Nothing to show", message: "None of your sources gave any titles. Pull down to try again.").frame(minHeight: 320)
        } else {
            if let lead = feed.shelves.first(where: { $0.id.hasSuffix("popular") })?.items.first, let from = feed.shelves.first(where: { $0.id.hasSuffix("popular") }) {
                AkiLink(value: SeriesRef(source: from.source, id: lead.id, title: lead.title, cover: lead.cover)) {
                    AkiBillboard(title: lead.title, cover: lead.cover, tag: model.name(of: from.source))
                }
            }
            continueShelf
            ForEach(feed.shelves) { shelf in
                AkiShelf(title: shelf.title, subtitle: shelf.subtitle) {
                    ForEach(shelf.items, id: \.id) { card in
                        AkiLink(value: SeriesRef(source: shelf.source, id: card.id, title: card.title, cover: card.cover)) {
                            AkiPoster(title: card.title, cover: card.cover, caption: chaptersLabel(card.chapters))
                        }
                    }
                }
            }
        }
    }

    @ViewBuilder private var results: some View {
        if let error = browse.error, browse.items.isEmpty {
            AkiEmptyState(title: error.title, message: error.message, action: ("Try again", { Task { await reload() } })).frame(minHeight: 320)
        } else if browse.items.isEmpty && !browse.loading {
            AkiEmptyState(title: submitted.isEmpty ? "Nothing to show" : "No results", message: submitted.isEmpty ? "This source gave no titles." : "Nothing matched “\(submitted)” in \(info?.name ?? source).").frame(minHeight: 320)
        } else if browse.items.isEmpty {
            AkiSpinner().frame(maxWidth: .infinity, minHeight: 240)
        } else if submitted.isEmpty && choices.isEmpty {
            sourceFeed
        } else {
            grid(browse.items)
            if browse.hasNext {
                AkiButton(browse.loading ? "Loading…" : "Show more") { Task { await load(reset: false) } }.frame(maxWidth: .infinity)
            }
        }
    }

    /// One source: its leading title, what you were reading, its top ten, what is new, then everything else.
    @ViewBuilder private var sourceFeed: some View {
        if let first = browse.items.first {
            AkiLink(value: SeriesRef(source: source, id: first.id, title: first.title, cover: first.cover)) {
                AkiBillboard(title: first.title, cover: first.cover, tag: "Popular on \(info?.name ?? "this source")")
            }
        }
        continueShelf
        AkiShelf(title: "Top 10", subtitle: info?.name) {
            ForEach(Array(browse.items.prefix(10).enumerated()), id: \.element.id) { index, card in
                AkiLink(value: SeriesRef(source: source, id: card.id, title: card.title, cover: card.cover)) {
                    AkiPoster(title: card.title, cover: card.cover, caption: chaptersLabel(card.chapters), rank: index + 1, width: 116)
                }
            }
        }
        if !feed.latest.isEmpty {
            AkiShelf(title: "New releases") {
                ForEach(feed.latest, id: \.id) { card in
                    AkiLink(value: SeriesRef(source: source, id: card.id, title: card.title, cover: card.cover)) {
                        AkiPoster(title: card.title, cover: card.cover, caption: chaptersLabel(card.chapters))
                    }
                }
            }
        }
        if browse.items.count > 10 {
            AkiText("Browse all", .heading)
            grid(Array(browse.items.dropFirst(10)))
        }
        if browse.hasNext {
            AkiButton(browse.loading ? "Loading…" : "Show more") { Task { await load(reset: false) } }.frame(maxWidth: .infinity)
        }
    }

    private func grid(_ cards: [Card]) -> some View {
        LazyVGrid(columns: columns, spacing: 22) {
            ForEach(cards, id: \.id) { card in
                AkiLink(value: SeriesRef(source: source, id: card.id, title: card.title, cover: card.cover)) {
                    AkiCoverCard(title: card.title, cover: card.cover, subtitle: chaptersLabel(card.chapters) ?? info?.name)
                }
            }
        }
    }

    private func reload() async {
        reading = (try? model.engine.history(limit: 24)) ?? []
        if isGlobal {
            if submitted.isEmpty {
                global.clear()
                await feed.loadAll(model.engine, report: model.report, sources: model.visibleSources)
            } else {
                await global.search(model.engine, sources: model.visibleSources, term: submitted, report: model.report)
            }
        } else {
            // New releases load beside the popular list, not after it.
            let latest = Task { if let info { await feed.loadLatest(model.engine, report: model.report, source: info) } }
            await load(reset: true)
            await latest.value
        }
    }

    private func load(reset: Bool) async {
        guard let info else { return }
        await browse.load(model.engine, report: model.report, source: info, term: submitted, choices: choices.map { Choice(id: $0.key, value: $0.value) }.sorted { $0.id < $1.id }, reset: reset)
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
