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
    /// The chapter whose pages were fetched ahead, so it is done once.
    @State private var fetchedAhead: String?
    @State private var saved = false
    @State private var loading = true
    @State private var error: (title: String, message: String)?
    @State private var target: ReaderTarget?
    /// A locked chapter waiting for the person to sign in: opened as soon as they have.
    @State private var waiting: ReaderTarget?
    @State private var signingIn = false
    @State private var afterSignIn: ReaderTarget?
    @State private var tab = SeriesTab.chapters
    @State private var expanded = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var kept: Set<String> = []
    @State private var bookmarks: [Bookmark] = []
    /// Counters that only change on something the person did, so a haptic never fires just from opening the page.
    @State private var savedTaps = 0
    @State private var downloadsDone = 0
    private enum Sheet { case move, categories, cover, forget }
    @State private var presented: Sheet?
    /// The cover the person chose for this series, as an address, or nil.
    @State private var chosenCover: String?
    @State private var problem: String?

    private let heroHeight: CGFloat = 470

    /// The cover the person chose, else the source's.
    private var cover: String? { chosenCover ?? detail?.cover ?? ref.cover }

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
        .akiAlert(Copy.Series.couldNotDownloadChapters, message: problem, isPresented: Binding(get: { problem != nil }, set: { if !$0 { problem = nil } }))
        .akiSheet(isPresented: $presented.equals(.move)) { MigrateSheet(ref: ref) { dismiss() } }
        .akiSheet(isPresented: $signingIn) { SignInSheet(source: ref.source) { afterSignIn = waiting } }
        .onChange(of: signingIn) { if !signingIn, let next = afterSignIn { afterSignIn = nil; target = next } }
        .akiPhotoPicker(isPresented: $presented.equals(.cover)) { jpeg in
            try? model.engine.setCustomCover(source: ref.source, series: ref.id, jpeg: jpeg)
            chosenCover = (try? model.engine.customCover(source: ref.source, series: ref.id)) ?? nil
            model.refresh()
        }
        .akiSheet(isPresented: $presented.equals(.categories)) { CategoryPicker(series: [CategoryPicker.Series(source: ref.source, id: ref.id)]) }
        .akiConfirm(Copy.Series.deleteDataQuestion(ref.title), message: Copy.Series.deleteDataHint, confirm: Copy.Series.deleteData, isPresented: $presented.equals(.forget)) {
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
                AkiImage(url: cover, placeholder: 1.4, fill: true, free: true)
                    .frame(width: geo.size.width, height: heroHeight + stretch)
                LinearGradient(colors: [Aki.scrimTop, .clear, Aki.scrimBottom], startPoint: .top, endPoint: .bottom)
                VStack(alignment: .leading, spacing: 10) {
                    HStack(spacing: 8) {
                        if let count = chaptersLabel(UInt32(chapters.count)) { AkiBadge(count, icon: "list", style: .glass) }
                        if let status = statusText { AkiBadge(status, icon: "clock", style: .glass) }
                        AkiBadge(model.name(of: ref.source), icon: "sources", style: .glass)
                    }
                    AkiText(detail?.title ?? ref.title, .display, tone: .onMedia, lines: 3).accessibilityAddTraits(.isHeader)
                    if let authors = detail?.authors, !authors.isEmpty {
                        AkiText(authors.joined(separator: ", "), .caption, tone: .onMedia, lines: 2)
                    }
                }
                .padding(.horizontal, 20).padding(.bottom, 46)
                .frame(maxWidth: 720, alignment: .leading)
                .frame(maxWidth: .infinity)
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
                AkiIconButton(symbol: "back", label: Copy.Series.back, onMedia: true) { dismiss() }
                Spacer()
                AkiIconButton(symbol: saved ? "bookmark-solid" : "bookmark", label: saved ? Copy.Library.removeFromLibrary : Copy.Series.addToLibrary, tint: saved ? Aki.primary : nil, onMedia: true) { Task { await toggleSaved() } }
                AkiMenu(symbol: "more", label: Copy.Series.moreOptions, onMedia: true, items: [
                    AkiMenuItem(title: Copy.Home.markAllAsRead, icon: "check") { markAll(read: true) },
                    AkiMenuItem(title: Copy.Series.markAllAsUnread, icon: "check") { markAll(read: false) },
                    AkiMenuItem(title: Copy.Series.downloadUnreadChapters, icon: "download") { download(chapters.filter { !$0.finished && !kept.contains($0.id) }) },
                    AkiMenuItem(title: Copy.Series.refresh, icon: "refresh") { Task { await load(force: true) } },
                    AkiMenuItem(title: Copy.Series.share, icon: "share") { model.sharing = SharePresented(item: .series(ref)) },
                    AkiMenuItem(title: Copy.Library.changeCategories, icon: "folder") { presented = .categories },
                    AkiMenuItem(title: Copy.Series.chooseCover, icon: "image") { presented = .cover },
                    AkiMenuItem(title: Copy.Series.resetCover, icon: "refresh") { clearCover() },
                    AkiMenuItem(title: Copy.Library.moveToAnotherSource, icon: "swap") { presented = .move },
                    AkiMenuItem(title: Copy.Series.deleteSeriesData, icon: "trash", destructive: true) { presented = .forget },
                ])
            }
        }
        .padding(.horizontal, 16).padding(.top, 4)
    }

    // MARK: sheet

    private var sheet: some View {
        VStack(spacing: 18) {
            AkiTabs(items: [(SeriesTab.chapters, Copy.Series.chapters), (SeriesTab.bookmarks, Copy.Series.bookmarks), (SeriesTab.details, Copy.Series.details)], selection: $tab)
            if tab == .bookmarks {
                bookmarkList
            } else if let error, chapters.isEmpty {
                AkiEmptyState(title: error.title, message: error.message, action: (Copy.Home.tryAgain, { Task { await load(force: true) } })).frame(minHeight: 260)
            } else if tab == .chapters && chapters.isEmpty && !loading {
                AkiEmptyState(title: Copy.Series.nothingToReadHere, message: Copy.Series.noChaptersInLanguage(model.name(of: ref.source)))
                    .frame(minHeight: 280)
            } else if tab == .chapters {
                chapterList
            } else {
                details
            }
        }
        .padding(.horizontal, 20).padding(.top, 22).padding(.bottom, 120)
        .frame(maxWidth: 720)
        .frame(maxWidth: .infinity)
        .background(Aki.background, in: UnevenRoundedRectangle(topLeadingRadius: Aki.sheetRadius, topTrailingRadius: Aki.sheetRadius, style: .continuous))
        .offset(y: -30)
    }

    private var chapterList: some View {
        LazyVStack(spacing: 12) {
            if chapters.isEmpty && loading { AkiSpinner().padding(40) }
            ForEach(chapters, id: \.id) { chapter in
                AkiTap(action: { open(chapter) }) {
                    AkiListCard(thumbnail: cover, label: label(chapter),
                                title: chapterName(number: chapter.number, title: chapter.title),
                                badges: [shortDate(chapter.dateMs), chapter.scanlator].compactMap { $0 }, dimmed: chapter.finished) {
                        if isLocked(chapter) {
                            // The mark is the state, so it sits where the read mark would; the whole row opens the sign-in.
                            AkiIcon("lock-solid", size: 20, tone: .muted)
                                .frame(width: 44, height: 44)
                                .accessibilityLabel(source?.lockedPromptsSignIn == true ? Copy.Series.lockedSignIn : Copy.Series.locked)
                        } else {
                            AkiTap(action: { toggleRead(chapter) }) {
                                AkiIcon(chapter.finished ? "check-solid" : "check", size: 22, tone: chapter.finished ? .text : .muted)
                                    .frame(width: 44, height: 44)
                            }
                            .accessibilityLabel(chapter.finished ? Copy.Series.markAsUnread(chapterName(number: chapter.number, title: chapter.title)) : Copy.Series.markAsRead(chapterName(number: chapter.number, title: chapter.title)))
                        }
                    }
                }
                .akiContextMenu([
                    kept.contains(chapter.id)
                        ? AkiMenuItem(title: Copy.Series.deleteDownload, icon: "trash", destructive: true) { removeDownload(chapter) }
                        : AkiMenuItem(title: Copy.Series.download, icon: "download") { download([chapter]) },
                ])
            }
        }
    }

    private var bookmarkList: some View {
        LazyVStack(spacing: 12) {
            if bookmarks.isEmpty {
                AkiEmptyState(title: Copy.Series.noBookmarksYet, message: Copy.Series.bookmarkHint).frame(minHeight: 260)
            }
            ForEach(bookmarks, id: \.self) { mark in
                AkiTap(action: { target = ReaderTarget(chapterID: mark.chapter, page: Int(mark.page), offset: mark.offset) }) {
                    AkiListCard(thumbnail: cover, label: Copy.Series.pageNumber(Int(mark.page) + 1),
                                title: chapterName(number: mark.chapterNumber, title: mark.chapterTitle),
                                badges: [shortDate(mark.createdMs)].compactMap { $0 }) {
                        AkiTap(action: { removeBookmark(mark) }) {
                            AkiIcon("trash", size: 22, tone: .muted).frame(width: 44, height: 44)
                        }
                        .accessibilityLabel(Copy.Series.removeBookmark(Int(mark.page) + 1))
                    }
                }
            }
        }
    }

    private var source: SourceInfo? { model.sources.first { $0.id == ref.source } }

    /// The source flags it and the person is not signed in. A signed-in person sees it as any chapter: the site decides what an account may read.
    private func isLocked(_ chapter: ChapterEntry) -> Bool { chapter.locked && source?.signedIn != true }

    /// A locked chapter asks for the account first, when the source says so and has one to sign in to; any other opens at its place.
    private func open(_ chapter: ChapterEntry) {
        let next = ReaderTarget(chapterID: chapter.id, page: Int(chapter.page ?? 0))
        if isLocked(chapter) && source?.lockedPromptsSignIn == true {
            waiting = next
            signingIn = true
        } else {
            target = next
        }
    }

    /// The small line over a chapter's title: where you are in it, or what it is.
    private func label(_ chapter: ChapterEntry) -> String {
        if chapter.finished { return kept.contains(chapter.id) ? Copy.Series.readDownloaded : Copy.Series.read }
        if kept.contains(chapter.id) { return Copy.Series.downloaded }
        if let page = chapter.page { return Copy.Series.pageNumber(Int(page) + 1) }
        if isLocked(chapter), let word = source?.lockedLabel { return word }
        let number = chapter.number.map { Copy.Home.chapterNumber($0.formatted(.number.precision(.fractionLength(0...2)))) }
        if let number, chapter.title.map({ !$0.localizedCaseInsensitiveContains(number) }) == true { return number }
        return Copy.Home.unread
    }

    private var details: some View {
        VStack(alignment: .leading, spacing: 18) {
            if let description = detail?.description {
                AkiCard(padding: 16) {
                    VStack(alignment: .leading, spacing: 8) {
                        AkiText(description, .body, lines: expanded ? nil : 6)
                        AkiButton(expanded ? Copy.Series.showLess : Copy.Home.showMore, variant: .ghost, size: .small) { withAnimation(AkiMotion.settle(reduceMotion)) { expanded.toggle() } }
                    }
                }
            }
            AkiCard(padding: 4) {
                VStack(spacing: 0) {
                    info(Copy.Series.status, statusText ?? Copy.Series.unknown)
                    info(Copy.Series.author, detail?.authors.isEmpty == false ? detail!.authors.joined(separator: ", ") : Copy.Series.unknown)
                    info(Copy.Series.source, model.name(of: ref.source))
                    info(Copy.Series.chapters, chapters.count.formatted())
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
        .accessibilityElement(children: .combine)
    }

    private var statusText: String? {
        switch detail?.status {
        case .ongoing: Copy.Series.ongoing
        case .completed: Copy.Series.completed
        case .hiatus: Copy.Series.hiatus
        case .cancelled: Copy.Series.cancelled
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
        resumable = Resumable(target: ReaderTarget(chapterID: resume.chapter, page: Int(resume.page), offset: resume.offset), label: started ? Copy.Series.continueSeries(name) : Copy.Series.startSeries(name))
        // Only for a series the person keeps or has begun: browsing past one should not cost the site a request.
        if started || saved { fetchPagesAhead(of: resume.chapter) }
    }

    /// Shows what is stored at once, then asks the site. The chapter list is not asked again within ten minutes of the
    /// last time (going back and forth between a series and the reader, say) unless `force` is set.
    private func load(force: Bool = false) async {
        error = nil
        saved = (try? model.engine.inLibrary(source: ref.source, id: ref.id)) ?? false
        setChapters((try? model.engine.cachedChapters(source: ref.source, series: ref.id)) ?? [])
        if detail == nil { detail = (try? model.engine.cachedDetails(source: ref.source, id: ref.id)) ?? nil }
        let checked = (try? model.engine.chaptersCheckedMs(source: ref.source, series: ref.id)).flatMap { $0 }
        let fresh = !force && !chapters.isEmpty && checked.map { Date().timeIntervalSince1970 * 1000 - Double($0) < 600_000 } == true
        loading = !fresh
        loadKept()
        loadBookmarks()
        chosenCover = (try? model.engine.customCover(source: ref.source, series: ref.id)) ?? nil
        if fresh {
            // Seen in the last ten minutes: the stored details are enough, unless there are none.
            if detail == nil {
                do { detail = try await model.engine.details(source: ref.source, id: ref.id) } catch { self.error = describe(error, Copy.Series.couldNotLoadSeries) }
            }
        } else {
            async let fetchedDetail = model.engine.details(source: ref.source, id: ref.id)
            async let fetchedChapters = model.engine.chapters(source: ref.source, id: ref.id)
            do { detail = try await fetchedDetail } catch { if detail == nil { self.error = describe(error, Copy.Series.couldNotLoadSeries) } }
            do { setChapters(try await fetchedChapters.entries) } catch { if chapters.isEmpty { self.error = describe(error, Copy.Series.couldNotLoadChapters) } }
        }
        loading = false
        loadBookmarks()
    }

    /// Asks for the pages of the chapter Continue opens while the series page is read, so the reader finds them ready.
    private func fetchPagesAhead(of chapter: String) {
        guard fetchedAhead != chapter else { return }
        fetchedAhead = chapter
        let (source, series, engine) = (ref.source, ref.id, model.engine)
        Task { _ = try? await engine.pages(source: source, series: series, chapter: chapter) }
    }

    private func loadKept() {
        kept = Set(((try? model.engine.downloads()) ?? []).filter { $0.source == ref.source && $0.series == ref.id && $0.state == "done" }.map(\.chapter))
    }

    private func download(_ list: [ChapterEntry]) {
        akiAnnounce(Copy.Series.downloading)
        Task {
            var failed = 0
            for chapter in list.reversed() {
                do { try await model.engine.downloadChapter(source: ref.source, series: ref.id, chapter: chapter.id) } catch { failed += 1 }
                loadKept()
            }
            if failed == 0 { downloadsDone += 1; akiAnnounce(Copy.Series.downloaded) }
            if failed > 0 { problem = Copy.Series.chaptersNotDownloaded(failed) }
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
            akiAnnounce(saved ? Copy.Series.addedToLibrary : Copy.Series.removedFromLibrary)
            if saved { putOnSaveCategory() }
            model.refresh()
        } catch { self.error = describe(error, Copy.Series.couldNotUpdateLibrary) }
    }

    private func clearCover() {
        try? model.engine.clearCustomCover(source: ref.source, series: ref.id)
        chosenCover = nil
        model.refresh()
    }

    /// What Settings says saving does about categories: nothing, ask which, or put the series on the chosen one.
    private func putOnSaveCategory() {
        switch model.prefs.saveCategory {
        case 0: break
        case -1: presented = .categories
        // A category that has since been deleted (or came back from a backup under another number) is not one to put it on.
        case let id where ((try? model.engine.categories()) ?? []).contains { $0.id == id }: try? model.engine.setCategory(source: ref.source, series: ref.id, category: id, member: true)
        default: break
        }
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
