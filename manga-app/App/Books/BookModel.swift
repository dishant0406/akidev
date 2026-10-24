import AkiManga
import ImageIO
import Observation
import SwiftUI

/// One open book: where the person is, how its chapters are set in type, and what the reader's screen shows.
///
/// A chapter is read and set in type when it is needed, on a background queue, and only the chapters around the current one are
/// kept: a long book costs what a few chapters cost.
@MainActor @Observable
final class BookModel: BookPaging {
    let info: BookInfo
    let contents: [BookEntry]
    let chapterCount: Int
    private let reader: BookReader
    private let engine: Engine
    private let sizes: [UInt64]
    private let saveStyle: (BookStyle) -> Void
    private let invertTaps: Bool

    var style: BookStyle {
        didSet {
            guard style != oldValue else { return }
            saveStyle(style)
            var type = style
            type.theme = oldValue.theme
            if type != oldValue { typeChanged() }
        }
    }

    /// The page being read, once the book has opened.
    private(set) var place: BookPlace?
    /// What the pager shows now, when it should rather than wait to be turned.
    private(set) var jump: BookJump?
    var chromeVisible = false
    /// How far through the book, 0 to 1; the slider moves it and lifting the finger goes there.
    var progress: Double = 0
    var failure: (title: String, message: String)?
    /// A link to a web page, waiting for the person to say whether to leave the app.
    var externalLink: URL?
    /// Where the person was before following a link inside the book: the way back.
    private(set) var trail: [Spot] = []
    var systemDark = false
    private(set) var screen = CGSize.zero
    private(set) var safe = UIEdgeInsets.zero

    @ObservationIgnored private var layouts: [Int: ChapterLayout] = [:]
    @ObservationIgnored private var building: Set<Int> = []
    @ObservationIgnored private var generation = 0
    @ObservationIgnored private var jumps = 0
    @ObservationIgnored private var wanted: (spot: Spot, forward: Bool, animated: Bool)?
    @ObservationIgnored private var restyle: Task<Void, Never>?
    @ObservationIgnored private let pictures = NSCache<NSString, CGImage>()
    @ObservationIgnored private var loadingPictures: Set<String> = []
    /// Pictures that could not be read, so a page redrawn does not try them again.
    @ObservationIgnored private var brokenPictures: Set<String> = []

    /// A place in the book that is not yet a page: the chapter, and either a position, an id or how far through it.
    struct Spot: Equatable {
        var chapter: Int
        var offset: Int?
        var anchor: String?
        var fraction: Double?
    }

    /// Opens the book, off the main thread: reading its contents can take a moment for a large one.
    static func open(_ info: BookInfo, engine: Engine, style: BookStyle, invertTaps: Bool, saveStyle: @escaping (BookStyle) -> Void) async throws -> BookModel {
        let (reader, contents, sizes) = try await Task.detached(priority: .userInitiated) {
            let reader = try engine.openBook(id: info.id)
            return (reader, reader.contents(), reader.chapterSizes())
        }.value
        return BookModel(info: info, engine: engine, reader: reader, contents: contents, sizes: sizes, style: style, invertTaps: invertTaps, saveStyle: saveStyle)
    }

    private init(info: BookInfo, engine: Engine, reader: BookReader, contents: [BookEntry], sizes: [UInt64], style: BookStyle, invertTaps: Bool, saveStyle: @escaping (BookStyle) -> Void) {
        self.info = info
        self.engine = engine
        self.reader = reader
        self.contents = contents
        self.sizes = sizes
        chapterCount = sizes.count
        self.style = style
        self.saveStyle = saveStyle
        self.invertTaps = invertTaps != info.rtl
        progress = info.progress
        pictures.totalCostLimit = 40 * 1024 * 1024
    }

    // MARK: BookPaging

    var theme: BookTheme { style.theme.resolved(dark: systemDark) }
    var pageInsets: UIEdgeInsets { BookGeometry.insets(safe: safe, margin: style.margin) }

    func layout(chapter: Int) -> ChapterLayout? {
        if let layout = layouts[chapter] { return layout }
        build(chapter)
        return nil
    }

    func footer(for place: BookPlace, of layout: ChapterLayout) -> (title: String, number: String) {
        let entry = contents.first { Int($0.chapter) == place.chapter && $0.anchor == nil } ?? contents.last { Int($0.chapter) <= place.chapter }
        return (entry?.title ?? info.title, "\(place.page + 1) / \(layout.count)")
    }

