import AkiManga
import SwiftUI
import os

/// The reader: a long strip, or pages turned left to right or right to left, with pinch zoom. Chrome is
/// glass and hides on tap. Where the person is (chapter, page, how far down it) is saved as they read.
struct ReaderView: View {
    @State private var model: ReaderModel
    @Environment(\.dismiss) private var dismiss
    @Environment(AppModel.self) private var app
    @State private var settings = false
    @State private var chapters = false

    init(model: ReaderModel) {
        _model = State(initialValue: model)
    }

    var body: some View {
        ZStack {
            app.prefs.readerBackground.color.ignoresSafeArea()
            if !model.segments.isEmpty {
                switch model.mode {
                case .strip: StripReader(model: model)
                case .paged, .rtl: PagedReader(model: model, close: { dismiss() })
                }
            } else if let error = model.error {
                AkiEmptyState(title: error.title, message: error.message, action: ("Try again", { Task { await model.open(model.current, page: model.page, offset: 0) } }))
            }
        }
        // One loader for the whole opening: while the chapter's pages are fetched, and while the strip is put at your place.
        .overlay {
            if model.error == nil && (model.segments.isEmpty || (model.mode == .strip && model.restoring)) {
                AkiOpeningLoader(label: model.chapter(model.current).map(model.label)).transition(.opacity)
            }
        }
        .animation(.easeOut(duration: 0.2), value: model.restoring)
        .overlay(alignment: .top) { if model.chrome { topBar.transition(.opacity) } }
        .overlay(alignment: .bottom) { if model.chrome, model.currentSegment != nil { bottomBar.transition(.opacity) } }
        .environment(\.colorScheme, .dark)
        .statusBarHidden(!model.chrome)
        .akiAnimation(quick: true, value: model.chrome)
        .task { await model.open(model.current, page: model.page, offset: model.restore?.offset ?? 0) }
        .akiSheet(isPresented: $settings) { ReaderSettingsSheet(model: model) }
        .akiSheet(isPresented: $chapters, full: true) {
            ReaderChaptersSheet(model: model) { chapter in
                chapters = false
                // Opens where the person was in it, or at its start.
                Task { await model.open(chapter.id, page: Int(chapter.page ?? 0), offset: 0) }
            }
        }
        .onAppear { UIApplication.shared.isIdleTimerDisabled = app.prefs.keepAwake; AppDelegate.lock(app.prefs.orientation.mask) }
        .onChange(of: app.prefs.orientation) { _, new in AppDelegate.lock(new.mask) }
        .onDisappear { model.flush(); UIApplication.shared.isIdleTimerDisabled = false; AppDelegate.lock(.portrait); AppDelegate.lock(OrientationLock.auto.mask) }
    }

    private var topBar: some View {
        AkiGlassGroup(spacing: 12) {
            HStack(spacing: 12) {
                AkiIconButton(symbol: "back", label: "Back", onMedia: true, scrim: Aki.scrimStrong) { dismiss() }
                AkiTap(action: { chapters = true }) {
                    VStack(alignment: .leading, spacing: 0) {
                        AkiText(model.ref.title, .captionStrong, tone: .onMedia, lines: 1)
                        if let chapter = model.chapter(model.current) { AkiText(model.label(chapter), .caption, tone: .onMedia, lines: 1).opacity(0.78) }
                    }
                    .padding(.horizontal, 16).padding(.vertical, 8)
                    .akiGlass(.capsule, tint: Aki.scrimStrong, interactive: false)
                }
                .accessibilityLabel("Chapters")
                Spacer(minLength: 0)
                if model.mode == .strip {
                    AkiIconButton(symbol: model.autoScrolling ? "pause" : "play", label: model.autoScrolling ? "Stop auto-scroll" : "Auto-scroll", onMedia: true, scrim: Aki.scrimStrong) { model.autoScrolling.toggle() }
                }
                AkiIconButton(symbol: "sliders", label: "Reader settings", onMedia: true, scrim: Aki.scrimStrong) { settings = true }
            }
        }
        .padding(.horizontal, 16).padding(.top, 8)
    }

