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

struct BrowseView: View {
    @Environment(AppModel.self) private var model
    @State private var browse = BrowseModel()
    @State private var source = UserDefaults.standard.string(forKey: "source") ?? "mangadex"
    @State private var mode = BrowseModel.Mode.popular
    @State private var term = ""
    @State private var submitted = ""

    private var info: SourceInfo? { model.sources.first { $0.id == source } }

    var body: some View {
        NavigationStack {
            ScrollView {
                if let error = browse.error, browse.items.isEmpty {
                    StateCard(title: error.title, message: error.message, action: ("Try again", { Task { await reload() } }))
                        .frame(minHeight: 420)
                } else if browse.items.isEmpty && !browse.loading {
                    StateCard(title: submitted.isEmpty ? "Nothing to show" : "No results", message: submitted.isEmpty ? "This source gave no titles." : "Nothing matched “\(submitted)” in \(info?.name ?? source).")
                        .frame(minHeight: 420)
                } else {
                    LazyVGrid(columns: gridColumns, spacing: 18) {
                        ForEach(browse.items, id: \.id) { card in
                            NavigationLink(value: SeriesRef(source: source, id: card.id, title: card.title, cover: card.cover)) {
                                CoverTile(title: card.title, cover: card.cover)
                            }.buttonStyle(.plain)
                        }
                    }.padding(16)
                    if browse.hasNext {
                        Button(browse.loading ? "Loading…" : "Show more") { Task { await load(reset: false) } }
                            .buttonStyle(.glass).padding(.bottom, 24)
                    }
                }
                if browse.loading && browse.items.isEmpty { ProgressView().padding(60) }
            }
            .background(Aki.canvas)
            .navigationTitle(info?.name ?? "Browse")
            .navigationBarTitleDisplayMode(.large)
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Menu {
                        Picker("Source", selection: $source) { ForEach(model.sources, id: \.id) { Text($0.name).tag($0.id) } }
                        if info?.canLatest == true {
                            Picker("List", selection: $mode) { ForEach(BrowseModel.Mode.allCases, id: \.self) { Text($0.rawValue).tag($0) } }
                        }
                    } label: { Image(systemName: "line.3.horizontal.decrease") }
                }
            }
            .searchable(text: $term, placement: .navigationBarDrawer(displayMode: .always), prompt: "Search \(info?.name ?? "")")
            .onSubmit(of: .search) { submitted = term; Task { await reload() } }
            .onChange(of: term) { if term.isEmpty && !submitted.isEmpty { submitted = ""; Task { await reload() } } }
            .onChange(of: source) { submitted = ""; term = ""; Task { await reload() } }
            .onChange(of: mode) { Task { await reload() } }
            .navigationDestination(for: SeriesRef.self) { SeriesView(ref: $0) }
            .task(id: model.sources.count) {
                if !model.sources.contains(where: { $0.id == source }), let first = model.sources.first { source = first.id }
                await reload()
            }
        }
    }

    private func reload() async { await load(reset: true) }

    private func load(reset: Bool) async {
        await browse.load(model.engine, source: source, mode: mode, term: submitted, reset: reset)
    }
}
