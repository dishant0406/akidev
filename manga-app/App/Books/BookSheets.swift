import AkiManga
import SwiftUI

/// What can be changed about how the book looks: theme, face, size, spacing, margins. Every change shows on the page at once.
struct BookSettingsSheet: View {
    let model: BookModel

    var body: some View {
        @Bindable var model = model
        ScrollView {
            VStack(alignment: .leading, spacing: 22) {
                AkiText("Type and theme", .title)
                BookThemePicker(selection: $model.style.theme)
                VStack(alignment: .leading, spacing: 10) {
                    AkiText("Reading", .captionStrong, tone: .muted)
                    AkiTabs(items: [(false, "Pages"), (true, "Scroll")], selection: $model.style.scroll)
                }
                AkiCard(padding: 16) {
                    VStack(spacing: 18) {
                        AkiSelect(title: "Font", selection: $model.style.font, options: BookFont.allCases.map { ($0, $0.title) })
                        stepper("Text size", value: $model.style.size, range: BookStyle.sizes, small: "A", large: "A")
                        stepper("Line spacing", value: $model.style.lineSpacing, range: BookStyle.spacings, small: "−", large: "+")
                    }
                }
                VStack(alignment: .leading, spacing: 10) {
                    AkiText("Margins", .captionStrong, tone: .muted)
                    AkiTabs(items: BookMargin.allCases.map { ($0, $0.title) }, selection: $model.style.margin)
                }
                light($model)
                AkiCard(padding: 16) {
                    VStack(spacing: 16) {
                        AkiSwitch(title: "Justify text", isOn: $model.style.justify)
                        AkiSwitch(title: "Hyphenate long words", subtitle: "Fewer gaps in justified lines.", isOn: $model.style.hyphenate)
                        AkiSwitch(title: "Use the book's fonts", subtitle: "For headings and special text, when the book carries its own.", isOn: $model.style.bookFonts)
                    }
                }
            }
            .padding(24)
        }
    }

    /// Brightness, warmth and the paper-like display.
    private func light(_ model: Bindable<BookModel>) -> some View {
        AkiCard(padding: 16) {
            VStack(alignment: .leading, spacing: 18) {
                AkiSwitch(title: "Set the brightness here", subtitle: "Goes lower than the phone's lowest, and is given back when you close the book.", isOn: Binding(
                    get: { model.wrappedValue.style.brightness != nil },
                    set: { model.wrappedValue.style.brightness = $0 ? 0.6 : nil }
                ))
                if let brightness = model.wrappedValue.style.brightness {
                    stepper("Brightness", value: Binding(get: { brightness }, set: { model.wrappedValue.style.brightness = $0 }), range: 0...1, small: "−", large: "+")
                }
                VStack(alignment: .leading, spacing: 10) {
                    AkiText("Warm tint", .bodyStrong)
                    AkiTabs(items: BookWarmth.allCases.map { ($0, $0.title) }, selection: model.style.warmth)
                }
                if model.wrappedValue.style.warmth != .off {
                    stepper("Warmth", value: model.style.warmthAmount, range: 0.1...0.8, small: "−", large: "+")
                }
                AkiSwitch(title: "Ink display", subtitle: "Grey, still and quiet like paper: no page-turn motion.", isOn: model.style.ink)
            }
        }
    }

    private func stepper(_ title: String, value: Binding<Double>, range: ClosedRange<Double>, small: String, large: String) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            AkiText(title, .bodyStrong)
            HStack(spacing: 12) {
                AkiText(small, .caption, tone: .muted)
                AkiSlider(value: value, range: range)
                AkiText(large, .heading, tone: .muted)
            }
        }
    }
}

/// The book's table of contents, and what you highlighted: tap an entry to go there. The part being read is yellow.
struct BookContentsSheet: View {
    let model: BookModel
    let close: () -> Void
    @State private var tab = Tab.contents
    @State private var exported: URL?
    /// The front and back matter are folded away until asked for.
    @State private var showFront = false
    @State private var showBack = false

    private enum Tab: Hashable { case contents, marks, about }

