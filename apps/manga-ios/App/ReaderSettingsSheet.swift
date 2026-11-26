import SwiftUI

/// What can be changed while reading: how pages are laid out, the backdrop, which way the screen turns. Each choice is a
/// row of pills, one tap, nothing hidden in a menu.
struct ReaderSettingsSheet: View {
    let model: ReaderModel
    @Environment(AppModel.self) private var app
    @State private var coverSet: Bool?

    var body: some View {
        @Bindable var prefs = app.prefs
        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                AkiText(Copy.Reader.readerSettings, .title).accessibilityAddTraits(.isHeader)
                choice(Copy.Reader.readingModeForSeries) {
                    AkiTabs(items: ReadingMode.allCases.map { ($0, $0.shortTitle) }, selection: Binding(get: { model.mode }, set: { model.setMode($0) }))
                }
                choice(Copy.Reader.autoScrollSpeed) {
                    AkiTabs(items: AutoScrollSpeed.allCases.map { ($0, $0.title) }, selection: $prefs.autoScrollSpeed)
                }
                choice(Copy.Reader.background) {
                    AkiTabs(items: ReaderBackground.allCases.map { ($0, $0.title) }, selection: $prefs.readerBackground)
                }
                // An iPad turns with the person, and with Split View no app may turn it: only a phone has this choice.
                if UIDevice.current.userInterfaceIdiom == .phone {
                    choice(Copy.Reader.orientation) {
                        AkiTabs(items: OrientationLock.allCases.map { ($0, $0.title) }, selection: $prefs.orientation)
                    }
                }
                AkiCard(padding: 16) {
                    VStack(spacing: 16) {
                        AkiSwitch(title: Copy.Reader.swapTapSides, subtitle: Copy.Reader.swapTapSidesHint, isOn: $prefs.invertTaps)
                        AkiSwitch(title: Copy.Reader.keepScreenOn, isOn: Binding(get: { prefs.keepAwake }, set: { prefs.keepAwake = $0; UIApplication.shared.isIdleTimerDisabled = $0 || model.autoScrolling }))
                    }
                }
                AkiButton(coverSet == true ? Copy.Reader.coverChanged : coverSet == false ? Copy.Reader.couldNotUseCover : Copy.Reader.useCoverPage, icon: "image", fullWidth: true) {
                    Task {
                        coverSet = await model.useCurrentPageAsCover()
                        app.refresh()
                        akiAnnounce(coverSet == true ? Copy.Reader.coverChanged : Copy.Reader.couldNotUseCover)
                    }
                }
            }
            .padding(24)
        }
    }

    private func choice<Content: View>(_ title: String, @ViewBuilder _ content: () -> Content) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            AkiText(title, .captionStrong, tone: .muted)
            content()
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel(title)
    }
}