    private var bottomBar: some View {
        AkiGlassGroup(spacing: 12) {
            HStack(spacing: 12) {
                if let older = model.older(than: model.current) {
                    AkiIconButton(symbol: "previous", label: "Previous chapter", onMedia: true, scrim: Aki.scrimStrong) { Task { await model.open(older.id, page: 0, offset: 0) } }
                }
                AkiIconButton(symbol: "list", label: "Chapters", onMedia: true, scrim: Aki.scrimStrong) { chapters = true }
                let count = model.currentSegment?.pages.images.count ?? 0
                AkiText("\(min(model.page + 1, max(count, 1))) / \(count)", .caption, tone: .onMedia)
                    .monospacedDigit()
                    .padding(.horizontal, 18).padding(.vertical, 12).akiGlass(.capsule, tint: Aki.scrimStrong, interactive: false)
                AkiIconButton(symbol: model.isBookmarked ? "bookmark-solid" : "bookmark", label: model.isBookmarked ? "Remove bookmark" : "Bookmark this page", tint: model.isBookmarked ? Aki.primary : nil, onMedia: true, scrim: Aki.scrimStrong) { model.toggleBookmark() }
                    .akiHaptic(trigger: model.isBookmarked)
                if let newer = model.newer(than: model.current) {
                    AkiIconButton(symbol: "next", label: "Next chapter", onMedia: true, scrim: Aki.scrimStrong) { Task { await model.open(newer.id, page: 0, offset: 0) } }
                }
            }
        }
        .padding(.bottom, 24)
    }
}

// MARK: long strip

/// The strip's scroll state, kept where reading or writing it does not redraw anything.
private final class StripScroll {
    var y: CGFloat = 0
    /// Where auto-scroll has taken the strip to, so a slow speed is not lost to rounding; nil until it starts moving.
    var target: CGFloat?
    /// A finger is on the strip, or it is coasting.
    var touched = false
    /// Auto-scroll waits until this moment after a finger lifts.
    var resumeAt: CFTimeInterval = 0
    var atEnd = false
    /// When auto-scroll got to the end of the rows and found a next chapter that has not come.
    var waitingSince: CFTimeInterval?
}

/// Pages one under another with no gap, the next chapter joined on as the end nears. Pinch to zoom the
/// width (it snaps back to the screen width below 1×); when zoomed, drag sideways too.
private struct StripReader: View {
    let model: ReaderModel
    @State private var position = ScrollPosition()
    @State private var width = Aki.screenSize.width
    @State private var scroll = StripScroll()
    @State private var live: CGFloat = 1
    @State private var clock = FrameClock()
    @Environment(AppModel.self) private var app
    @Environment(\.scenePhase) private var phase

    var body: some View {
        ScrollView(model.zoom > 1 ? [.vertical, .horizontal] : [.vertical]) {
            // The rows are a view of their own: auto-scroll moves `position` on every frame and redraws this body, and a
            // child whose inputs have not changed is not rebuilt.
            LazyVStack(spacing: 0) {
                StripRows(model: model, width: width * model.zoom, shift: keepPlace)
            }
            .scrollTargetLayout()
            .frame(width: width * model.zoom)
        }
        .scrollPosition($position)
        // The strip is laid out at once but not shown until it has been moved to where the person was: showing it first made
        // the chapter open at the top and then jump.
        .opacity(model.restoring ? 0 : 1)
        .onScrollGeometryChange(for: CGFloat.self) { $0.contentOffset.y } action: { _, y in scroll.y = y }
        .onScrollGeometryChange(for: Bool.self) { $0.contentOffset.y + $0.containerSize.height >= $0.contentSize.height - 2 } action: { _, end in scroll.atEnd = end }
        .onScrollPhaseChange { _, phase in
            let touched = phase == .tracking || phase == .interacting || phase == .decelerating
            guard touched != scroll.touched else { return }
            scroll.touched = touched
            // After a finger lifts, auto-scroll carries on from wherever the strip came to rest, a second later.
            scroll.target = nil
            if !touched { scroll.resumeAt = CACurrentMediaTime() + 1 }
        }
        .onGeometryChange(for: CGFloat.self) { $0.size.width } action: { width = $0 }
        .scaleEffect(live)
        .simultaneousGesture(
            MagnifyGesture()
                .onChanged { live = min(max($0.magnification, 0.6), 3 / max(model.zoom, 1)) }
                .onEnded { value in
                    model.zoom = min(max(model.zoom * value.magnification, 1), 3)
                    live = 1
                    scroll.target = nil
                }
        )
        .onTapGesture { model.chrome.toggle() }
        .task(id: model.restoreToken) { await restore() }
        .onAppear { clock.tick = { step($0) } }
        .onChange(of: model.autoScrolling) { _, on in on ? startAutoScroll() : stopAutoScroll() }
        .onChange(of: phase) { _, new in if new != .active { model.autoScrolling = false } }
        .onDisappear {
            model.autoScrolling = false
            stopAutoScroll()
            // The tick holds this view's state, and the state holds the clock: break the loop.
            clock.tick = { _ in }
        }
    }

