import AkiManga
import SwiftUI

struct ReaderTarget: Identifiable, Hashable {
    let chapterID: String
    let page: Int
    /// How far down that page, 0 to 1.
    var offset: Double = 0
    var id: String { chapterID }
}

enum SeriesTab: Hashable { case chapters, bookmarks, details }

/// A series: a full-width picture with its title, and a sheet that rises over it with the chapters.
struct SeriesView: View {
    let ref: SeriesRef
    @Environment(AppModel.self) private var model
    @Environment(\.dismiss) private var dismiss
    @State private var detail: Detail?
    @State private var chapters: [ChapterEntry] = []
    /// Where Continue goes, worked out when the chapters or progress change, not on every redraw.
    @State private var resumable: Resumable?
    @State private var saved = false
    @State private var loading = true
    @State private var error: (title: String, message: String)?
    @State private var target: ReaderTarget?
    @State private var tab = SeriesTab.chapters
    @State private var expanded = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var kept: Set<String> = []
    @State private var bookmarks: [Bookmark] = []
    @State private var forgetting = false
    /// Counters that only change on something the person did, so a haptic never fires just from opening the page.
    @State private var savedTaps = 0
    @State private var downloadsDone = 0
    @State private var moving = false
    @State private var problem: String?

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
            .akiRefreshable(inset: Aki.safeTop) { await load(force: true) }
            .background(Aki.background)

            // Pinned over the scrolling page, so back is always there and always takes the tap.
            VStack { controls; Spacer() }

