import AkiManga
import SwiftUI

/// Where books are found: one search across every catalogue the person added, and the catalogues themselves to open.
struct BooksBrowseView: View {
    @Environment(AppModel.self) private var model
    @State private var catalogues: [CatalogueInfo] = []
    @State private var term = ""
    @State private var search = BookSearchAcross()
    @State private var adding = false
    @State private var address = ""
    @State private var problem: (title: String, message: String)?

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 20) {
                    AkiHeader(title: "Browse", subtitle: catalogues.isEmpty ? "Find books to read" : (catalogues.count == 1 ? "1 catalogue" : "\(catalogues.count) catalogues")) {
                        AkiIconButton(symbol: "add", label: "Add a catalogue", tint: Aki.primary) { adding = true }
                    }
                    if catalogues.isEmpty {
                        AkiEmptyState(title: "No catalogue yet", message: "A catalogue is a shop or library of books that shares an OPDS feed, such as a Calibre-Web server or Standard Ebooks. Add one by its address. Akidev does not include or endorse any.", action: ("Add a catalogue", { adding = true })).frame(minHeight: 340)
                    } else {
                        AkiInput(placeholder: "Search every catalogue", text: $term, icon: "search", onSubmit: { Task { await search.run(model.engine, catalogues: catalogues, term: term) } }, onClear: { search.clear() })
                        if search.groups.isEmpty {
                            AkiSection(title: "Catalogues") {
                                ForEach(catalogues, id: \.url) { catalogue in
                                    AkiLink(value: CatalogueRef(url: catalogue.url, name: catalogue.name, search: nil)) {
                                        AkiRow(title: catalogue.name, subtitle: catalogue.url, icon: "book-solid", showsChevron: true)
                                    }
                                    .akiContextMenu([AkiMenuItem(title: "Remove", icon: "trash", destructive: true) { try? model.engine.removeCatalogue(url: catalogue.url); load() }])
                                }
                            }
                        } else {
                            results
                        }
                    }
                }
                .akiPage(narrow: true)
            }
            .underTabBar()
            .background(AkiBackdrop())
            .akiNoNavigationBar()
            .akiDestination(for: CatalogueRef.self) { CatalogueView(ref: $0) }
        }
        .onAppear(perform: load)
        .akiPrompt("Add a book catalogue", message: "The address of an OPDS catalogue, for example a Calibre-Web or Standard Ebooks feed.", placeholder: "example.org/opds", confirm: "Add", text: $address, isPresented: $adding) {
            let link = address
            address = ""
            Task {
                do {
                    _ = try await model.engine.addCatalogue(url: link)
                    load()
                } catch { problem = describe(error) }
            }
        }
        .akiAlert(problem?.title ?? "", message: problem?.message, isPresented: Binding(get: { problem != nil }, set: { if !$0 { problem = nil } }))
    }

    /// What each catalogue found, as its answer arrives.
    private var results: some View {
        VStack(alignment: .leading, spacing: 22) {
            ForEach(search.groups) { group in
                VStack(alignment: .leading, spacing: 10) {
                    AkiText(group.name, .heading)
                    switch group.state {
                    case .loading: AkiSpinner().frame(maxWidth: .infinity, minHeight: 60)
                    case .failed(let message): AkiText(message, .caption, tone: .muted)
                    case .done(let items):
                        if items.isEmpty { AkiText("Nothing found.", .caption, tone: .muted) }
                        LazyVStack(spacing: 12) { ForEach(Array(items.prefix(8).enumerated()), id: \.offset) { _, item in CatalogueItemRow(item: item, search: group.search) } }
                    }
                }
            }
        }
    }

    private func load() { catalogues = (try? model.engine.catalogues()) ?? [] }
}

/// One search sent to every catalogue that can be searched; each answers on its own and a slow one does not hold the others.
@MainActor @Observable
final class BookSearchAcross {
    enum State { case loading, done([CatalogueItem]), failed(String) }
    struct Group: Identifiable {
        let id: String
        let name: String
        var search: String?
        var state: State
    }

    private(set) var groups: [Group] = []
    private var term = ""

    func clear() {
        term = ""
        groups = []
    }

    func run(_ engine: Engine, catalogues: [CatalogueInfo], term words: String) async {
        let words = words.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !words.isEmpty else { return clear() }
        term = words
        groups = catalogues.map { Group(id: $0.url, name: $0.name, search: nil, state: .loading) }
        await withTaskGroup(of: (String, String?, State).self) { group in
            for catalogue in catalogues {
                group.addTask {
                    do {
                        // A catalogue says where it is searched on its first page.
                        guard let address = try await engine.catalogueFeed(url: catalogue.url).search else { return (catalogue.url, nil, .failed("This catalogue cannot be searched.")) }
                        return (catalogue.url, address, .done(try await engine.catalogueSearch(search: address, term: words).items))
                    } catch { return (catalogue.url, nil, .failed(describe(error).message)) }
                }
            }
            for await (url, address, state) in group where term == words {
                if let at = groups.firstIndex(where: { $0.id == url }) {
                    groups[at].search = address
                    groups[at].state = state
                }
            }
        }
    }
}
