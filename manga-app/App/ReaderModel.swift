import AkiManga
import Observation
import SwiftUI

enum ReadingMode: String, CaseIterable, Identifiable {
    case strip, paged, rtl
    var id: String { rawValue }
    /// The same, short enough for a row of three pills.
    var shortTitle: String {
        switch self {
        case .strip: "Strip"
        case .paged: "Paged"
        case .rtl: "Right to left"
        }
    }
    var title: String {
        switch self {
        case .strip: "Long strip"
        case .paged: "Pages, left to right"
        case .rtl: "Pages, right to left"
        }
    }
}

struct Segment: Identifiable {
    let chapterID: String
    let pages: PageList
    var id: String { chapterID }
    var headers: [String: String] { Dictionary(pages.headers.map { ($0.name, $0.value) }, uniquingKeysWith: { $1 }) }
}

/// Where to put the reader when its pages are on screen.
struct Restore {
    let chapter: String
    let page: Int
    /// How far down that page, 0 to 1 of its height.
    let offset: Double
}

/// Everything the reader knows: the chapters laid out, which one the person is in and how far, the mode
/// and zoom, and the work done ahead (the next chapter's pages and first images).
@MainActor @Observable
final class ReaderModel {
    let engine: Engine
    let ref: SeriesRef
    /// Newest first, as the engine returns them: the next chapter to read is the one before.
    let chapters: [ChapterEntry]

    var segments: [Segment] = []
    var mode: ReadingMode
    var zoom: CGFloat = 1
    var current: String
    var page = 0
    var pagedIndex = 0
    var chrome = true
    /// The strip is scrolling by itself.
    var autoScrolling = false
    /// The pages marked in this series.
    var bookmarks: [Bookmark] = []
    var loading = false
    var error: (title: String, message: String)?
    var restore: Restore?
    var restoreToken = 0
    /// Set while the strip is being put back where it was: the scrolling that does it is not saved, and the strip is not shown.
    var restoring = false
    @ObservationIgnored private(set) var aspects: [String: CGFloat] = [:]
    @ObservationIgnored private var heightRatios: CGFloat = 0
    /// The height, as a multiple of the width, a page is likely to have: the average of those already seen, so a page
    /// that has not arrived holds about the room it will take and the strip does not jump when it does.
    var typicalHeightRatio: CGFloat {
        guard !aspects.isEmpty else { return 1.4 }
        return min(max(heightRatios / CGFloat(aspects.count), 0.8), 8)
    }

    /// Notes a page's width-to-height ratio once its picture is known.
    func noteAspect(_ aspect: CGFloat, of url: String) {
        guard aspect > 0, aspect.isFinite else { return }
        if let old = aspects[url] { heightRatios -= 1 / old }
        aspects[url] = aspect
        heightRatios += 1 / aspect
    }
    @ObservationIgnored private var cache: [String: PageList] = [:]
    @ObservationIgnored private var latest: (chapter: String, page: Int, offset: Double)?
    @ObservationIgnored private var finished: Set<String> = []
    @ObservationIgnored private var saveTask: Task<Void, Never>?
    @ObservationIgnored private var appending = false
    @ObservationIgnored private var warmed: Set<String> = []

    init(engine: Engine, ref: SeriesRef, chapters: [ChapterEntry], start: ReaderTarget) {
        self.engine = engine
        self.ref = ref
        self.chapters = chapters
        self.current = start.chapterID
        self.page = start.page
        self.mode = UserDefaults.standard.string(forKey: "mode.\(ref.source).\(ref.id)").flatMap(ReadingMode.init) ?? .strip
        self.restore = Restore(chapter: start.chapterID, page: start.page, offset: start.offset)
        loadBookmarks()
    }

    // MARK: chapters

    func chapter(_ id: String) -> ChapterEntry? { chapters.first { $0.id == id } }

    func newer(than id: String) -> ChapterEntry? {
        guard let at = chapters.firstIndex(where: { $0.id == id }), at > 0 else { return nil }
        return chapters[at - 1]
    }

