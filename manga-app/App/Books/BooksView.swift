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
    private let columns = [GridItem(.flexible(), spacing: 14, alignment: .top), GridItem(.flexible(), spacing: 14, alignment: .top)]

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    AkiHeader(title: "Books", subtitle: subtitle) {
                        AkiIconButton(symbol: "add", label: "Add books", tint: Aki.primary) { importing = true }
                    }
                    if books.isEmpty {
                        AkiEmptyState(title: "No books yet", message: "Add EPUB files from your device. They stay on this phone until you remove them.", action: ("Add books", { importing = true })).frame(minHeight: 380)
                    } else {
                        LazyVGrid(columns: columns, spacing: 22) {
                            ForEach(books, id: \.id) { book in
                                AkiTap(action: { reading = book }) {
                                    AkiBookCard(title: book.title, author: book.authors.first, cover: book.cover, subtitle: progressLine(book))
                                }
                                .akiContextMenu([AkiMenuItem(title: "Remove", icon: "trash", destructive: true) { removing = book }])
                            }
                        }
                    }
                }
                .padding(.horizontal, 20).padding(.top, 8)
            }
            .underTabBar()
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
        .akiFullScreen(item: $reading, onDismiss: load) { book in BookReaderScreen(book: book) }
        .akiAlert(problem?.title ?? "", message: problem?.message, isPresented: Binding(get: { problem != nil }, set: { if !$0 { problem = nil } }))
        .akiConfirm("Remove “\(removing?.title ?? "")”?", message: "Its file is deleted from this phone. Your place in it is forgotten.", confirm: "Remove", isPresented: Binding(get: { removing != nil }, set: { if !$0 { removing = nil } })) {
            if let removing { try? model.engine.removeBook(id: removing.id) }
            load()
        }
    }

    private var subtitle: String {
        if let adding { return "Adding \(adding.done + 1) of \(adding.total)…" }
        return books.count == 1 ? "1 book" : "\(books.count) books"
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
