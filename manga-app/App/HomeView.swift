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

    func load(_ engine: Engine, source: String, mode: Mode, term: String, reset: Bool) async {
        let newKey = "\(source)|\(mode)|\(term)"
        if reset || newKey != key { items = []; page = 0; hasNext = false; key = newKey; error = nil }
        guard !loading else { return }
        loading = true
        defer { loading = false }
        do {
            let next = page + 1
            let result: CardPage
            if !term.isEmpty {
                result = try await engine.search(source: source, term: term, page: next)
            } else if mode == .latest {
                result = try await engine.latest(source: source, page: next)
            } else {
                result = try await engine.popular(source: source, page: next)
            }
            guard key == newKey else { return }
            page = next
            items += result.items.filter { new in !items.contains { $0.id == new.id } }
            hasNext = result.hasNext
        } catch {
            guard key == newKey else { return }
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

    private var info: SourceInfo? { model.sources.first { $0.id == source } }
    private let columns = [GridItem(.flexible(), spacing: 14, alignment: .top), GridItem(.flexible(), spacing: 14, alignment: .top)]

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    AkiHeader(title: "Discover", subtitle: info?.name)
                    if model.sources.isEmpty {
                        AkiEmptyState(title: "No sources yet", message: "Aki Manga reads what a source plugin describes, and it comes with none. Add a repository in Sources to get started.", action: ("Open Sources", { model.tab = .sources }))
                            .frame(minHeight: 420)
                    } else {
                        AkiInput(placeholder: "Search \(info?.name ?? "")", text: $term, icon: "magnifyingglass", onSubmit: { submitted = term; Task { await reload() } }, onClear: { submitted = ""; Task { await reload() } })
                        if model.sources.count > 1 { AkiChips(items: model.sources.map { ($0.id, $0.name) }, selection: $source) }
                        if info?.canLatest == true && submitted.isEmpty {
                            AkiTabs(items: BrowseModel.Mode.allCases.map { ($0, $0.rawValue) }, selection: $mode)
                        }
                        results
                    }
                }
                .padding(.horizontal, 20).padding(.top, 8)
            }
            .scrollDismissesKeyboard(.interactively)
            .underTabBar()
            .background(AkiBackdrop())
            .akiNoNavigationBar()
            .akiDestination(for: SeriesRef.self) { SeriesView(ref: $0) }
            .onChange(of: source) { submitted = ""; term = ""; Task { await reload() } }
            .onChange(of: mode) { Task { await reload() } }
            .task(id: model.revision) {
                if !model.sources.contains(where: { $0.id == source }) { source = model.sources.first?.id ?? "" }
                guard !source.isEmpty else { return }
                await reload()
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
        } else {
            if submitted.isEmpty, let first = browse.items.first { featured(first) }
            LazyVGrid(columns: columns, spacing: 22) {
                ForEach(submitted.isEmpty ? Array(browse.items.dropFirst()) : browse.items, id: \.id) { card in
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
                LinearGradient(colors: [.clear, .black.opacity(0.7)], startPoint: .center, endPoint: .bottom)
                VStack(alignment: .leading, spacing: 8) {
                    AkiBadge(mode == .latest ? "New" : "Popular", icon: "sparkles", style: .glass)
                    AkiText(card.title, .title, tone: .onImage, lines: 2)
                }.padding(18)
            }
            .frame(height: 320)
            .clipShape(RoundedRectangle(cornerRadius: 30, style: .continuous))
        }
    }

    private func reload() async { await load(reset: true) }

    private func load(reset: Bool) async {
        await browse.load(model.engine, source: source, mode: mode, term: submitted, reset: reset)
    }
}