    func picture(path: String, width: CGFloat, ready: @escaping @MainActor @Sendable () -> Void) -> CGImage? {
        let pixels = Int((width * Aki.screenScale).rounded())
        let key = "\(path)|\(pixels)"
        if let found = pictures.object(forKey: key as NSString) { return found }
        guard !brokenPictures.contains(key), loadingPictures.insert(key).inserted else { return nil }
        let reader = reader
        Task.detached(priority: .userInitiated) {
            let image = (try? reader.resource(path: path)).flatMap { data -> CGImage? in
                guard let source = CGImageSourceCreateWithData(data as CFData, nil) else { return nil }
                let options: [CFString: Any] = [kCGImageSourceCreateThumbnailFromImageAlways: true, kCGImageSourceCreateThumbnailWithTransform: true, kCGImageSourceThumbnailMaxPixelSize: max(pixels, 64)]
                return CGImageSourceCreateThumbnailAtIndex(source, 0, options as CFDictionary)
            }
            await MainActor.run {
                self.loadingPictures.remove(key)
                guard let image else {
                    self.brokenPictures.insert(key)
                    return
                }
                self.pictures.setObject(image, forKey: key as NSString, cost: image.bytesPerRow * image.height)
                ready()
            }
        }
        return nil
    }

    func turned(to place: BookPlace) {
        self.place = place
        // A page counted from the end waits for its chapter to be set in type.
        guard let layout = layouts[place.chapter], place.page >= 0 else { return }
        progress = progress(of: place, in: layout)
        let finished = place.chapter == chapterCount - 1 && place.page >= layout.count - 1
        engine.saveBookPositionQuietly(id: info.id, chapter: place.chapter, offset: layout.start(of: place.page), progress: progress, finished: finished)
        // The chapters around this one are made ready before they are needed; the ones far away are let go.
        if place.page >= layout.count - 3 { build(place.chapter + 1) }
        if place.page <= 2 { build(place.chapter - 1) }
        for chapter in layouts.keys where abs(chapter - place.chapter) > 2 { layouts[chapter] = nil }
    }

    func tapped(_ zone: BookTapZone, link: String?) {
        if let link { return follow(link) }
        switch zone {
        case .middle: withAnimation(.snappy(duration: 0.25)) { chromeVisible.toggle() }
        case .back: turn(by: invertTaps ? 1 : -1)
        case .forward: turn(by: invertTaps ? -1 : 1)
        }
    }

    // MARK: Opening and moving

    /// Tells the book how big the screen is; sets the first page when it is known.
    func resize(_ size: CGSize, safe: UIEdgeInsets) {
        guard size.width > 0, size != screen || safe != self.safe else { return }
        let first = screen == .zero
        screen = size
        self.safe = safe
        if first {
            go(to: Spot(chapter: min(Int(info.chapter), max(chapterCount - 1, 0)), offset: Int(info.offset)), forward: true, animated: false)
        } else {
            typeChanged()
        }
    }

    /// Goes to a place in the book: at once if its chapter is ready, when it is if not.
    func go(to spot: Spot, forward: Bool = true, animated: Bool = false) {
        wanted = (spot, forward, animated)
        if let layout = layouts[spot.chapter] { arrive(layout) } else { build(spot.chapter) }
    }

    private func arrive(_ layout: ChapterLayout) {
        guard let (spot, forward, animated) = wanted, spot.chapter == layout.index else { return }
        wanted = nil
        let page: Int
        if let anchor = spot.anchor, let offset = layout.offset(of: anchor) {
            page = layout.page(containing: offset)
        } else if let offset = spot.offset {
            page = layout.page(containing: offset)
        } else if let fraction = spot.fraction {
            page = min(Int(fraction * Double(layout.count)), layout.count - 1)
        } else {
            page = 0
        }
        let target = BookPlace(chapter: layout.index, page: page)
        jumps += 1
        jump = BookJump(id: jumps, place: target, forward: forward, animated: animated)
        turned(to: target)
    }

    /// Turns the page by one, across chapter ends.
    private func turn(by step: Int) {
        guard let place, place.page >= 0, let layout = layouts[place.chapter] else { return }
        let page = place.page + step
        if page >= 0, page < layout.count {
            move(to: BookPlace(chapter: place.chapter, page: page), forward: step > 0)
        } else if step > 0, place.chapter + 1 < chapterCount {
            go(to: Spot(chapter: place.chapter + 1, offset: 0), forward: true, animated: true)
        } else if step < 0, place.chapter > 0 {
            go(to: Spot(chapter: place.chapter - 1, offset: nil, fraction: 1), forward: false, animated: true)
        }
    }

    private func move(to target: BookPlace, forward: Bool) {
        jumps += 1
        jump = BookJump(id: jumps, place: target, forward: forward, animated: true)
        turned(to: target)
    }

    /// Goes to a place the slider or the contents named: how far through the book, or an entry.
    func go(toProgress fraction: Double) {
        let total = Double(sizes.reduce(0, +))
        guard total > 0 else { return go(to: Spot(chapter: Int(fraction * Double(chapterCount)).clamped(to: 0..<chapterCount), offset: 0)) }
        var before = 0.0
        for (chapter, size) in sizes.enumerated() {
            let share = Double(size) / total
            if fraction <= before + share || chapter == sizes.count - 1 {
                let inside = share > 0 ? (fraction - before) / share : 0
                return go(to: Spot(chapter: chapter, offset: nil, fraction: min(max(inside, 0), 1)), forward: chapter >= (place?.chapter ?? 0))
            }
            before += share
        }
    }

