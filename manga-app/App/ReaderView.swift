import AkiManga
import SwiftUI

/// The long-strip reader: pages one under another with no gaps, the position kept as the person
/// scrolls, the next chapter one tap away. Chrome is glass and hides on tap.
struct ReaderView: View {
    let ref: SeriesRef
    /// Newest first, as the engine returns them: the next chapter to read is the one before.
    let chapters: [ChapterEntry]
    @State var start: ReaderTarget

    @Environment(AppModel.self) private var model
    @Environment(\.dismiss) private var dismiss
    @State private var index = 0
    @State private var pages: PageList?
    @State private var error: (title: String, message: String)?
    @State private var current: Int?
    @State private var chrome = true
    @State private var loadedFor: String?

    private var chapter: ChapterEntry? { chapters.indices.contains(index) ? chapters[index] : nil }
    private var headers: [String: String] { Dictionary(uniqueKeysWithValues: (pages?.headers ?? []).map { ($0.name, $0.value) }) }
    private var newer: ChapterEntry? { index > 0 ? chapters[index - 1] : nil }

    init(ref: SeriesRef, chapters: [ChapterEntry], start: ReaderTarget) {
        self.ref = ref
        self.chapters = chapters
        _start = State(initialValue: start)
    }

    var body: some View {
        ZStack {
            Aki.canvas.ignoresSafeArea()
            if let pages {
                strip(pages)
            } else if let error {
                StateCard(title: error.title, message: error.message, action: ("Try again", { Task { await load(chapter: start.chapterID, page: start.page) } }))
            } else {
                ProgressView()
            }
        }
        .overlay(alignment: .top) { if chrome { topBar.transition(.opacity) } }
        .overlay(alignment: .bottom) { if chrome, let pages { bottomBar(pages).transition(.opacity) } }
        .statusBarHidden(!chrome)
        .animation(.smooth(duration: 0.2), value: chrome)
        .task {
            index = chapters.firstIndex { $0.id == start.chapterID } ?? 0
            await load(chapter: start.chapterID, page: start.page)
        }
    }

    private func strip(_ pages: PageList) -> some View {
        ScrollView {
            LazyVStack(spacing: 0) {
                ForEach(Array(pages.images.enumerated()), id: \.offset) { offset, url in
                    RemoteImage(url: url, headers: headers, placeholder: 1.4).id(offset)
                }
                endCard.id(pages.images.count)
            }
            .scrollTargetLayout()
        }
        .scrollPosition(id: $current, anchor: .top)
        .ignoresSafeArea()
        .contentShape(Rectangle())
        .onTapGesture { chrome.toggle() }
        .onChange(of: current) { _, page in saveProgress(page: page, of: pages.images.count) }
    }

    private var endCard: some View {
        VStack(spacing: 12) {
            Eyebrow("End of chapter")
            if let newer {
                Button { Task { await open(newer) } } label: {
                    Text("Next · \(label(newer))").frame(maxWidth: 320)
                }.buttonStyle(.glassProminent)
            } else {
                Text("You are up to date").font(Aki.title(20)).foregroundStyle(Aki.text)
            }
            Button("Back to the series") { dismiss() }.buttonStyle(.glass)
        }
        .frame(maxWidth: .infinity).padding(.vertical, 80)
    }

    private var topBar: some View {
        GlassEffectContainer {
            HStack(spacing: 12) {
                Button { dismiss() } label: { Image(systemName: "chevron.left").padding(6) }
                    .buttonStyle(.glass).buttonBorderShape(.circle)
                VStack(alignment: .leading, spacing: 0) {
                    Text(ref.title).font(.footnote.weight(.semibold)).lineLimit(1)
                    if let chapter { Text(label(chapter)).font(.caption2).foregroundStyle(Aki.dim).lineLimit(1) }
                }
                .padding(.horizontal, 16).padding(.vertical, 8)
                .glassEffect(.regular, in: .capsule)
                Spacer(minLength: 0)
            }
        }
        .padding(.horizontal, 16).padding(.top, 8)
    }

    private func bottomBar(_ pages: PageList) -> some View {
        GlassEffectContainer(spacing: 12) {
            HStack(spacing: 12) {
                if index + 1 < chapters.count {
                    Button { Task { await open(chapters[index + 1]) } } label: { Image(systemName: "backward.end.fill").padding(6) }
                        .buttonStyle(.glass).buttonBorderShape(.circle)
                }
                let page = min((current ?? 0) + 1, pages.images.count)
                Text("\(page) / \(pages.images.count)").font(.footnote.monospacedDigit())
                    .padding(.horizontal, 18).padding(.vertical, 12).glassEffect(.regular, in: .capsule)
                if let newer {
                    Button { Task { await open(newer) } } label: { Image(systemName: "forward.end.fill").padding(6) }
                        .buttonStyle(.glass).buttonBorderShape(.circle)
                }
            }
        }
        .padding(.bottom, 24)
    }

    private func label(_ chapter: ChapterEntry) -> String {
        if let title = chapter.title { return title }
        if let number = chapter.number { return "Chapter \(number.formatted(.number.precision(.fractionLength(0...2))))" }
        return "Chapter"
    }

    private func open(_ chapter: ChapterEntry) async {
        guard let at = chapters.firstIndex(where: { $0.id == chapter.id }) else { return }
        index = at
        start = ReaderTarget(chapterID: chapter.id, page: 0)
        await load(chapter: chapter.id, page: 0)
    }

    private func load(chapter id: String, page: Int) async {
        pages = nil
        error = nil
        current = nil
        loadedFor = nil
        do {
            let list = try await model.engine.pages(source: ref.source, series: ref.id, chapter: id)
            pages = list
            loadedFor = id
            // Let the strip lay out, then go to where the person left off.
            try? await Task.sleep(for: .milliseconds(150))
            current = min(page, max(list.images.count - 1, 0))
        } catch {
            self.error = describe(error)
        }
    }

    private func saveProgress(page: Int?, of count: Int) {
        guard let page, let id = loadedFor, count > 0 else { return }
        // Finished is the end card reaching the top, not the last page: the last page is still to be read.
        let finished = page >= count
        try? model.engine.setProgress(source: ref.source, series: ref.id, chapter: id, page: UInt32(min(page, count - 1)), offset: 0, finished: finished)
    }
}
