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
    @FocusState private var searching: Bool

    private var info: SourceInfo? { model.sources.first { $0.id == source } }
    private let columns = [GridItem(.flexible(), spacing: 14, alignment: .top), GridItem(.flexible(), spacing: 14, alignment: .top)]

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    ScreenHeader(title: "Discover", subtitle: info?.name) { EmptyView() }
                    if model.sources.isEmpty {
                        StateCard(title: "No sources yet", message: "Aki Manga reads what a source plugin describes, and it comes with none. Add a repository in Sources to get started.", action: ("Open Sources", { model.tab = .sources }))
                            .frame(minHeight: 420)
                    } else {
                        searchField
                        if model.sources.count > 1 { sourceChips }
                        if info?.canLatest == true && submitted.isEmpty {
                            PillTabs(items: BrowseModel.Mode.allCases.map { ($0, $0.rawValue) }, selection: $mode)
                        }
                        results
                    }
                }
                .padding(.horizontal, 20).padding(.top, 8)
            }
            .scrollDismissesKeyboard(.interactively)
            .underTabBar()
            .background(AmbientBackground())
            .toolbar(.hidden, for: .navigationBar)
            .navigationDestination(for: SeriesRef.self) { SeriesView(ref: $0) }
            .onChange(of: source) { submitted = ""; term = ""; Task { await reload() } }
            .onChange(of: mode) { Task { await reload() } }
            .task(id: model.revision) {
                if !model.sources.contains(where: { $0.id == source }) { source = model.sources.first?.id ?? "" }
                guard !source.isEmpty else { return }
                await reload()
            }
        }
    }

    private var searchField: some View {
        HStack(spacing: 10) {
            Image(systemName: "magnifyingglass").font(.system(size: 16, weight: .semibold)).foregroundStyle(Aki.dim)
            TextField("Search \(info?.name ?? "")", text: $term)
                .font(.system(size: 16, design: .rounded)).foregroundStyle(Aki.ink)
                .submitLabel(.search).focused($searching).autocorrectionDisabled().textInputAutocapitalization(.never)
                .onSubmit { submitted = term; Task { await reload() } }
            if !term.isEmpty {
                Button { term = ""; submitted = ""; Task { await reload() } } label: { Image(systemName: "xmark.circle.fill").foregroundStyle(Aki.dim) }.buttonStyle(.plain)
            }
        }
        .padding(.horizontal, 18).frame(height: 52)
        .glassEffect(.regular, in: .capsule)
    }

    private var sourceChips: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            GlassEffectContainer(spacing: 8) {
                HStack(spacing: 8) {
                    ForEach(model.sources, id: \.id) { item in
                        let on = item.id == source
                        Button { withAnimation(.smooth(duration: 0.25)) { source = item.id } } label: {
                            Text(item.name).font(.system(size: 14, weight: on ? .bold : .medium, design: .rounded))
                                .foregroundStyle(on ? Aki.onYellow : Aki.ink).padding(.horizontal, 16).frame(height: 38).contentShape(Capsule())
                        }
                        .buttonStyle(.plain)
                        .glassEffect(on ? .regular.tint(Aki.yellow).interactive() : .regular.interactive(), in: .capsule)
                    }
                }
            }
        }
        .scrollClipDisabled()
    }

    @ViewBuilder private var results: some View {
        if let error = browse.error, browse.items.isEmpty {
            StateCard(title: error.title, message: error.message, action: ("Try again", { Task { await reload() } })).frame(minHeight: 320)
        } else if browse.items.isEmpty && !browse.loading {
            StateCard(title: submitted.isEmpty ? "Nothing to show" : "No results", message: submitted.isEmpty ? "This source gave no titles." : "Nothing matched “\(submitted)” in \(info?.name ?? source).").frame(minHeight: 320)
        } else if browse.items.isEmpty {
            ProgressView().frame(maxWidth: .infinity, minHeight: 240)
        } else {
            if submitted.isEmpty, let first = browse.items.first { featured(first) }
            LazyVGrid(columns: columns, spacing: 22) {
                ForEach(submitted.isEmpty ? Array(browse.items.dropFirst()) : browse.items, id: \.id) { card in
                    NavigationLink(value: SeriesRef(source: source, id: card.id, title: card.title, cover: card.cover)) {
                        CoverCard(title: card.title, cover: card.cover, subtitle: info?.name)
                    }.buttonStyle(.plain)
                }
            }
            if browse.hasNext {
                GlassTextButton(title: browse.loading ? "Loading…" : "Show more") { Task { await load(reset: false) } }.frame(maxWidth: .infinity)
            }
        }
    }

    /// The first title, large.
    private func featured(_ card: Card) -> some View {
        NavigationLink(value: SeriesRef(source: source, id: card.id, title: card.title, cover: card.cover)) {
            ZStack(alignment: .bottomLeading) {
                RemoteImage(url: card.cover, placeholder: 1.1, fill: true, free: true)
                LinearGradient(colors: [.clear, .black.opacity(0.7)], startPoint: .center, endPoint: .bottom)
                VStack(alignment: .leading, spacing: 8) {
                    Chip(text: mode == .latest ? "New" : "Popular", symbol: "sparkles", onImage: true)
                    Text(card.title).font(Aki.title(26)).foregroundStyle(.white).lineLimit(2).multilineTextAlignment(.leading)
                }.padding(18)
            }
            .frame(height: 320)
            .clipShape(RoundedRectangle(cornerRadius: 30, style: .continuous))
        }.buttonStyle(.plain)
    }

    private func reload() async { await load(reset: true) }

    private func load(reset: Bool) async {
        await browse.load(model.engine, source: source, mode: mode, term: submitted, reset: reset)
    }
}