            if let next = resumable {
                AkiButton(next.label, icon: "play-solid", variant: .primary, size: .large) { target = next.target }
                    .padding(.horizontal, 24).padding(.bottom, 12)
                    .transition(.move(edge: .bottom).combined(with: .opacity))
            }
        }
        .akiAnimation(value: resumable?.label)
        .akiNoNavigationBar()
        .hidesTabBar()
        .task {
            await load()
            #if DEBUG
            if UserDefaults.standard.bool(forKey: "autoContinue"), target == nil { target = resumable?.target }
            #endif
        }
        .akiHaptic(.impact(weight: .medium), trigger: savedTaps)
        .akiHaptic(.success, trigger: downloadsDone)
        .akiHaptic(.error, trigger: problem)
        .akiAlert("Download failed", message: problem, isPresented: Binding(get: { problem != nil }, set: { if !$0 { problem = nil } }))
        .akiSheet(isPresented: $moving) { MigrateSheet(ref: ref) { dismiss() } }
        .akiConfirm("Forget \(ref.title)?", message: "It leaves your library and its reading progress and downloads are deleted.", confirm: "Forget", isPresented: $forgetting) {
            try? model.engine.forgetSeries(source: ref.source, series: ref.id)
            model.refresh()
            dismiss()
        }
        .akiFullScreen(item: $target, onDismiss: { Task { await refreshProgress() } }) { target in
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
                AkiImage(url: detail?.cover ?? ref.cover, placeholder: 1.4, fill: true, free: true)
                    .frame(width: geo.size.width, height: heroHeight + stretch)
                LinearGradient(colors: [Aki.scrimTop, .clear, Aki.scrimBottom], startPoint: .top, endPoint: .bottom)
                VStack(alignment: .leading, spacing: 10) {
                    HStack(spacing: 8) {
                        if !chapters.isEmpty { AkiBadge("\(chapters.count) chapters", icon: "list", style: .glass) }
                        if let status = statusText { AkiBadge(status, icon: "clock", style: .glass) }
                        AkiBadge(model.name(of: ref.source), icon: "sources", style: .glass)
                    }
                    AkiText(detail?.title ?? ref.title, .display, tone: .onMedia, lines: 3)
                    if let authors = detail?.authors, !authors.isEmpty {
                        AkiText(authors.joined(separator: ", "), .caption, tone: .onMedia, lines: 1)
                    }
                }
                .padding(.horizontal, 20).padding(.bottom, 46)
            }
            .frame(width: geo.size.width, height: heroHeight + stretch, alignment: .bottom)
            .clipped()
            .offset(y: -stretch)
        }
        .frame(height: heroHeight)
    }

    /// Back, save and more: glass circles over the picture.
    private var controls: some View {
        AkiGlassGroup(spacing: 12) {
            HStack {
                AkiIconButton(symbol: "back", label: "Back", onMedia: true) { dismiss() }
                Spacer()
                AkiIconButton(symbol: saved ? "bookmark-solid" : "bookmark", label: saved ? "Remove from library" : "Save to library", tint: saved ? Aki.primary : nil, onMedia: true) { Task { await toggleSaved() } }
                AkiMenu(symbol: "more", label: "More", onMedia: true, items: [
                    AkiMenuItem(title: "Mark all as read", icon: "check") { markAll(read: true) },
                    AkiMenuItem(title: "Mark all as unread", icon: "check") { markAll(read: false) },
                    AkiMenuItem(title: "Download unread", icon: "download") { download(chapters.filter { !$0.finished && !kept.contains($0.id) }) },
                    AkiMenuItem(title: "Refresh", icon: "refresh") { Task { await load(force: true) } },
                    AkiMenuItem(title: "Move to another source", icon: "swap") { moving = true },
                    AkiMenuItem(title: "Forget this series", icon: "trash", destructive: true) { forgetting = true },
                ])
            }
        }
        .padding(.horizontal, 16).padding(.top, 4)
    }

    // MARK: sheet

    private var sheet: some View {
        VStack(spacing: 18) {
            AkiTabs(items: [(SeriesTab.chapters, "Chapters"), (SeriesTab.bookmarks, "Bookmarks"), (SeriesTab.details, "Details")], selection: $tab)
            if tab == .bookmarks {
                bookmarkList
            } else if let error, chapters.isEmpty {
                AkiEmptyState(title: error.title, message: error.message, action: ("Try again", { Task { await load(force: true) } })).frame(minHeight: 260)
            } else if tab == .chapters && chapters.isEmpty && !loading {
                AkiEmptyState(title: "Nothing to read here", message: "\(model.name(of: ref.source)) lists this series but has no chapters it can show in your language. Licensed series are often only linked to the publisher's own site. Try another source, or another language in the source's settings.")
                    .frame(minHeight: 280)
            } else if tab == .chapters {
                chapterList
            } else {
                details
            }
        }
        .padding(.horizontal, 20).padding(.top, 22).padding(.bottom, 120)
        .frame(maxWidth: .infinity)
        .background(Aki.background, in: UnevenRoundedRectangle(topLeadingRadius: Aki.sheetRadius, topTrailingRadius: Aki.sheetRadius, style: .continuous))
        .offset(y: -30)
    }

    private var chapterList: some View {
        LazyVStack(spacing: 12) {
            if chapters.isEmpty && loading { AkiSpinner().padding(40) }
            ForEach(chapters, id: \.id) { chapter in
                AkiTap(action: { target = ReaderTarget(chapterID: chapter.id, page: Int(chapter.page ?? 0)) }) {
                    AkiListCard(thumbnail: detail?.cover ?? ref.cover, label: label(chapter),
                                title: chapterName(number: chapter.number, title: chapter.title),
                                badges: [shortDate(chapter.dateMs), chapter.scanlator].compactMap { $0 }, dimmed: chapter.finished) {
                        AkiTap(action: { toggleRead(chapter) }) {
                            AkiIcon(chapter.finished ? "check-solid" : "check", size: 22, tone: chapter.finished ? .text : .muted)
                                .frame(width: 44, height: 44)
                        }
                        .accessibilityLabel(chapter.finished ? "Mark as unread" : "Mark as read")
                    }
                }
                .akiContextMenu([
                    kept.contains(chapter.id)
                        ? AkiMenuItem(title: "Delete download", icon: "trash", destructive: true) { removeDownload(chapter) }
                        : AkiMenuItem(title: "Download", icon: "download") { download([chapter]) },
                ])
            }
        }
    }

    private var bookmarkList: some View {
        LazyVStack(spacing: 12) {
            if bookmarks.isEmpty {
                AkiEmptyState(title: "No bookmarks yet", message: "Tap the bookmark button while you read to mark a page, and it will be here.").frame(minHeight: 260)
            }
            ForEach(bookmarks, id: \.self) { mark in
                AkiTap(action: { target = ReaderTarget(chapterID: mark.chapter, page: Int(mark.page), offset: mark.offset) }) {
                    AkiListCard(thumbnail: detail?.cover ?? ref.cover, label: "Page \(mark.page + 1)",
                                title: chapterName(number: mark.chapterNumber, title: mark.chapterTitle),
                                badges: [shortDate(mark.createdMs)].compactMap { $0 }) {
                        AkiTap(action: { removeBookmark(mark) }) {
                            AkiIcon("trash", size: 22, tone: .muted).frame(width: 44, height: 44)
                        }
                        .accessibilityLabel("Remove bookmark")
                    }
                }
            }
        }
    }

    /// The small line over a chapter's title: where you are in it, or what it is.
    private func label(_ chapter: ChapterEntry) -> String {
        if chapter.finished { return kept.contains(chapter.id) ? "Read · Downloaded" : "Read" }
        if kept.contains(chapter.id) { return "Downloaded" }
        if let page = chapter.page { return "Page \(page + 1)" }
        let number = chapter.number.map { "Chapter \($0.formatted(.number.precision(.fractionLength(0...2))))" }
        if let number, chapter.title.map({ !$0.localizedCaseInsensitiveContains(number) }) == true { return number }
        return "Unread"
    }

    private var details: some View {
        VStack(alignment: .leading, spacing: 18) {
            if let description = detail?.description {
                AkiCard(padding: 16) {
                    VStack(alignment: .leading, spacing: 8) {
                        AkiText(description, .body, lines: expanded ? nil : 6)
                        AkiButton(expanded ? "Show less" : "Read more", variant: .ghost, size: .small) { withAnimation(AkiMotion.settle(reduceMotion)) { expanded.toggle() } }
                    }
                }
            }
            AkiCard(padding: 4) {
                VStack(spacing: 0) {
                    info("Status", statusText ?? "Unknown")
                    info("Author", detail?.authors.isEmpty == false ? detail!.authors.joined(separator: ", ") : "—")
                    info("Source", model.name(of: ref.source))
                    info("Chapters", "\(chapters.count)")
                }
            }
            if let tags = detail?.tags, !tags.isEmpty {
                AkiFlow(spacing: 8) { ForEach(tags, id: \.self) { AkiBadge($0) } }
            }
        }
    }

    private func info(_ name: String, _ value: String) -> some View {
        HStack {
            AkiText(name, .caption, tone: .muted)
            Spacer()
            AkiText(value, .bodyStrong, align: .trailing)
        }
        .padding(.horizontal, 12).padding(.vertical, 12)
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

    private func setChapters(_ list: [ChapterEntry]) {
        chapters = list
        guard !list.isEmpty, let resume = try? model.engine.resume(source: ref.source, series: ref.id),
              let chapter = list.first(where: { $0.id == resume.chapter }) else { return resumable = nil }
        let started = list.contains { $0.finished || $0.page != nil }
        let name = chapterName(number: chapter.number, title: chapter.title)
        resumable = Resumable(target: ReaderTarget(chapterID: resume.chapter, page: Int(resume.page), offset: resume.offset), label: started ? "Continue · \(name)" : "Start · \(name)")
    }

    /// Shows what is stored at once, then asks the site. The chapter list is not asked again within ten minutes of the
    /// last time (going back and forth between a series and the reader, say) unless `force` is set.
    private func load(force: Bool = false) async {
        error = nil
        saved = (try? model.engine.inLibrary(source: ref.source, id: ref.id)) ?? false
        setChapters((try? model.engine.cachedChapters(source: ref.source, series: ref.id)) ?? [])
        let checked = (try? model.engine.chaptersCheckedMs(source: ref.source, series: ref.id)).flatMap { $0 }
        let fresh = !force && !chapters.isEmpty && checked.map { Date().timeIntervalSince1970 * 1000 - Double($0) < 600_000 } == true
        loading = !fresh
        loadKept()
        loadBookmarks()
        async let fetchedDetail = model.engine.details(source: ref.source, id: ref.id)
        if fresh {
            do { detail = try await fetchedDetail } catch { if detail == nil { self.error = describe(error) } }
        } else {
            async let fetchedChapters = model.engine.chapters(source: ref.source, id: ref.id)
            do { detail = try await fetchedDetail } catch { if detail == nil { self.error = describe(error) } }
            do { setChapters(try await fetchedChapters.entries) } catch { if chapters.isEmpty { self.error = describe(error) } }
        }
        loading = false
        loadBookmarks()
    }

    private func loadKept() {
        kept = Set(((try? model.engine.downloads()) ?? []).filter { $0.source == ref.source && $0.series == ref.id && $0.state == "done" }.map(\.chapter))
    }

    private func download(_ list: [ChapterEntry]) {
        Task {
            var failed = 0
            for chapter in list.reversed() {
                do { try await model.engine.downloadChapter(source: ref.source, series: ref.id, chapter: chapter.id) } catch { failed += 1 }
                loadKept()
            }
            if failed == 0 { downloadsDone += 1 }
            if failed > 0 { problem = failed == 1 ? "One chapter could not be downloaded." : "\(failed) chapters could not be downloaded." }
        }
    }

    private func removeDownload(_ chapter: ChapterEntry) {
        try? model.engine.deleteDownload(source: ref.source, series: ref.id, chapter: chapter.id)
        loadKept()
    }

    private func loadBookmarks() {
        bookmarks = (try? model.engine.bookmarks(source: ref.source, series: ref.id)) ?? []
    }

    private func removeBookmark(_ mark: Bookmark) {
        try? model.engine.removeBookmark(source: ref.source, series: ref.id, chapter: mark.chapter, page: mark.page)
        loadBookmarks()
    }

    private func refreshProgress() async {
        loadBookmarks()
        setChapters((try? model.engine.cachedChapters(source: ref.source, series: ref.id)) ?? chapters)
        model.refresh()
    }

    private func toggleSaved() async {
        do {
            try await model.engine.setInLibrary(source: ref.source, id: ref.id, saved: !saved)
            saved.toggle()
            savedTaps += 1
            model.refresh()
        } catch { self.error = describe(error) }
    }

    private func toggleRead(_ chapter: ChapterEntry) {
        try? model.engine.mark(source: ref.source, series: ref.id, chapters: [chapter.id], read: !chapter.finished)
        setChapters((try? model.engine.cachedChapters(source: ref.source, series: ref.id)) ?? chapters)
        model.refresh()
    }

    private func markAll(read: Bool) {
        try? model.engine.mark(source: ref.source, series: ref.id, chapters: chapters.map(\.id), read: read)
        setChapters((try? model.engine.cachedChapters(source: ref.source, series: ref.id)) ?? chapters)
        model.refresh()
    }
}
