import AkiManga
import SwiftUI

/// The reader for a book: the page fills the screen; the bars are glass and come and go with a tap in the middle.
struct BookReaderView: View {
    @State var model: BookModel
    let close: () -> Void
    @Environment(\.colorScheme) private var scheme
    @Environment(AppModel.self) private var app
    @State private var settings = false
    @State private var contents = false

    var body: some View {
        GeometryReader { proxy in
            ZStack {
                model.theme.paperColor
                BookPager(paging: model, jump: model.jump)
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
        .akiSheet(isPresented: $settings) { BookSettingsSheet(model: model) }
        .akiSheet(isPresented: $contents, full: true) { BookContentsSheet(model: model) { contents = false } }
        .akiConfirm("Open this link?", message: model.externalLink?.absoluteString ?? "", confirm: "Open", isPresented: Binding(get: { model.externalLink != nil }, set: { if !$0 { model.externalLink = nil } })) {
            if let url = model.externalLink { UIApplication.shared.open(url) }
        }
        .onAppear { UIApplication.shared.isIdleTimerDisabled = app.prefs.keepAwake }
        .onDisappear { UIApplication.shared.isIdleTimerDisabled = false }
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
