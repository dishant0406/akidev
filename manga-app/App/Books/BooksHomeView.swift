import AkiManga
import SwiftUI

/// The first tab for someone who mostly reads books: where they left off, what they have not started, their shelves and the
/// catalogues they can get more from.
struct BooksHomeView: View {
    @Environment(AppModel.self) private var model
    @State private var books: [BookInfo] = []
    @State private var categories: [CategoryInfo] = []
    @State private var catalogues: [CatalogueInfo] = []
    @State private var loaded = false
    @State private var reading: BookInfo?
    @State private var highlights: [BookHighlightIn] = []

    /// The book opened last that is not finished.
    private var current: BookInfo? {
        books.filter { !$0.finished && $0.openedMs != nil }.max { ($0.openedMs ?? 0) < ($1.openedMs ?? 0) }
    }

    private var unread: [BookInfo] { books.filter { $0.openedMs == nil }.sorted { $0.addedMs > $1.addedMs } }

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 26) {
                    AkiHeader(title: "Home", subtitle: books.isEmpty ? "Your reading" : (books.count == 1 ? "1 book" : "\(books.count) books")) {
                        AkiIconButton(symbol: "add", label: "Add books", tint: Aki.primary) { model.tab = .books; model.addingBook = true }
                    }
                    if loaded && books.isEmpty {
                        AkiEmptyState(title: "Your shelf is empty", message: "Add EPUB files from your device, or get books from a catalogue.", action: ("Add books", { model.tab = .books; model.addingBook = true }), secondary: ("Find a catalogue", { model.tab = .browse })).frame(minHeight: 340)
                    }
                    if let current { continueCard(current) }
                    if !unread.isEmpty {
                        AkiShelf(title: "Not started", subtitle: "Added, never opened") {
                            ForEach(unread.prefix(12), id: \.id) { book in poster(book, caption: book.authors.first) }
                        }
                    }
                    ForEach(categories.filter { shelf in books.contains { $0.categories.contains(shelf.id) } }, id: \.id) { shelf in
                        AkiShelf(title: shelf.name) {
                            ForEach(books.filter { $0.categories.contains(shelf.id) }, id: \.id) { book in poster(book, caption: progressLine(book)) }
                        }
                    }
                    if !highlights.isEmpty {
                        AkiSection(title: "Recent highlights") {
                            ForEach(highlights.prefix(3), id: \.highlight.id) { found in
                                AkiTap(action: { open(found) }) { highlightCard(found) }
                            }
                            AkiLink(value: HighlightsRoute()) { AkiButton("All highlights", icon: "forward", fullWidth: true, action: {}).allowsHitTesting(false) }
                        }
                    }
                    catalogueSection
                }
                .padding(.horizontal, 20).padding(.top, 8)
            }
            .underTabBar()
            .background(AkiBackdrop())
            .akiNoNavigationBar()
            .akiDestination(for: CatalogueRef.self) { CatalogueView(ref: $0) }
            .akiDestination(for: HighlightsRoute.self) { _ in HighlightsView(open: open) }
        }
        .onAppear(perform: load)
        .akiFullScreen(item: $reading, onDismiss: load) { book in BookReaderScreen(book: book) }
    }

    private func continueCard(_ book: BookInfo) -> some View {
        AkiTap(action: { reading = book }) {
            AkiCard(padding: 14) {
                HStack(spacing: 14) {
                    AkiImage(url: book.cover, placeholder: 1.5, fill: true)
                        .frame(width: 84)
                        .clipShape(RoundedRectangle(cornerRadius: Aki.thumbRadius, style: .continuous))
                    VStack(alignment: .leading, spacing: 8) {
                        AkiText("Continue reading", .eyebrow, tone: .muted)
                        AkiText(book.title, .heading, lines: 2)
                        if let author = book.authors.first { AkiText(author, .caption, tone: .muted, lines: 1) }
                        AkiProgressBar(fraction: book.progress)
                        AkiText("\(Int((book.progress * 100).rounded()))%", .caption, tone: .muted)
                    }
                    Spacer(minLength: 0)
                }
            }
        }
    }

    private func poster(_ book: BookInfo, caption: String?) -> some View {
        AkiTap(action: { reading = book }) { AkiPoster(title: book.title, cover: book.cover, caption: caption) }
    }

    private func highlightCard(_ found: BookHighlightIn) -> some View {
        AkiCard(padding: 14) {
            VStack(alignment: .leading, spacing: 6) {
                AkiText("“\(found.highlight.quote)”", .body, lines: 3)
                if let note = found.highlight.note, !note.isEmpty { AkiText(note, .caption, tone: .muted, lines: 2) }
                AkiText(found.title, .captionStrong, tone: .muted, lines: 1)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    /// Opens the book where the words are: its place is moved there first, then it is read as usual.
    private func open(_ found: BookHighlightIn) {
        guard let book = books.first(where: { $0.id == found.book }) else { return }
        try? model.engine.saveBookPosition(id: book.id, chapter: found.highlight.chapter, offset: found.highlight.start, progress: book.progress, finished: book.finished)
        reading = (try? model.engine.bookInfo(id: book.id)) ?? book
    }

    private func progressLine(_ book: BookInfo) -> String {
        if book.finished { return "Finished" }
        return book.openedMs == nil ? "New" : "\(Int((book.progress * 100).rounded()))%"
    }

    @ViewBuilder private var catalogueSection: some View {
        AkiSection(title: "Catalogues", footer: "Get books from a catalogue you add (an OPDS feed). Akidev does not include or endorse any.") {
            ForEach(catalogues, id: \.url) { catalogue in
                AkiLink(value: CatalogueRef(url: catalogue.url, name: catalogue.name, search: nil)) {
                    AkiRow(title: catalogue.name, subtitle: catalogue.url, icon: "book-solid", showsChevron: true)
                }
            }
            AkiButton(catalogues.isEmpty ? "Add a book catalogue" : "Manage catalogues", icon: "add", fullWidth: true) { model.tab = .browse }
        }
    }

    private func load() {
        let engine = model.engine
        Task {
            books = await Task.detached { (try? engine.books()) ?? [] }.value
            categories = (try? engine.categories()) ?? []
            catalogues = (try? engine.catalogues()) ?? []
            highlights = (try? engine.recentHighlights(limit: 3)) ?? []
            loaded = true
        }
    }
}
