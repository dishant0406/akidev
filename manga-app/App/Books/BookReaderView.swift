import AkiManga
import SwiftUI

/// The reader for a book: the page fills the screen; the bars are glass and come and go with a tap in the middle.
struct BookReaderView: View {
    @State var model: BookModel
    let close: () -> Void
    @Environment(\.colorScheme) private var scheme
    @Environment(\.scenePhase) private var phase
    @Environment(AppModel.self) private var app
    @State private var settings = false
    @State private var contents = false
    @State private var searching = false

    var body: some View {
        GeometryReader { proxy in
            ZStack {
                model.theme.paperColor
                Group {
                    if model.style.scroll {
                        BookScroller(paging: model, jump: model.jump)
                    } else {
                        BookPager(paging: model, jump: model.jump, still: model.style.ink, locked: model.selection != nil)
                    }
                }
                .saturation(model.style.ink ? 0 : 1)
                BookComfortOverlay(style: model.style)
                if model.place == nil { AkiOpeningLoader(label: model.info.title) }
            }
            .onAppear { model.systemDark = scheme == .dark; model.resize(proxy.size, safe: BookGeometry.safe) }
            .onChange(of: proxy.size) { _, size in model.resize(size, safe: BookGeometry.safe) }
            .onChange(of: scheme) { _, now in model.systemDark = now == .dark }
        }
        .ignoresSafeArea()
        .overlay(alignment: .top) { if model.chromeVisible { topBar.transition(.opacity) } }
        .overlay(alignment: .bottom) {
            VStack(spacing: 12) {
                if !model.trail.isEmpty { backToPage.transition(.opacity) }
                if let listener = model.listener { ListenBar(listener: listener).transition(.opacity) }
                if model.chromeVisible { bottomBar.transition(.opacity) }
            }
            .padding(.bottom, 24)
        }
        // The bars take the colours of the page they are over.
        .environment(\.colorScheme, model.theme.isDark ? .dark : .light)
        .statusBarHidden(!model.chromeVisible)
        .persistentSystemOverlays(model.chromeVisible ? .automatic : .hidden)
        .akiAnimation(quick: true, value: model.chromeVisible)
        .akiAnimation(quick: true, value: model.trail.count)
        .akiAnimation(quick: true, value: model.listener != nil)
        .akiSheet(isPresented: $settings) { BookSettingsSheet(model: model) }
        .akiSheet(isPresented: $contents, full: true) { BookContentsSheet(model: model) { contents = false } }
        .akiSheet(isPresented: $searching, full: true) { BookSearchSheet(model: model) { searching = false } }
        .akiSheet(item: $model.noteDraft) { draft in BookNoteSheet(model: model, draft: draft) { model.noteDraft = nil } }
        .akiSheet(item: $model.footnote) { note in BookFootnoteSheet(model: model, note: note) { model.footnote = nil } }
        .akiAlert(model.failure?.title ?? "", message: model.failure?.message, isPresented: Binding(get: { model.failure != nil }, set: { if !$0 { model.failure = nil } }))
        .akiConfirm("Open this link?", message: model.externalLink?.absoluteString ?? "", confirm: "Open", isPresented: Binding(get: { model.externalLink != nil }, set: { if !$0 { model.externalLink = nil } })) {
            if let url = model.externalLink { UIApplication.shared.open(url) }
        }
        .onAppear { UIApplication.shared.isIdleTimerDisabled = app.prefs.keepAwake; BookComfort.apply(model.style.brightness) }
        .onChange(of: model.style.brightness) { _, now in BookComfort.apply(now) }
        // The phone's brightness is the phone's: it is given back when the app is left and taken again when it returns.
        .onChange(of: phase) { _, now in if now == .active { BookComfort.apply(model.style.brightness) } else { BookComfort.restore() } }
        .onDisappear { model.stopListening(); UIApplication.shared.isIdleTimerDisabled = false; BookComfort.restore() }
    }

    private var topBar: some View {
        AkiGlassGroup(spacing: 12) {
            HStack(spacing: 12) {
                AkiIconButton(symbol: "back", label: "Back", action: close)
                AkiTap(action: { contents = true }) {
                    VStack(alignment: .leading, spacing: 0) {
                        AkiText(model.info.title, .captionStrong, lines: 1)
                        if let author = model.info.authors.first { AkiText(author, .caption, tone: .muted, lines: 1) }
                    }
                    .padding(.horizontal, 16).padding(.vertical, 8)
                    .akiGlass(.capsule, interactive: false)
                }
                .accessibilityLabel("Contents")
                Spacer(minLength: 0)
                if model.listener == nil, let voice = app.readAloud.current {
                    AkiIconButton(symbol: "headphones", label: "Read aloud") { model.listen(voice: voice, prefs: app.prefs) }
                }
                AkiIconButton(symbol: "search", label: "Search in this book") { searching = true }
                AkiIconButton(symbol: "sliders", label: "Type and theme") { settings = true }
            }
        }
        .padding(.horizontal, 16).padding(.top, 8)
    }

    private var bottomBar: some View {
        AkiGlassGroup(spacing: 12) {
            HStack(spacing: 12) {
                AkiIconButton(symbol: "list", label: "Contents") { contents = true }
                HStack(spacing: 12) {
                    AkiSlider(value: $model.progress, onEnd: { model.go(toProgress: model.progress) })
                    AkiText("\(Int((model.progress * 100).rounded()))%", .caption, lines: 1)
                        .monospacedDigit()
                        .frame(width: 40, alignment: .trailing)
                }
                .padding(.horizontal, 18).frame(height: 44)
                .akiGlass(.capsule, interactive: false)
            }
        }
        .padding(.horizontal, 16)
    }

    private var backToPage: some View {
        AkiButton("Back to where you were", icon: "back", variant: .primary, size: .small) { model.goBack() }
    }
}