    var body: some View {
        ScrollViewReader { proxy in
            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    VStack(alignment: .leading, spacing: 4) {
                        AkiText(tab == .contents ? "Contents" : tab == .marks ? "Highlights" : "About", .title)
                        AkiText(model.info.title, .caption, tone: .muted, lines: 1)
                    }
                    AkiTabs(items: [(Tab.contents, "Contents"), (Tab.marks, "Marks \(model.marks.count)"), (Tab.about, "About")], selection: $tab)
                    switch tab {
                    case .contents:
                        LazyVStack(spacing: 10) {
                            if hasFront {
                                fold("Before the book", count: frontCount, open: $showFront)
                                if showFront { ForEach(indices(of: .front), id: \.self) { index in row(model.contents[index], current: index == currentIndex).id(index) } }
                            }
                            ForEach(indices(of: .body), id: \.self) { index in
                                row(model.contents[index], current: index == currentIndex).id(index)
                            }
                            if hasBack {
                                fold("After the book", count: backCount, open: $showBack)
                                if showBack { ForEach(indices(of: .back), id: \.self) { index in row(model.contents[index], current: index == currentIndex).id(index) } }
                            }
                        }
                    case .marks: marks
                    case .about: BookAboutView(model: model, close: close)
                    }
                }
                .padding(.horizontal, 20).padding(.top, 24).padding(.bottom, 32)
            }
            .onAppear {
                // Reading the front or back matter: its fold is open, so the place shows.
                if let current = model.contents.indices.contains(currentIndex) ? group(model.contents[currentIndex]) : nil {
                    showFront = current == .front
                    showBack = current == .back
                }
                DispatchQueue.main.async { proxy.scrollTo(currentIndex, anchor: .center) }
            }
            .onChange(of: tab) { _, now in if now == .marks { exported = model.exportMarkdown() } }
            .onChange(of: model.marks.count) { exported = tab == .marks ? model.exportMarkdown() : nil }
        }
    }

    private enum Group { case front, body, back }

    /// Which part of the book an entry is in: the part of the chapter it opens. The contents page itself and the cover are not listed.
    private func group(_ entry: BookEntry) -> Group? {
        switch model.sections.indices.contains(Int(entry.chapter)) ? model.sections[Int(entry.chapter)] : .body {
        case .cover, .contents: nil
        case .front: Int(entry.chapter) < model.start ? .front : .back
        case .body: .body
        case .back: .back
        }
    }

    private func indices(of wanted: Group) -> [Int] {
        model.contents.indices.filter { group(model.contents[$0]) == wanted }
    }

    private var frontCount: Int { indices(of: .front).count }
    private var backCount: Int { indices(of: .back).count }
    private var hasFront: Bool { frontCount > 0 }
    private var hasBack: Bool { backCount > 0 }

    /// A quiet line that opens or folds the front or back matter.
    private func fold(_ title: String, count: Int, open: Binding<Bool>) -> some View {
        AkiTap(action: { open.wrappedValue.toggle() }) {
            HStack {
                AkiText("\(title) · \(count)", .captionStrong, tone: .muted)
                Spacer(minLength: 8)
                AkiIcon(open.wrappedValue ? "down" : "forward", size: 12, tone: .muted)
            }
            .padding(.horizontal, 18).padding(.vertical, 10)
            .contentShape(Rectangle())
        }
    }

    @ViewBuilder private var marks: some View {
        if model.marks.isEmpty {
            AkiEmptyState(title: "No highlights yet", message: "Press and hold a word in the book, then drag the handles to choose what to highlight.").frame(minHeight: 240)
        } else {
            LazyVStack(spacing: 10) {
                ForEach(model.marks, id: \.id) { mark in
                    AkiTap(action: { model.open(mark); close() }) {
                        HStack(alignment: .top, spacing: 12) {
                            HighlightDot(colour: Int(mark.colour)).padding(.top, 5)
                            VStack(alignment: .leading, spacing: 6) {
                                AkiText(mark.quote, .body, lines: 4)
                                if let note = mark.note { AkiText(note, .caption, tone: .muted, lines: 4) }
                                AkiText(model.title(ofChapter: Int(mark.chapter)), .caption, tone: .muted, lines: 1)
                            }
                            Spacer(minLength: 0)
                        }
                        .padding(.horizontal, 18).padding(.vertical, 14)
                        .background(Aki.surface, in: RoundedRectangle(cornerRadius: 22, style: .continuous))
                        .contentShape(RoundedRectangle(cornerRadius: 22, style: .continuous))
                    }
                    .akiContextMenu([AkiMenuItem(title: "Remove", icon: "trash", destructive: true) { model.remove(mark) }])
                }
            }
            if let exported { AkiShare(title: "Share as Markdown", icon: "share", file: exported) }
        }
    }

    /// The last entry that begins at or before the chapter being read.
    private var currentIndex: Int {
        let chapter = model.place?.chapter ?? 0
        return model.contents.lastIndex { Int($0.chapter) <= chapter } ?? 0
    }

    private func row(_ entry: BookEntry, current: Bool) -> some View {
        AkiTap(action: { model.open(entry); close() }) {
            HStack {
                AkiText(entry.title, entry.depth == 0 ? .bodyStrong : .body, tone: current ? .onPrimary : .text, lines: 2)
                Spacer(minLength: 8)
            }
            .padding(.leading, 18 + CGFloat(min(entry.depth, 3)) * 16).padding(.trailing, 18).padding(.vertical, 14)
            .background(current ? Aki.primary : Aki.surface, in: RoundedRectangle(cornerRadius: 22, style: .continuous))
            .contentShape(RoundedRectangle(cornerRadius: 22, style: .continuous))
        }
    }
}

