import AkiManga
import SwiftUI

/// The reader: a long strip, or pages turned left to right or right to left, with pinch zoom. Chrome is
/// glass and hides on tap. Where the person is (chapter, page, how far down it) is saved as they read.
struct ReaderView: View {
    @State private var model: ReaderModel
    @Environment(\.dismiss) private var dismiss
    @Environment(AppModel.self) private var app

    init(model: ReaderModel) {
        _model = State(initialValue: model)
    }

    var body: some View {
        ZStack {
            Color.black.ignoresSafeArea()
            if !model.segments.isEmpty {
                switch model.mode {
                case .strip: StripReader(model: model, close: { dismiss() })
                case .paged, .rtl: PagedReader(model: model, close: { dismiss() })
                }
            } else if let error = model.error {
                AkiEmptyState(title: error.title, message: error.message, action: ("Try again", { Task { await model.open(model.current, page: model.page, offset: 0) } }))
            } else {
                AkiSpinner()
            }
        }
        .overlay(alignment: .top) { if model.chrome { topBar.transition(.opacity) } }
        .overlay(alignment: .bottom) { if model.chrome, model.currentSegment != nil { bottomBar.transition(.opacity) } }
        .environment(\.colorScheme, .dark)
        .statusBarHidden(!model.chrome)
        .animation(.smooth(duration: 0.2), value: model.chrome)
        .task { await model.open(model.current, page: model.page, offset: model.restore?.offset ?? 0) }
        .onAppear { UIApplication.shared.isIdleTimerDisabled = app.prefs.keepAwake }
        .onDisappear { model.flush(); UIApplication.shared.isIdleTimerDisabled = false }
    }

    private var topBar: some View {
        AkiGlassGroup(spacing: 12) {
            HStack(spacing: 12) {
                AkiIconButton(symbol: "chevron.left", label: "Back") { dismiss() }
                VStack(alignment: .leading, spacing: 0) {
                    AkiText(model.ref.title, .captionStrong, tone: .onDark, lines: 1)
                    if let chapter = model.chapter(model.current) { AkiText(model.label(chapter), .caption, tone: .dim, lines: 1) }
                }
                .padding(.horizontal, 16).padding(.vertical, 8)
                .akiGlass(.capsule, interactive: false)
                Spacer(minLength: 0)
                AkiMenu(symbol: model.mode.symbol, label: "Reading mode", items: ReadingMode.allCases.map { mode in
                    AkiMenuItem(title: mode.title, icon: mode.symbol, isSelected: mode == model.mode) { model.setMode(mode) }
                })
            }
        }
        .padding(.horizontal, 16).padding(.top, 8)
    }

    private var bottomBar: some View {
        AkiGlassGroup(spacing: 12) {
            HStack(spacing: 12) {
                if let older = model.older(than: model.current) {
                    AkiIconButton(symbol: "backward.end.fill", label: "Previous chapter") { Task { await model.open(older.id, page: 0, offset: 0) } }
                }
                let count = model.currentSegment?.pages.images.count ?? 0
                AkiText("\(min(model.page + 1, max(count, 1))) / \(count)", .caption, tone: .onDark)
                    .monospacedDigit()
                    .padding(.horizontal, 18).padding(.vertical, 12).akiGlass(.capsule, interactive: false)
                if let newer = model.newer(than: model.current) {
                    AkiIconButton(symbol: "forward.end.fill", label: "Next chapter") { Task { await model.open(newer.id, page: 0, offset: 0) } }
                }
            }
        }
        .padding(.bottom, 24)
    }
}

// MARK: long strip

/// Pages one under another with no gap, the next chapter joined on as the end nears. Pinch to zoom the
/// width (it snaps back to the screen width below 1×); when zoomed, drag sideways too.
private struct StripReader: View {
    let model: ReaderModel
    let close: () -> Void
    @State private var position = ScrollPosition()
    @State private var width = UIScreen.main.bounds.width
    @State private var contentY: CGFloat = 0
    @State private var live: CGFloat = 1

