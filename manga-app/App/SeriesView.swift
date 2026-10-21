import AkiManga
import SwiftUI

struct ReaderTarget: Identifiable, Hashable {
    let chapterID: String
    let page: Int
    /// How far down that page, 0 to 1.
    var offset: Double = 0
    var id: String { chapterID }
}

enum SeriesTab: Hashable { case episodes, details }

/// A series: a full-width picture with its title, and a sheet that rises over it with the chapters.
struct SeriesView: View {
    let ref: SeriesRef
    @Environment(AppModel.self) private var model
    @Environment(\.dismiss) private var dismiss
    @State private var detail: Detail?
    @State private var chapters: [ChapterEntry] = []
    @State private var saved = false
    @State private var loading = true
    @State private var error: (title: String, message: String)?
    @State private var target: ReaderTarget?
    @State private var tab = SeriesTab.episodes
    @State private var expanded = false

    private let heroHeight: CGFloat = 470

    var body: some View {
        ZStack(alignment: .bottom) {
            ScrollView {
                VStack(spacing: 0) {
                    hero
                    sheet
                }
            }
            .ignoresSafeArea(edges: .top)
            .scrollIndicators(.hidden)
            .background(Aki.canvas)

            if let next = resumable {
                YellowGlassButton(title: next.label, symbol: "play.fill") { target = next.target }
                    .padding(.horizontal, 24).padding(.bottom, 12)
                    .transition(.move(edge: .bottom).combined(with: .opacity))
            }
        }
        .animation(.smooth(duration: 0.3), value: resumable?.label)
        .toolbar(.hidden, for: .navigationBar)
        .hidesTabBar()
        .task {
            await load()
            #if DEBUG
            if UserDefaults.standard.bool(forKey: "autoContinue"), target == nil { target = resumable?.target }
            #endif
        }
        .fullScreenCover(item: $target, onDismiss: { Task { await refreshProgress() } }) { target in
            ReaderView(model: ReaderModel(engine: model.engine, ref: ref, chapters: chapters, start: target))
        }
    }

    // MARK: hero

    private var hero: some View {
        GeometryReader { geo in
            let minY = geo.frame(in: .global).minY
            // Pulled down, the picture grows with the pull; pushed up it scrolls away as normal.
            let stretch = max(minY, 0)
            ZStack(alignment: .bottomLeading) {
                RemoteImage(url: detail?.cover ?? ref.cover, placeholder: 1.4, fill: true, free: true)
                    .frame(width: geo.size.width, height: heroHeight + stretch)
                LinearGradient(colors: [.black.opacity(0.35), .clear, .black.opacity(0.78)], startPoint: .top, endPoint: .bottom)
                VStack(alignment: .leading, spacing: 10) {
                    HStack(spacing: 8) {
                        if !chapters.isEmpty { Chip(text: "\(chapters.count) chapters", symbol: "list.bullet", onImage: true) }
                        if let status = statusText { Chip(text: status, symbol: "circle.fill", onImage: true) }
                        Chip(text: model.name(of: ref.source), symbol: "puzzlepiece.extension", onImage: true)
                    }
                    Text(detail?.title ?? ref.title).font(Aki.title(28)).foregroundStyle(.white)
                        .lineLimit(3).shadow(color: .black.opacity(0.4), radius: 8, y: 2)
                    if let authors = detail?.authors, !authors.isEmpty {
                        Text(authors.joined(separator: ", ")).font(.system(size: 14, weight: .medium, design: .rounded)).foregroundStyle(.white.opacity(0.85)).lineLimit(1)
                    }
                }
                .padding(.horizontal, 20).padding(.bottom, 46)
            }
            .frame(width: geo.size.width, height: heroHeight + stretch, alignment: .bottom)
            .clipped()
            .offset(y: -stretch)
            .overlay(alignment: .top) { controls.offset(y: -stretch) }
        }
        .frame(height: heroHeight)
    }

