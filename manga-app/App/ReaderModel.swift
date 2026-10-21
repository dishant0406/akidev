import AkiManga
import Observation
import SwiftUI

enum ReadingMode: String, CaseIterable, Identifiable {
    case strip, paged, rtl
    var id: String { rawValue }
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
    var loading = false
    var error: (title: String, message: String)?
    var restore: Restore?
    var restoreToken = 0
    /// Set while the strip is being put back where it was, so the scrolling that does it is not saved.
    @ObservationIgnored var restoring = false
    @ObservationIgnored var aspects: [String: CGFloat] = [:]
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
            Task.detached(priority: .utility) { _ = await ImageLoader.shared.image(link, headers: headers, widthPx: width) }
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
            if mode == .strip, page >= segment.pages.images.count - 3 { Task { await appendNewerIfNeeded() } }
        }
    }

    /// The end of a chapter has been reached.
    func finish(_ chapterID: String) {
        guard finished.insert(chapterID).inserted else { return }
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
