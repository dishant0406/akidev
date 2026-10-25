import AkiManga
import SwiftUI
import UniformTypeIdentifiers

extension BookInfo: @retroactive Identifiable {}

/// The books on this device: add one from Files, pick one up where you left it.
struct BooksView: View {
    #if DEBUG
    nonisolated(unsafe) private static var openedByArgument = false
    #endif
    @Environment(AppModel.self) private var model
    @State private var books: [BookInfo] = []
    @State private var importing = false
    /// What is being added now, for the line under the title.
    @State private var adding: (done: Int, total: Int)?
    @State private var problem: (title: String, message: String)?
    @State private var reading: BookInfo?
    @State private var removing: BookInfo?
    @State private var categories: [CategoryInfo] = []
    /// -1 is every category, -2 the books on none.
    @State private var category: Int64 = -1
    @State private var managing = false
    /// The book whose categories are being changed, and the one a cover picture is being chosen for.
    @State private var sorting: BookInfo?
    @State private var coverFor: BookInfo?
    @State private var choosingCover = false
    @State private var filter = BookFilter.all
    @State private var searching = false
    @State private var words = ""
    @State private var selecting = false
    @State private var picked: Set<String> = []
    @State private var assigning: [BookInfo] = []
    @State private var details: BookInfo?
    @State private var detailMarks: [BookHighlight] = []
    @State private var removingPicked: [BookInfo] = []
    /// Two columns on a phone; as many as fit on a wider screen.
    private let columns = [GridItem(.adaptive(minimum: 150, maximum: 190), spacing: 14, alignment: .top)]

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    AkiHeader(title: model.prefs.readerKind == .books ? "Library" : "Books", subtitle: subtitle) {
                        AkiGlassGroup(spacing: 10) {
                            HStack(spacing: 10) {
                                AkiMenu(symbol: "filter", label: "Sort and filter", items: sortItems)
                                AkiIconButton(symbol: "add", label: "Add books", tint: Aki.primary) { importing = true }
                            }
                        }
                    }
                    if searching {
                        AkiInput(placeholder: "Search your books", text: $words, icon: "search", onClear: { words = "" })
                    }
                    if !categories.isEmpty && !books.isEmpty {
                        AkiChips(items: [(Int64(-1), "All")] + categories.map { ($0.id, "\($0.name) \($0.books)") } + [(Int64(-2), "No category")], selection: $category)
                    }
                    if books.isEmpty {
                        AkiEmptyState(title: "No books yet", message: "Add EPUB files from your device. They stay on this phone until you remove them.", action: ("Add books", { importing = true })).frame(minHeight: 380)
                    } else {
                        if shown.isEmpty {
                            AkiEmptyState(title: "Nothing here", message: category == -2 ? "Every book is on a category." : "No books on this shelf yet. Long-press a book to put it on one.").frame(minHeight: 260)
                        }
                        LazyVGrid(columns: columns, spacing: 22) {
                            ForEach(shown, id: \.id) { book in
                                AkiTap(action: { if selecting { toggle(book) } else { reading = book } }) {
                                    AkiBookCard(title: book.title, author: book.authors.first, cover: book.cover, subtitle: progressLine(book))
                                        .overlay(alignment: .topLeading) { if selecting { tick(picked.contains(book.id)) } }
                                }
                                .akiContextMenu(selecting ? [] : menu(for: book))
                            }
                        }
                    }
                }
                .akiPage()
            }
            .underTabBar()
            .overlay(alignment: .bottom) { if selecting && !picked.isEmpty { selectionBar } }
            .background(AkiBackdrop())
            .akiNoNavigationBar()
        }
        .onAppear(perform: load)
        #if DEBUG
        // `-openBook 2` opens the third book on the shelf, `-openBook Pride` the first whose title has that word, to drive the reader
        // from a script.
        .task {
            // Once: the tab appears again each time it is come back to, and the book must not open again with it.
            guard let which = UserDefaults.standard.string(forKey: "openBook"), !Self.openedByArgument else { return }
            Self.openedByArgument = true
            load()
            func pick() -> BookInfo? { Int(which).flatMap { books.indices.contains($0) ? books[$0] : nil } ?? books.first { $0.title.localizedCaseInsensitiveContains(which) } }
            for _ in 0..<20 where pick() == nil { try? await Task.sleep(for: .milliseconds(200)) }
            reading = pick()
        }
        #endif
        .onChange(of: model.addingBook, initial: true) { _, now in if now { model.addingBook = false; importing = true } }
        .akiFilesImporter(isPresented: $importing, types: [.epub], onPick: add)
        .akiSheet(isPresented: $managing, content: { CategoriesSheet().onDisappear(perform: load) })
        .akiSheet(item: $sorting, content: { book in CategoryPicker(series: [], books: [book]).onDisappear(perform: load) })
        .akiSheet(isPresented: Binding(get: { !assigning.isEmpty }, set: { if !$0 { assigning = [] } }), content: { CategoryPicker(series: [], books: assigning).onDisappear { load(); picked = []; selecting = false } })
        .akiSheet(item: $details, content: { book in detailSheet(book) })
        .akiPhotoPicker(isPresented: $choosingCover, width: 300) { jpeg in
            if let book = coverFor { try? model.engine.setBookCustomCover(id: book.id, jpeg: jpeg) }
            load()
        }
        .akiFullScreen(item: $reading, onDismiss: load) { book in BookReaderScreen(book: book) }
        .akiAlert(problem?.title ?? "", message: problem?.message, isPresented: Binding(get: { problem != nil }, set: { if !$0 { problem = nil } }))
        .akiConfirm("Remove \(removingPicked.count) books?", message: "Their files are deleted from this phone. Your place in them is forgotten.", confirm: "Remove", isPresented: Binding(get: { !removingPicked.isEmpty }, set: { if !$0 { removingPicked = [] } })) {
            for book in removingPicked { try? model.engine.removeBook(id: book.id) }
            removingPicked = []; picked = []; selecting = false
            load()
        }
        .akiConfirm("Remove “\(removing?.title ?? "")”?", message: "Its file is deleted from this phone. Your place in it is forgotten.", confirm: "Remove", isPresented: Binding(get: { removing != nil }, set: { if !$0 { removing = nil } })) {
            if let removing { try? model.engine.removeBook(id: removing.id) }
            load()
        }
    }

    private var subtitle: String {
        if let adding { return "Adding \(adding.done + 1) of \(adding.total)…" }
        return books.count == 1 ? "1 book" : "\(books.count) books"
    }

    private var shown: [BookInfo] {
        shelf(books, sort: model.prefs.bookSort, reversed: model.prefs.bookReversed, filter: filter, category: category, words: words)
    }

    private var sortItems: [AkiMenuItem] {
        BookSort.allCases.map { option in AkiMenuItem(title: option.rawValue, icon: "order", isSelected: option == model.prefs.bookSort) { model.prefs.bookSort = option } }
            + [AkiMenuItem(title: "Reverse order", icon: "swap", isSelected: model.prefs.bookReversed) { model.prefs.bookReversed.toggle() }]
            + BookFilter.allCases.map { option in AkiMenuItem(title: option.rawValue, icon: "filter", isSelected: option == filter) { filter = option } }
            + [AkiMenuItem(title: searching ? "Hide search" : "Search", icon: "search") { searching.toggle(); if !searching { words = "" } },
               AkiMenuItem(title: selecting ? "Done selecting" : "Select", icon: "check") { selecting.toggle(); picked = [] },
               AkiMenuItem(title: "Categories", icon: "folder") { managing = true }]
    }

    private func toggle(_ book: BookInfo) {
        if !picked.insert(book.id).inserted { picked.remove(book.id) }
    }

    private func tick(_ on: Bool) -> some View {
        AkiIcon(on ? "check-solid" : "check", size: 22, tone: on ? .onPrimary : .onMedia)
            .padding(6)
            .background(on ? Aki.primary : Aki.scrim, in: Circle())
            .padding(8)
    }

    /// What can be done to the books picked: put them on categories, mark them finished or not started, remove them.
    private var selectionBar: some View {
        let chosen = books.filter { picked.contains($0.id) }
        return AkiGlassGroup(spacing: 12) {
            HStack(spacing: 12) {
                AkiIconButton(symbol: "folder", label: "Categories") { assigning = chosen }
                AkiIconButton(symbol: "check", label: "Mark as finished") { for book in chosen { try? model.engine.finishBook(id: book.id) }; picked = []; selecting = false; load() }
                AkiIconButton(symbol: "restore", label: "Mark as not started") { for book in chosen { try? model.engine.resetBook(id: book.id) }; picked = []; selecting = false; load() }
                AkiIconButton(symbol: "trash", label: "Remove", tint: Aki.destructive) { removingPicked = chosen }
            }
        }
        .padding(.bottom, 112)
    }

    private func detailSheet(_ book: BookInfo) -> some View {
        BookDetailSheet(
            book: book, marks: detailMarks,
            read: { details = nil; reading = book },
            finish: { try? model.engine.finishBook(id: book.id); details = nil; load() },
            reset: { try? model.engine.resetBook(id: book.id); details = nil; load() },
            categories: { details = nil; sorting = book },
            chooseCover: { details = nil; coverFor = book; choosingCover = true },
            ownCover: { try? model.engine.clearBookCustomCover(id: book.id); details = nil; load() },
            remove: { details = nil; removing = book }
        )
    }

    private func menu(for book: BookInfo) -> [AkiMenuItem] {
        var items = [AkiMenuItem(title: "Details", icon: "list") { detailMarks = (try? model.engine.bookMarks(book: book.id)) ?? []; details = book },
                     AkiMenuItem(title: "Categories", icon: "folder") { sorting = book },
                     AkiMenuItem(title: "Choose a cover picture", icon: "image") { coverFor = book; choosingCover = true }]
        if book.customCover {
            items.append(AkiMenuItem(title: "Use the book's own cover", icon: "restore") { try? model.engine.clearBookCustomCover(id: book.id); load() })
        }
        items.append(AkiMenuItem(title: "Remove", icon: "trash", destructive: true) { removing = book })
        return items
    }

    private func progressLine(_ book: BookInfo) -> String {
        if book.finished { return "Finished" }
        if book.openedMs == nil { return "New" }
        return "\(Int((book.progress * 100).rounded()))%"
    }

    private func load() {
        let engine = model.engine
        Task {
            let found = await Task.detached { (try? engine.books()) ?? [] }.value
            if found != books { books = found }
            categories = (try? engine.categories()) ?? []
            if category >= 0 && !categories.contains(where: { $0.id == category }) || category == -2 && categories.isEmpty { category = -1 }
        }
    }

    /// Adds the files one after another, each off the main thread; a book that cannot be read says why and the rest go on.
    private func add(_ files: [URL]) {
        guard !files.isEmpty else { return }
        let engine = model.engine
        adding = (0, files.count)
        Task {
            var failures: [String] = []
            for (index, file) in files.enumerated() {
                adding = (index, files.count)
                let result = await Task.detached { Result { try engine.importBook(path: file.path) } }.value
                if case .failure(let error) = result { failures.append("\(file.deletingPathExtension().lastPathComponent): \(describe(error).message)") }
                try? FileManager.default.removeItem(at: file.deletingLastPathComponent())
            }
            adding = nil
            load()
            if !failures.isEmpty {
                problem = (failures.count == 1 ? "A book could not be added" : "\(failures.count) books could not be added", failures.joined(separator: "\n"))
            }
        }
    }
}