    /// Back, save and more: glass circles over the picture.
    private var controls: some View {
        GlassEffectContainer(spacing: 12) {
            HStack {
                GlassIconButton(symbol: "chevron.left", onImage: true) { dismiss() }
                Spacer()
                GlassIconButton(symbol: saved ? "bookmark.fill" : "bookmark", tint: saved ? Aki.yellow : nil, onImage: true) { Task { await toggleSaved() } }
                Menu {
                    Button("Mark all as read", systemImage: "checkmark.circle") { markAll(read: true) }
                    Button("Mark all as unread", systemImage: "circle") { markAll(read: false) }
                    Button("Refresh", systemImage: "arrow.clockwise") { Task { await load() } }
                } label: {
                    GlassIconButton(symbol: "ellipsis", onImage: true) {}.icon.glassEffect(.regular.interactive(), in: .circle)
                }
            }
        }
        .padding(.horizontal, 16).padding(.top, Aki.safeTop + 4)
    }

    // MARK: sheet

    private var sheet: some View {
        VStack(spacing: 18) {
            PillTabs(items: [(SeriesTab.episodes, "Chapters"), (SeriesTab.details, "Details")], selection: $tab)
            if let error, chapters.isEmpty {
                StateCard(title: error.title, message: error.message, action: ("Try again", { Task { await load() } })).frame(minHeight: 260)
            } else if tab == .episodes {
                chapterList
            } else {
                details
            }
        }
        .padding(.horizontal, 20).padding(.top, 22).padding(.bottom, 120)
        .frame(maxWidth: .infinity)
        .background(Aki.canvas, in: UnevenRoundedRectangle(topLeadingRadius: Aki.sheet, topTrailingRadius: Aki.sheet, style: .continuous))
        .offset(y: -30)
    }

    private var chapterList: some View {
        LazyVStack(spacing: 12) {
            if chapters.isEmpty && loading { ProgressView().padding(40) }
            ForEach(chapters, id: \.id) { chapter in
                HStack(spacing: 8) {
                    Button { target = ReaderTarget(chapterID: chapter.id, page: Int(chapter.page ?? 0)) } label: {
                        InboxRow(thumbnail: detail?.cover ?? ref.cover,
                                 label: chapter.finished ? "Read" : (chapter.page != nil ? "Page \(chapter.page! + 1)" : (chapter.title.map { !$0.localizedCaseInsensitiveContains(numberLabel(chapter)) } == true ? numberLabel(chapter) : "Unread")),
                                 title: chapterName(number: chapter.number, title: chapter.title),
                                 chips: [shortDate(chapter.dateMs), chapter.scanlator].compactMap { $0 },
                                 dimmed: chapter.finished)
                    }.buttonStyle(.plain)
                }
                .overlay(alignment: .trailing) {
                    Button { toggleRead(chapter) } label: {
                        Image(systemName: chapter.finished ? "checkmark.circle.fill" : "circle")
                            .font(.system(size: 22)).foregroundStyle(chapter.finished ? Aki.yellow : Aki.dim)
                            .frame(width: 44, height: 44).contentShape(Rectangle())
                    }.buttonStyle(.plain).padding(.trailing, 10)
                }
            }
        }
    }

    private func numberLabel(_ chapter: ChapterEntry) -> String {
        chapter.number.map { "Chapter \($0.formatted(.number.precision(.fractionLength(0...2))))" } ?? "Chapter"
    }