    /// A page above the one being read came in taller or shorter than the room held for it: the strip is moved by the same
    /// amount, so what is on screen stays where it was instead of jumping.
    private func keepPlace(by change: CGFloat) {
        scroll.y += change
        scroll.target = scroll.target.map { $0 + change }
        position.scrollTo(y: scroll.y)
    }

    private func startAutoScroll() {
        scroll.target = nil
        scroll.waitingSince = nil
        model.chrome = false
        UIApplication.shared.isIdleTimerDisabled = true
        clock.start()
    }

    private func stopAutoScroll() {
        clock.stop()
        UIApplication.shared.isIdleTimerDisabled = app.prefs.keepAwake
    }

    /// Moves the strip on by the time since the last frame times the speed. A finger on the strip, or just off it, holds it
    /// still. At the end of the last chapter it stops; at the end of a chapter whose next one has not come in a few seconds
    /// (it failed to load) it stops too, with the bars back so the person sees where they are.
    private func step(_ seconds: CFTimeInterval) {
        guard model.autoScrolling else { return }
        let now = CACurrentMediaTime()
        if scroll.touched || now < scroll.resumeAt {
            scroll.target = nil
            return
        }
        if scroll.atEnd {
            scroll.target = nil
            if model.segments.last.flatMap({ model.newer(than: $0.chapterID) }) == nil {
                model.autoScrolling = false
            } else if let since = scroll.waitingSince, now - since > 4 {
                model.autoScrolling = false
                model.chrome = true
            } else if scroll.waitingSince == nil {
                scroll.waitingSince = now
            }
            return
        }
        scroll.waitingSince = nil
        let next = (scroll.target ?? scroll.y) + CGFloat(seconds) * app.prefs.autoScrollSpeed.points
        scroll.target = next
        position.scrollTo(y: next)
    }

    /// Puts the strip back where it was: the page, and how far down it, once that page's shape is known.
    private func restore() async {
        guard let target = model.restore, let segment = model.segments.first(where: { $0.chapterID == target.chapter }) else {
            model.restoring = false
            return
        }
        model.restoring = true
        scroll.target = nil
        try? await Task.sleep(for: .milliseconds(120))
        position.scrollTo(id: "\(target.chapter)|\(target.page)", anchor: .top)
        model.restore = nil
        if target.offset > 0.001, segment.pages.images.indices.contains(target.page) {
            let url = segment.pages.images[target.page]
            // How far down the page is only known once its picture is: waited for, but not for ever.
            for _ in 0..<15 where model.aspects[url] == nil { try? await Task.sleep(for: .milliseconds(100)) }
            try? await Task.sleep(for: .milliseconds(150))
            if let aspect = model.aspects[url] {
                position.scrollTo(y: scroll.y + CGFloat(target.offset) * (width * model.zoom / aspect))
                try? await Task.sleep(for: .milliseconds(150))
            }
        }
        // Shown now, and the page counter told where it is (nothing reports it until the person scrolls).
        model.restoring = false
        model.report(chapter: target.chapter, page: target.page, offset: target.offset)
    }
}