/// Words in the book, and where they are.
struct BookSearchSheet: View {
    let model: BookModel
    let close: () -> Void
    @State private var query = ""
    @State private var asked = ""
    @State private var hits: [BookHit] = []
    @State private var searching = false
    @State private var task: Task<Void, Never>?

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                AkiText("Search", .title)
                AkiInput(placeholder: "Words in this book", text: $query, icon: "search", onSubmit: run, onClear: { task?.cancel(); hits = []; asked = ""; searching = false })
                if searching && hits.isEmpty {
                    AkiSpinner().frame(maxWidth: .infinity, minHeight: 80)
                } else if !asked.isEmpty && hits.isEmpty {
                    AkiText("Nothing for “\(asked)”.", .body, tone: .muted)
                } else if hits.first?.similar == true {
                    AkiText("Nothing is written exactly like that. These are the nearest words.", .caption, tone: .muted)
                }
                LazyVStack(spacing: 10) {
                    ForEach(Array(hits.enumerated()), id: \.offset) { _, hit in
                        AkiTap(action: { model.open(hit); close() }) {
                            VStack(alignment: .leading, spacing: 4) {
                                AkiText(hit.snippet, .body, lines: 3)
                                AkiText(model.title(ofChapter: Int(hit.chapter)), .caption, tone: .muted, lines: 1)
                            }
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .padding(.horizontal, 18).padding(.vertical, 14)
                            .background(Aki.surface, in: RoundedRectangle(cornerRadius: 22, style: .continuous))
                            .contentShape(RoundedRectangle(cornerRadius: 22, style: .continuous))
                        }
                    }
                }
            }
            .padding(24)
        }
        .onDisappear { task?.cancel() }
    }

    private func run() {
        let words = query.trimmingCharacters(in: .whitespacesAndNewlines)
        task?.cancel()
        hits = []
        asked = words
        guard !words.isEmpty else { return }
        searching = true
        task = Task {
            let found = await model.search(words)
            guard !Task.isCancelled else { return }
            hits = found
            searching = false
        }
    }
}

/// A note on a highlight.
struct BookNoteSheet: View {
    let model: BookModel
    @State var draft: NoteDraft
    let close: () -> Void

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                AkiText("Note", .title)
                AkiText(draft.quote, .caption, tone: .muted, lines: 4)
                AkiTextArea(text: $draft.text, height: 140, prose: true)
                AkiButton("Save", variant: .primary, size: .large, fullWidth: true) { model.saveNote(draft); close() }
            }
            .padding(24)
        }
    }
}

