import AkiManga
import SwiftUI

/// A page of a catalogue to open: its address, its name, and how to search the catalogue it belongs to.
struct CatalogueRef: Hashable {
    let url: String
    let name: String
    let search: String?
}

/// A catalogue of books: shelves to open and books to get. Getting one downloads it onto the shelf in the Books tab.
struct CatalogueView: View {
    let ref: CatalogueRef
    @Environment(AppModel.self) private var model
    @Environment(\.dismiss) private var dismiss
    @State private var feed: CatalogueFeed?
    @State private var items: [CatalogueItem] = []
    @State private var next: String?
    @State private var term = ""
    @State private var searched: String?
    @State private var failure: (title: String, message: String)?
    @State private var loadingMore = false
    @State private var getting: Set<String> = []
    @State private var got: Set<String> = []
    @State private var problem: (title: String, message: String)?

    private var search: String? { feed?.search ?? ref.search }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                AkiBackHeader(title: searched.map { "“\($0)”" } ?? ref.name, subtitle: feed == nil ? nil : "\(items.count) shown", back: { dismiss() })
                if search != nil {
                    AkiInput(placeholder: "Search this catalogue", text: $term, icon: "search", onSubmit: { Task { await run() } }, onClear: { searched = nil; Task { await load(ref.url) } })
                }
                if let failure {
                    AkiEmptyState(title: failure.title, message: failure.message, action: ("Try again", { Task { await load(searched == nil ? ref.url : nil) } })).frame(minHeight: 260)
                } else if feed == nil {
                    AkiSpinner().frame(maxWidth: .infinity, minHeight: 200)
                } else if items.isEmpty {
                    AkiEmptyState(title: "Nothing here", message: "This page of the catalogue has no books.").frame(minHeight: 240)
                }
                LazyVStack(spacing: 12) {
                    ForEach(Array(items.enumerated()), id: \.offset) { _, item in row(item) }
                }
                if let next {
                    if loadingMore { AkiSpinner().frame(maxWidth: .infinity) } else { AkiButton("More", fullWidth: true) { Task { await more(next) } } }
                }
            }
            .padding(.horizontal, 20).padding(.top, 8)
        }
        .underTabBar()
        .background(AkiBackdrop())
        .akiNoNavigationBar()
        .hidesTabBar()
        .task { if feed == nil { await load(ref.url) } }
        .akiAlert(problem?.title ?? "", message: problem?.message, isPresented: Binding(get: { problem != nil }, set: { if !$0 { problem = nil } }))
    }

    @ViewBuilder private func row(_ item: CatalogueItem) -> some View {
        if let get = item.get {
            AkiListCard(thumbnail: item.cover, label: item.author ?? "", title: item.title) {
                if got.contains(get) {
                    AkiBadge("On your shelf", icon: "tick")
                } else if getting.contains(get) {
                    AkiSpinner()
                } else {
                    AkiButton("Get", variant: .primary, size: .small) { Task { await fetch(get) } }
                }
            }
        } else if let open = item.open {
            AkiLink(value: CatalogueRef(url: open, name: item.title, search: search)) {
                AkiListCard(thumbnail: item.cover, label: item.author ?? "Open", title: item.title) { AkiIcon("forward", size: 13, tone: .muted) }
            }
        }
    }

    private func load(_ url: String?) async {
        failure = nil
        guard let url else { return await run() }
        do {
            let found = try await model.engine.catalogueFeed(url: url)
            feed = found
            items = found.items
            next = found.next
        } catch { failure = describe(error) }
    }

    private func run() async {
        let words = term.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !words.isEmpty, let search else { return }
        failure = nil
        feed = nil
        searched = words
        do {
            let found = try await model.engine.catalogueSearch(search: search, term: words)
            feed = found
            items = found.items
            next = found.next
        } catch {
            feed = CatalogueFeed(title: "", items: [], next: nil, search: search)
            items = []
            failure = describe(error)
        }
    }

    private func more(_ url: String) async {
        loadingMore = true
        defer { loadingMore = false }
        do {
            let found = try await model.engine.catalogueFeed(url: url)
            items += found.items
            next = found.next
        } catch { problem = describe(error) }
    }

    private func fetch(_ url: String) async {
        getting.insert(url)
        defer { getting.remove(url) }
        do {
            _ = try await model.engine.getBook(url: url)
            got.insert(url)
        } catch { problem = describe(error) }
    }
}