/// Every row of every chapter laid out so far: the pages, and a closing row after each chapter.
private struct StripRows: View {
    let model: ReaderModel
    let width: CGFloat
    /// Called with how much a row above the top of the screen grew (negative: shrank) when its picture arrived.
    let shift: (CGFloat) -> Void
    @Environment(\.dismiss) private var dismiss

    /// Each row has an id the scroll position can name.
    private struct Item: Identifiable {
        enum Kind {
            case page(Int, String, [String: String])
            case end(Segment)
        }
        let id: String
        let chapter: String
        let kind: Kind
    }

    /// How far a row is scrolled past the top of the screen, in steps of `step` points, and its height; changes only
    /// when a step is crossed, so reading the position is not done on every frame of a scroll.
    private struct RowSpan: Equatable {
        static let step: CGFloat = 40
        var bucket: Int
        var height: Int
        static let off = RowSpan(bucket: -1, height: 0)
        init(bucket: Int, height: Int) { self.bucket = bucket; self.height = height }
        init(frame: CGRect) {
            guard frame.minY <= 1, frame.maxY > 1, frame.height.isFinite else { self = .off; return }
            self.init(bucket: Int(-frame.minY / Self.step), height: Int(frame.height))
        }
        var offset: Double { min(max(Double(CGFloat(bucket) * Self.step) / Double(max(height, 1)), 0), 1) }
    }

    private var items: [Item] {
        model.segments.flatMap { segment -> [Item] in
            let headers = segment.headers
            let pages = segment.pages.images.enumerated().map { index, url in
                Item(id: "\(segment.chapterID)|\(index)", chapter: segment.chapterID, kind: .page(index, url, headers))
            }
            return pages + [Item(id: "\(segment.chapterID)|end", chapter: segment.chapterID, kind: .end(segment))]
        }
    }

    var body: some View {
        ForEach(items) { item in
            switch item.kind {
            case .page(let index, let url, let headers):
                AkiImage(url: url, headers: headers, placeholder: model.aspects[url].map { 1 / $0 } ?? model.typicalHeightRatio, onAspect: { model.noteAspect($0, of: url) }, pageNumber: index)
                    .frame(width: width)
                    .onGeometryChange(for: RowSpan.self) { RowSpan(frame: $0.frame(in: .scrollView)) } action: { span in
                        if span != .off { model.report(chapter: item.chapter, page: index, offset: span.offset) }
                    }
                    .onGeometryChange(for: CGRect.self) { $0.frame(in: .scrollView) } action: { old, new in
                        guard old.height > 0, abs(new.height - old.height) > 0.5, old.minY < 0 else { return }
                        // Wholly above the top of the screen: all of the change is above what is read. Across it: the share above.
                        let above = old.maxY <= 0 ? 1 : min(max(-old.minY / old.height, 0), 1)
                        shift((new.height - old.height) * above)
                    }
            case .end(let segment):
                chapterEnd(segment)
            }
        }
    }

    private func chapterEnd(_ segment: Segment) -> some View {
        let next = model.newer(than: segment.chapterID)
        let note = model.skippedNote(after: segment.chapterID)
        return VStack(spacing: 10) {
            AkiText("End of \(model.chapter(segment.chapterID).map(model.label) ?? "chapter")", .eyebrow, tone: .onMedia)
            if let next {
                AkiText(model.segments.contains { $0.chapterID == next.id } ? "Next chapter below" : "Next: \(model.label(next))", .caption, tone: .muted)
                if let note { AkiText(note, .caption, tone: .muted) }
            } else {
                AkiText("You are up to date", .title, tone: .onMedia)
                AkiButton("Back to the series") { dismiss() }.padding(.top, 6)
            }
        }
        .frame(width: width, height: next == nil ? 220 : note == nil ? 96 : 124)
        .onGeometryChange(for: CGFloat.self) { $0.frame(in: .scrollView).minY } action: { top in
            if top < Aki.screenSize.height * 0.9 { model.finish(segment.chapterID) }
        }
    }
}