/// Opens the book and shows it, or says why it cannot be shown.
struct BookReaderScreen: View {
    let book: BookInfo
    @Environment(AppModel.self) private var app
    @State private var model: BookModel?
    @State private var failure: String?
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        Group {
            if let model {
                BookReaderView(model: model, close: { dismiss() })
            } else if let failure {
                AkiEmptyState(title: "This book cannot be opened", message: failure, action: ("Close", { dismiss() }))
            } else {
                AkiOpeningLoader(label: book.title)
            }
        }
        .task {
            guard model == nil else { return }
            let prefs = app.prefs
            #if DEBUG
            // `-bookInk YES`, `-bookWarmth always`, `-bookBrightness 0.1`, `-bookTheme ink`, `-bookScroll YES`: the reader's comfort settings, from a script.
            let defaults = UserDefaults.standard
            if defaults.object(forKey: "bookInk") != nil { prefs.bookStyle.ink = defaults.bool(forKey: "bookInk") }
            if let warmth = defaults.string(forKey: "bookWarmth").flatMap(BookWarmth.init(rawValue:)) { prefs.bookStyle.warmth = warmth }
            if defaults.object(forKey: "bookScroll") != nil { prefs.bookStyle.scroll = defaults.bool(forKey: "bookScroll") }
            if defaults.object(forKey: "bookBrightness") != nil { prefs.bookStyle.brightness = defaults.double(forKey: "bookBrightness") }
            if let theme = defaults.string(forKey: "bookTheme").flatMap(BookTheme.init(rawValue:)) { prefs.bookStyle.theme = theme }
            #endif
            do {
                model = try await BookModel.open(book, engine: app.engine, style: prefs.bookStyle, invertTaps: prefs.invertTaps, saveStyle: { prefs.bookStyle = $0 })
            } catch { failure = describe(error).message }
        }
    }
}