/// A footnote, in a card over the page, with a way to go to it.
struct BookFootnoteSheet: View {
    let model: BookModel
    let note: Footnote
    let close: () -> Void

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                AkiText("Note", .title)
                AkiText(note.text, .body)
                AkiButton("Go to the note", icon: "forward", fullWidth: true) {
                    close()
                    Task { await model.follow(inBook: note.link) }
                }
            }
            .padding(24)
        }
    }
}


/// What the book says about itself and the pages around it: who wrote it, how big it is, and the copyright page, dedication,
/// acknowledgements, notes and colophon, each a card to read here or to open in the book.
struct BookAboutView: View {
    let model: BookModel
    let close: () -> Void
    /// The text of each page, read when the card is opened.
    @State private var texts: [Int: String] = [:]
    @State private var opened: Set<Int> = []

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            HStack(alignment: .top, spacing: 16) {
                AkiImage(url: model.info.cover, placeholder: 1.5, fill: true)
                    .frame(width: 84)
                    .clipShape(RoundedRectangle(cornerRadius: Aki.thumbRadius, style: .continuous))
                VStack(alignment: .leading, spacing: 6) {
                    AkiText(model.info.title, .heading, lines: 4)
                    if !model.info.authors.isEmpty { AkiText(model.info.authors.joined(separator: ", "), .caption, tone: .muted, lines: 3) }
                    AkiText(facts, .caption, tone: .muted, lines: 3)
                }
                Spacer(minLength: 0)
            }
            if model.aboutPages.isEmpty {
                AkiText("This book has no pages besides the book itself.", .caption, tone: .muted)
            }
            LazyVStack(spacing: 10) {
                // A page that is only a picture (a title page) has nothing to read here.
                ForEach(model.aboutPages.filter { texts[$0]?.isEmpty != true }, id: \.self) { chapter in card(chapter) }
            }
        }
        // Each page is read once for its title and text: they are short, and a long one (notes) is only shown in part.
        .task {
            for chapter in model.aboutPages where texts[chapter] == nil { texts[chapter] = await model.plainText(ofChapter: chapter) }
        }
    }

    private var facts: String {
        var parts: [String] = []
        if let language = model.info.language { parts.append(language.uppercased()) }
        parts.append(ByteCountFormatter.string(fromByteCount: Int64(model.info.bytes), countStyle: .file))
        parts.append("\(model.chapterCount) parts")
        return parts.joined(separator: " · ")
    }

    private func card(_ chapter: Int) -> some View {
        let text = texts[chapter]
        let open = opened.contains(chapter)
        return VStack(alignment: .leading, spacing: 10) {
            AkiTap(action: { toggle(chapter) }) {
                HStack {
                    AkiText(text.map { model.pageTitle(chapter, text: $0) } ?? model.contents.first { Int($0.chapter) == chapter }?.title ?? "Page \(chapter + 1)", .bodyStrong, lines: 2)
                    Spacer(minLength: 8)
                    AkiIcon(open ? "down" : "forward", size: 12, tone: .muted)
                }
                .contentShape(Rectangle())
            }
            if open {
                if let text {
                    // A long page (notes) shows its start: the rest is in the book.
                    AkiText(String(text.prefix(1800)) + (text.count > 1800 ? "…" : ""), .body, tone: .muted)
                    AkiButton("Open in the book", icon: "forward", size: .small) { model.open(BookEntry(title: "", depth: 0, chapter: UInt32(chapter), anchor: nil)); close() }
                } else {
                    AkiSpinner().frame(maxWidth: .infinity)
                }
            }
        }
        .padding(.horizontal, 18).padding(.vertical, 14)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Aki.surface, in: RoundedRectangle(cornerRadius: 22, style: .continuous))
    }

    private func toggle(_ chapter: Int) {
        if opened.remove(chapter) == nil { opened.insert(chapter) }
    }
}