    func older(than id: String) -> ChapterEntry? {
        guard let at = chapters.firstIndex(where: { $0.id == id }), at + 1 < chapters.count else { return nil }
        return chapters[at + 1]
    }

    /// "Chapter 179 is missing from this source" when the chapter after this one skips ahead, else nil.
    func skippedNote(after id: String) -> String? {
        guard let this = chapter(id)?.number, let next = newer(than: id)?.number, let skipped = skippedChapters(from: this, to: next) else { return nil }
        return skipped.count == 1 ? "Chapter \(skipped.lowerBound) is missing from this source" : "Chapters \(skipped.lowerBound) to \(skipped.upperBound) are missing from this source"
    }

    func label(_ chapter: ChapterEntry) -> String {
        if let title = chapter.title { return title }
        if let number = chapter.number { return "Chapter \(number.formatted(.number.precision(.fractionLength(0...2))))" }
        return "Chapter"
    }

    var currentSegment: Segment? { segments.first { $0.chapterID == current } }

    private func pageList(_ chapterID: String) async throws -> PageList {
        if let hit = cache[chapterID] { return hit }
        let list = try await engine.pages(source: ref.source, series: ref.id, chapter: chapterID)
        cache[chapterID] = list
        return list
    }

    /// Opens a chapter, at a page and how far down it, using the pages already fetched if there are.
    func open(_ chapterID: String, page: Int, offset: Double) async {
        autoScrolling = false
        loading = true
        error = nil
        do {
            let list = try await pageList(chapterID)
            let at = min(max(page, 0), max(list.images.count - 1, 0))
            if UserDefaults.standard.string(forKey: "mode.\(ref.source).\(ref.id)") == nil {
                mode = list.reading == .pagedRtl ? .rtl : list.reading == .paged ? .paged : list.reading == .longStrip ? .strip : ReadingMode(rawValue: UserDefaults.standard.string(forKey: "defaultMode") ?? "") ?? .strip
            }
            current = chapterID
            self.page = at
            pagedIndex = at
            // The strip's first rows report themselves as "the page at the top" while it is laid out; that is
            // not where the person is, so reporting waits until the strip has been put back.
            restoring = mode == .strip
            segments = [Segment(chapterID: chapterID, pages: list)]
            restore = Restore(chapter: chapterID, page: at, offset: offset)
            restoreToken += 1
            latest = (chapterID, at, offset)
            loading = false
            prefetchNewer()
        } catch {
            loading = false
            self.error = describe(error)
        }
    }

    func setMode(_ new: ReadingMode) {
        guard new != mode, let segment = currentSegment else { return }
        autoScrolling = false
        UserDefaults.standard.set(new.rawValue, forKey: "mode.\(ref.source).\(ref.id)")
        let offset = latest?.chapter == current ? latest?.offset ?? 0 : 0
        mode = new
        zoom = 1
        restoring = new == .strip
        segments = [segment]
        pagedIndex = page
        restore = Restore(chapter: current, page: page, offset: new == .strip ? offset : 0)
        restoreToken += 1
    }

    // MARK: cover

    /// Makes the page on screen the series' cover. False when the picture could not be read.
    func useCurrentPageAsCover() async -> Bool {
        guard let segment = currentSegment, segment.pages.images.indices.contains(page), let link = URL(string: segment.pages.images[page]),
              let image = await ImageLoader.shared.image(link, headers: segment.headers, widthPx: 600),
              let jpeg = coverJPEG(image, width: 600) else { return false }
        return (try? engine.setCustomCover(source: ref.source, series: ref.id, jpeg: jpeg)) != nil
    }

    // MARK: bookmarks

    /// Whether the page the person is on (the one at the top of the strip) is marked.
    var isBookmarked: Bool { bookmarks.contains { $0.chapter == current && Int($0.page) == page } }

    func loadBookmarks() {
        bookmarks = (try? engine.bookmarks(source: ref.source, series: ref.id)) ?? []
    }