    /// Every row of every chapter laid out so far, each with an id the scroll position can name.
    private struct Item: Identifiable {
        enum Kind {
            case page(Int, String, [String: String])
            case end(Segment)
        }
        let id: String
        let chapter: String
        let kind: Kind
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
        ScrollView(model.zoom > 1 ? [.vertical, .horizontal] : [.vertical]) {
            LazyVStack(spacing: 0) {
                ForEach(items) { item in
                    switch item.kind {
                    case .page(let index, let url, let headers):
                        AkiImage(url: url, headers: headers, placeholder: 1.4, onAspect: { model.aspects[url] = $0 })
                            .frame(width: width * model.zoom)
                            .onGeometryChange(for: CGRect.self) { $0.frame(in: .scrollView) } action: { frame in
                                if frame.minY <= 1, frame.maxY > 1 {
                                    model.report(chapter: item.chapter, page: index, offset: min(max(-frame.minY / max(frame.height, 1), 0), 1))
                                }
                            }
                    case .end(let segment):
                        chapterEnd(segment)
                    }
                }
            }
            .scrollTargetLayout()
            .frame(width: width * model.zoom)
        }
        .scrollPosition($position)
        .onScrollGeometryChange(for: CGFloat.self) { $0.contentOffset.y } action: { _, y in contentY = y }
        .onGeometryChange(for: CGFloat.self) { $0.size.width } action: { width = $0 }
        .ignoresSafeArea()
        .scaleEffect(live)
        .simultaneousGesture(
            MagnifyGesture()
                .onChanged { live = min(max($0.magnification, 0.6), 3 / max(model.zoom, 1)) }
                .onEnded { value in
                    model.zoom = min(max(model.zoom * value.magnification, 1), 3)
                    live = 1
                }
        )
        .onTapGesture { model.chrome.toggle() }
        .task(id: model.restoreToken) { await restore() }
    }

    private func chapterEnd(_ segment: Segment) -> some View {
        let next = model.newer(than: segment.chapterID)
        return VStack(spacing: 10) {
            AkiText("End of \(model.chapter(segment.chapterID).map(model.label) ?? "chapter")", .eyebrow, tone: .onDark)
            if let next {
                AkiText(model.segments.contains { $0.chapterID == next.id } ? "Next chapter below" : "Next: \(model.label(next))", .caption, tone: .dim)
            } else {
                AkiText("You are up to date", .title, tone: .onDark)
                AkiButton("Back to the series", action: close).padding(.top, 6)
            }
        }
        .frame(width: width, height: next == nil ? 220 : 96)
        .onGeometryChange(for: CGFloat.self) { $0.frame(in: .scrollView).minY } action: { top in
            if top < UIScreen.main.bounds.height * 0.9 { model.finish(segment.chapterID) }
        }
    }

    /// Puts the strip back where it was: the page, and how far down it, once that page's shape is known.
    private func restore() async {
        guard let target = model.restore, let segment = model.segments.first(where: { $0.chapterID == target.chapter }) else { return }
        model.restoring = true
        defer { model.restoring = false }
        try? await Task.sleep(for: .milliseconds(120))
        position.scrollTo(id: "\(target.chapter)|\(target.page)", anchor: .top)
        model.restore = nil
        guard target.offset > 0.001, segment.pages.images.indices.contains(target.page) else { return }
        let url = segment.pages.images[target.page]
        for _ in 0..<40 where model.aspects[url] == nil { try? await Task.sleep(for: .milliseconds(100)) }
        try? await Task.sleep(for: .milliseconds(200))
        if let aspect = model.aspects[url] {
            position.scrollTo(y: contentY + CGFloat(target.offset) * (width * model.zoom / aspect))
            try? await Task.sleep(for: .milliseconds(250))
        }
    }
}

// MARK: pages

/// One page at a time, turned by swiping or by tapping the edges (the right edge goes on in left-to-right
/// reading, the left edge in right-to-left). Double tap or pinch to zoom.
private struct PagedReader: View {
    @Bindable var model: ReaderModel
    @Environment(AppModel.self) private var app
    let close: () -> Void

    var body: some View {
        let segment = model.currentSegment ?? model.segments[0]
        let headers = segment.headers
        let count = segment.pages.images.count
        let rtl = model.mode == .rtl
        TabView(selection: $model.pagedIndex) {
            ForEach(Array(segment.pages.images.enumerated()), id: \.offset) { index, url in
                AkiZoomPage(url: url, headers: headers, onTap: { tap($0, count: count, rtl: rtl) })
                    .ignoresSafeArea()
                    .tag(index)
            }
            endPage(segment).tag(count)
        }
        .tabViewStyle(.page(indexDisplayMode: .never))
        .environment(\.layoutDirection, rtl ? .rightToLeft : .leftToRight)
        .ignoresSafeArea()
        .onChange(of: model.pagedIndex) { _, index in
            if index >= count {
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
        if (0...count).contains(next) {
            withAnimation(.smooth(duration: 0.25)) { model.pagedIndex = next }
        }
    }

    private func endPage(_ segment: Segment) -> some View {
        VStack(spacing: 14) {
            AkiText("End of \(model.chapter(segment.chapterID).map(model.label) ?? "chapter")", .eyebrow, tone: .onDark)
            if let next = model.newer(than: segment.chapterID) {
                AkiButton("Next · \(model.label(next))", variant: .primary, size: .large) { Task { await model.open(next.id, page: 0, offset: 0) } }
            } else {
                AkiText("You are up to date", .title, tone: .onDark)
            }
            AkiButton("Back to the series", action: close)
        }
        .environment(\.layoutDirection, .leftToRight)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}
