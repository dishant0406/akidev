import SwiftUI

/// What can be changed while reading: how pages are laid out, the backdrop, which way the screen turns. Each choice is a
/// row of pills, one tap, nothing hidden in a menu.
struct ReaderSettingsSheet: View {
    let model: ReaderModel
    @Environment(AppModel.self) private var app

    var body: some View {
        @Bindable var prefs = app.prefs
        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                AkiText("Reader", .title)
                choice("Reading mode, this series") {
                    AkiTabs(items: ReadingMode.allCases.map { ($0, $0.shortTitle) }, selection: Binding(get: { model.mode }, set: { model.setMode($0) }))
                }
                choice("Auto-scroll speed") {
                    AkiTabs(items: AutoScrollSpeed.allCases.map { ($0, $0.title) }, selection: $prefs.autoScrollSpeed)
                }
                choice("Background") {
                    AkiTabs(items: ReaderBackground.allCases.map { ($0, $0.title) }, selection: $prefs.readerBackground)
                }
                choice("Orientation") {
                    AkiTabs(items: OrientationLock.allCases.map { ($0, $0.title) }, selection: $prefs.orientation)
                }
                AkiCard(padding: 16) {
                    VStack(spacing: 16) {
                        AkiSwitch(title: "Swap tap sides", subtitle: "In paged reading the left edge goes forward.", isOn: $prefs.invertTaps)
                        AkiSwitch(title: "Keep the screen on", isOn: Binding(get: { prefs.keepAwake }, set: { prefs.keepAwake = $0; UIApplication.shared.isIdleTimerDisabled = $0 }))
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
    }
}
