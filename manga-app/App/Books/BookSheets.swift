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

    private enum Tab: Hashable { case contents, marks }

    var body: some View {
        ScrollViewReader { proxy in
            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    VStack(alignment: .leading, spacing: 4) {
                        AkiText(tab == .contents ? "Contents" : "Highlights", .title)
                        AkiText(model.info.title, .caption, tone: .muted, lines: 1)
                    }
                    AkiTabs(items: [(Tab.contents, "Contents"), (Tab.marks, "Highlights \(model.marks.count)")], selection: $tab)
                    switch tab {
                    case .contents:
                        LazyVStack(spacing: 10) {
                            ForEach(Array(model.contents.enumerated()), id: \.offset) { index, entry in
                                row(entry, current: index == currentIndex).id(index)
                            }
                        }
                    case .marks: marks
                    }
                }
                .padding(.horizontal, 20).padding(.top, 24).padding(.bottom, 32)
            }
            .onAppear { DispatchQueue.main.async { proxy.scrollTo(currentIndex, anchor: .center) } }
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
                }
                LazyVStack(spacing: 10) {
                    ForEach(Array(hits.enumerated()), id: \.offset) { _, hit in
                        AkiTap(action: { model.open(hit, length: asked.utf16.count); close() }) {
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
                if searching && !hits.isEmpty { AkiSpinner().frame(maxWidth: .infinity) }
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
            await model.search(words) { found in hits += found }
            if !Task.isCancelled { searching = false }
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