    /// Marks the page on screen, and how far down it in the strip; marking a marked page takes the mark off.
    func toggleBookmark() {
        if isBookmarked {
            try? engine.removeBookmark(source: ref.source, series: ref.id, chapter: current, page: UInt32(page))
        } else {
            let offset = mode == .strip && latest?.chapter == current && latest?.page == page ? latest?.offset ?? 0 : 0
            try? engine.addBookmark(source: ref.source, series: ref.id, chapter: current, page: UInt32(page), offset: offset)
        }
        loadBookmarks()
    }

    // MARK: ahead

    /// The next chapter's pages and its first images, fetched while this one is being read.
    func prefetchNewer() {
        guard let next = newer(than: current) else { return }
        Task { [weak self] in
            guard let self, let list = try? await pageList(next.id) else { return }
            warm(list, from: 0, count: 3)
        }
    }

    /// Loads the pictures just after `index` so they are there when the page is turned.
    func warm(_ list: PageList, from index: Int, count: Int) {
        let headers = Dictionary(list.headers.map { ($0.name, $0.value) }, uniquingKeysWith: { $1 })
        let scale = Aki.screenScale
        let width = Aki.screenSize.width * (mode == .strip ? zoom : 2) * scale
        for url in list.images.dropFirst(max(index, 0)).prefix(count) where warmed.insert("\(url)@\(Int(width))").inserted {
            guard let link = URL(string: url) else { continue }
            Task.detached(priority: .utility) { [weak self] in
                guard let picture = await ImageLoader.shared.image(link, headers: headers, widthPx: width) else { return }
                // The shape is noted as soon as it is known, so a row not yet on screen already has the right height.
                await self?.noteAspect(picture.size.width / picture.size.height, of: url)
            }
        }
    }

    /// Continues the strip with the next chapter once the reader is near the end of the last one.
    func appendNewerIfNeeded() async {
        guard mode == .strip, !appending, let last = segments.last, let next = newer(than: last.chapterID) else { return }
        appending = true
        defer { appending = false }
        guard let list = try? await pageList(next.id), segments.last?.chapterID == last.chapterID else { return }
        segments.append(Segment(chapterID: next.id, pages: list))
    }

    // MARK: position

    /// The row at the top of the strip, or the page on screen: where the person is.
    func report(chapter: String, page: Int, offset: Double) {
        guard !restoring else { return }
        // Called on every frame of a scroll; the work ahead only changes when the page does.
        let moved = chapter != current || self.page != page
        if chapter != current {
            // Reading on into the next chapter finishes the one before.
            if let at = chapters.firstIndex(where: { $0.id == chapter }), let was = chapters.firstIndex(where: { $0.id == current }), at < was {
                finish(current)
            }
            current = chapter
            prefetchNewer()
        }
        if self.page != page { self.page = page }
        latest = (chapter, page, offset)
        scheduleSave()
        if moved, let segment = segments.first(where: { $0.chapterID == chapter }) {
            warm(segment.pages, from: page + 1, count: 3)
            // And the pages just above: scrolling back up meets them already loaded and already the right height.
            warm(segment.pages, from: max(page - 3, 0), count: min(3, page))
            if mode == .strip, page >= segment.pages.images.count - 3 { Task { await appendNewerIfNeeded() } }
        }
    }

    /// The end of a chapter has been reached.
    func finish(_ chapterID: String) {
        // Only a chapter still laid out can have been reached: opening another chapter tears the strip down, and its rows then
        // report positions that are not the person reading to the end.
        guard !restoring, segments.contains(where: { $0.chapterID == chapterID }), finished.insert(chapterID).inserted else { return }
        try? engine.setProgress(source: ref.source, series: ref.id, chapter: chapterID, page: 0, offset: 0, finished: true)
    }

    private func scheduleSave() {
        saveTask?.cancel()
        saveTask = Task { [weak self] in
            try? await Task.sleep(for: .milliseconds(350))
            guard !Task.isCancelled else { return }
            self?.flush()
        }
    }

    /// Writes the position down now.
    func flush() {
        saveTask?.cancel()
        guard let latest, !finished.contains(latest.chapter) else { return }
        try? engine.setProgress(source: ref.source, series: ref.id, chapter: latest.chapter, page: UInt32(latest.page), offset: latest.offset, finished: false)
    }
}