    private var details: some View {
        VStack(alignment: .leading, spacing: 18) {
            if let description = detail?.description {
                VStack(alignment: .leading, spacing: 8) {
                    Text(description).font(.system(size: 15, design: .rounded)).foregroundStyle(Aki.ink).lineLimit(expanded ? nil : 6)
                    Button(expanded ? "Show less" : "Read more") { withAnimation(.smooth) { expanded.toggle() } }
                        .font(.system(size: 14, weight: .bold, design: .rounded)).foregroundStyle(Aki.ink)
                }
                .padding(16).frame(maxWidth: .infinity, alignment: .leading)
                .background(Aki.card, in: RoundedRectangle(cornerRadius: Aki.cardRadius, style: .continuous))
            }
            VStack(spacing: 0) {
                info("Status", statusText ?? "Unknown")
                info("Author", detail?.authors.joined(separator: ", ") ?? "—")
                info("Source", model.name(of: ref.source))
                info("Chapters", "\(chapters.count)")
            }
            .background(Aki.card, in: RoundedRectangle(cornerRadius: Aki.cardRadius, style: .continuous))
            if let tags = detail?.tags, !tags.isEmpty {
                FlowLayout(spacing: 8) { ForEach(tags, id: \.self) { Chip(text: $0) } }
            }
        }
    }

    private func info(_ label: String, _ value: String) -> some View {
        HStack {
            Text(label).font(.system(size: 14, weight: .medium, design: .rounded)).foregroundStyle(Aki.dim)
            Spacer()
            Text(value).font(.system(size: 14, weight: .bold, design: .rounded)).foregroundStyle(Aki.ink).multilineTextAlignment(.trailing)
        }
        .padding(.horizontal, 16).padding(.vertical, 14)
    }

    private var statusText: String? {
        switch detail?.status {
        case .ongoing: "Ongoing"
        case .completed: "Completed"
        case .hiatus: "Hiatus"
        case .cancelled: "Cancelled"
        default: nil
        }
    }

    // MARK: actions

    private struct Resumable {
        let target: ReaderTarget
        let label: String
    }

    private var resumable: Resumable? {
        guard !chapters.isEmpty else { return nil }
        let resume = try? model.engine.resume(source: ref.source, series: ref.id)
        let started = chapters.contains { $0.finished || $0.page != nil }
        guard let resume, let index = chapters.firstIndex(where: { $0.id == resume.chapter }) else { return nil }
        let name = chapterName(number: chapters[index].number, title: chapters[index].title)
        return Resumable(target: ReaderTarget(chapterID: resume.chapter, page: Int(resume.page), offset: resume.offset), label: started ? "Continue · \(name)" : "Start · \(name)")
    }

    private func load() async {
        loading = true
        error = nil
        saved = (try? model.engine.inLibrary(source: ref.source, id: ref.id)) ?? false
        chapters = (try? model.engine.cachedChapters(source: ref.source, series: ref.id)) ?? []
        async let fetchedDetail = model.engine.details(source: ref.source, id: ref.id)
        async let fetchedChapters = model.engine.chapters(source: ref.source, id: ref.id)
        do { detail = try await fetchedDetail } catch { if detail == nil { self.error = describe(error) } }
        do { chapters = try await fetchedChapters.entries } catch { if chapters.isEmpty { self.error = describe(error) } }
        loading = false
    }

    private func refreshProgress() async {
        chapters = (try? model.engine.cachedChapters(source: ref.source, series: ref.id)) ?? chapters
        model.refresh()
    }

    private func toggleSaved() async {
        do {
            try await model.engine.setInLibrary(source: ref.source, id: ref.id, saved: !saved)
            saved.toggle()
            model.refresh()
        } catch { self.error = describe(error) }
    }

    private func toggleRead(_ chapter: ChapterEntry) {
        try? model.engine.mark(source: ref.source, series: ref.id, chapters: [chapter.id], read: !chapter.finished)
        chapters = (try? model.engine.cachedChapters(source: ref.source, series: ref.id)) ?? chapters
        model.refresh()
    }

    private func markAll(read: Bool) {
        try? model.engine.mark(source: ref.source, series: ref.id, chapters: chapters.map(\.id), read: read)
        chapters = (try? model.engine.cachedChapters(source: ref.source, series: ref.id)) ?? chapters
        model.refresh()
    }
}