// MARK: pages

/// One page at a time, turned by swiping or by tapping the edges (the right edge goes on in left-to-right
/// reading, the left edge in right-to-left). Double tap or pinch to zoom.
private struct PagedReader: View {
    @Bindable var model: ReaderModel
    @Environment(AppModel.self) private var app
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    let close: () -> Void

    var body: some View {
        let segment = model.currentSegment ?? model.segments[0]
        let headers = segment.headers
        let count = segment.pages.images.count
        let rtl = model.mode == .rtl
        TabView(selection: $model.pagedIndex) {
            ForEach(Array(segment.pages.images.enumerated()), id: \.offset) { index, url in
                AkiPage(number: index, url: url, headers: headers, onTap: { tap($0, count: count, rtl: rtl) })
                    .tag(index)
            }
            endPage(segment).tag(count)
            // Swiping on past the end page opens the next chapter.
            if let next = model.newer(than: segment.chapterID) { continuePage(next).tag(count + 1) }
        }
        .tabViewStyle(.page(indexDisplayMode: .never))
        // A new chapter is a new set of pages: without this the view stays on the page position of the one before.
        .id(segment.chapterID)
        .environment(\.layoutDirection, rtl ? .rightToLeft : .leftToRight)
        .onChange(of: model.pagedIndex) { was, index in
            // Only a swipe from the end page to the one after it opens the next chapter: when a chapter is replaced the
            // selection can briefly sit past the end of the new one, which is not a swipe.
            if index == count + 1 {
                if was == count, let next = model.newer(than: segment.chapterID) { Task { await model.open(next.id, page: 0, offset: 0) } }
            } else if index == count {
                model.finish(segment.chapterID)
            } else {
                model.report(chapter: segment.chapterID, page: index, offset: 0)
            }
        }
        .onAppear { model.restoring = false }
    }

    private func tap(_ x: CGFloat, count: Int, rtl: Bool) {
        // x is 0 at the left edge of the screen whatever the reading direction.
        let flipped = rtl != app.prefs.invertTaps
        let step: Int? = x > 0.7 ? (flipped ? -1 : 1) : x < 0.3 ? (flipped ? 1 : -1) : nil
        guard let step else { return model.chrome.toggle() }
        let next = model.pagedIndex + step
        if (0...(count + (model.newer(than: model.current) == nil ? 0 : 1))).contains(next) {
            withAnimation(AkiMotion.quick(reduceMotion)) { model.pagedIndex = next }
        }
    }

    /// Where the swipe past the end page lands while the next chapter is fetched; if that fails it says so and can try again.
    private func continuePage(_ next: ChapterEntry) -> some View {
        VStack(spacing: 14) {
            if let error = model.error {
                AkiText(error.title, .title, tone: .onMedia)
                AkiButton("Try again", variant: .primary) { Task { await model.open(next.id, page: 0, offset: 0) } }
            } else {
                AkiText(model.label(next), .eyebrow, tone: .onMedia)
                AkiSpinner()
            }
        }
        .environment(\.layoutDirection, .leftToRight)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private func endPage(_ segment: Segment) -> some View {
        VStack(spacing: 14) {
            AkiText("End of \(model.chapter(segment.chapterID).map(model.label) ?? "chapter")", .eyebrow, tone: .onMedia)
            if let note = model.skippedNote(after: segment.chapterID) { AkiText(note, .caption, tone: .muted) }
            if let next = model.newer(than: segment.chapterID) {
                AkiButton("Next · \(model.label(next))", variant: .primary, size: .large) { Task { await model.open(next.id, page: 0, offset: 0) } }
            } else {
                AkiText("You are up to date", .title, tone: .onMedia)
            }
            AkiButton("Back to the series", action: close)
        }
        .environment(\.layoutDirection, .leftToRight)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}