    func open(_ entry: BookEntry) {
        trail = []
        go(to: Spot(chapter: Int(entry.chapter), offset: entry.anchor == nil ? 0 : nil, anchor: entry.anchor), forward: Int(entry.chapter) >= (place?.chapter ?? 0))
    }

    private func follow(_ link: String) {
        if let url = URL(string: link), url.scheme == "http" || url.scheme == "https" {
            externalLink = url
            return
        }
        let reader = reader
        Task {
            // Finding where a link goes can mean reading a large file: not on the main thread.
            guard let target = await Task.detached(priority: .userInitiated, operation: { reader.target(link: link) }).value else { return }
            if let place, let layout = layouts[place.chapter] { trail.append(Spot(chapter: place.chapter, offset: layout.start(of: place.page))) }
            go(to: Spot(chapter: Int(target.chapter), offset: target.anchor == nil ? 0 : nil, anchor: target.anchor), forward: true, animated: false)
        }
    }

    /// Back to where the person was before a link inside the book took them away.
    func goBack() {
        guard let spot = trail.popLast() else { return }
        go(to: spot, forward: false, animated: false)
    }

    // MARK: Setting type

    /// Sets the chapter in type on a background queue. The pages are told when it is done.
    private func build(_ chapter: Int) {
        guard chapter >= 0, chapter < chapterCount, screen != .zero, layouts[chapter] == nil, building.insert(chapter).inserted else { return }
        let typesetter = ChapterTypesetter(
            style: style,
            size: BookGeometry.textSize(screen: screen, safe: safe, margin: style.margin),
            language: info.language,
            pictureSize: { [reader] path in
                guard let data = try? reader.resource(path: path), let source = CGImageSourceCreateWithData(data as CFData, nil),
                      let found = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any],
                      let width = found[kCGImagePropertyPixelWidth] as? CGFloat, let height = found[kCGImagePropertyPixelHeight] as? CGFloat else { return nil }
                return CGSize(width: width, height: height)
            }
        )
        let reader = reader
        let made = generation
        Task.detached(priority: .userInitiated) {
            let content = (try? reader.chapter(index: UInt32(chapter))) ?? Self.unreadable
            let layout = typesetter.layout(index: chapter, chapter: content)
            await MainActor.run { self.finished(layout, generation: made) }
        }
    }

    private func finished(_ layout: ChapterLayout, generation made: Int) {
        guard made == generation else { return }
        building.remove(layout.index)
        layouts[layout.index] = layout
        NotificationCenter.default.post(name: .bookLayoutReady, object: layout.index)
        arrive(layout)
        if let place, place.chapter == layout.index, place.page < 0 { turned(to: BookPlace(chapter: place.chapter, page: layout.count - 1)) }
    }

    /// Takes the place of a chapter that cannot be read, so the book still opens and turns.
    nonisolated private static let unreadable = BookChapter(
        blocks: [BookBlock(start: 0, kind: .text(runs: [BookRun(text: "This part of the book could not be read.", bold: false, italic: true, underline: false, strike: false, smallCaps: false, mono: false, shift: 0, size: 1, link: nil)]), heading: 0, align: .center, left: 0, firstLine: 0, spaceBefore: 0, pageBreakBefore: false)],
        anchors: []
    )

    /// The type or the page changed: set the chapters again, a moment after the last change, and come back to the same words.
    private func typeChanged() {
        restyle?.cancel()
        restyle = Task { [weak self] in
            try? await Task.sleep(for: .milliseconds(250))
            guard !Task.isCancelled, let self else { return }
            var spot = Spot(chapter: self.place?.chapter ?? 0, offset: 0)
            if let place = self.place, let layout = self.layouts[place.chapter] { spot.offset = layout.start(of: place.page) }
            self.generation += 1
            self.layouts = [:]
            self.building = []
            self.go(to: spot)
        }
    }

    private func progress(of place: BookPlace, in layout: ChapterLayout) -> Double {
        let total = Double(sizes.reduce(0, +))
        guard total > 0 else { return (Double(place.chapter) + Double(place.page) / Double(max(layout.count, 1))) / Double(max(chapterCount, 1)) }
        let before = Double(sizes.prefix(place.chapter).reduce(0, +))
        let inside = Double(sizes[place.chapter]) * Double(place.page) / Double(max(layout.count, 1))
        return min((before + inside) / total, 1)
    }
}

private extension Int {
    func clamped(to range: Range<Int>) -> Int { Swift.min(Swift.max(self, range.lowerBound), range.upperBound - 1) }
}

private extension Engine {
    /// The place is a convenience, not the book: failing to keep it must not interrupt reading.
    func saveBookPositionQuietly(id: String, chapter: Int, offset: Int, progress: Double, finished: Bool) {
        try? saveBookPosition(id: id, chapter: UInt32(chapter), offset: UInt32(max(offset, 0)), progress: progress, finished: finished)
    }
}
