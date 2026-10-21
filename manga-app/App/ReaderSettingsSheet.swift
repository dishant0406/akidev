import SwiftUI

/// What can be changed while reading: how pages are laid out, the backdrop, which way the screen turns.
struct ReaderSettingsSheet: View {
    let model: ReaderModel
    @Environment(AppModel.self) private var app

    var body: some View {
        @Bindable var prefs = app.prefs
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                AkiText("Reader", .title)
                AkiCard(padding: 16) {
                    VStack(spacing: 16) {
                        AkiSelect(title: "This series", selection: Binding(get: { model.mode }, set: { model.setMode($0) }), options: ReadingMode.allCases.map { ($0, $0.title) })
                        AkiSelect(title: "Background", selection: $prefs.readerBackground, options: ReaderBackground.allCases.map { ($0, $0.title) })
                        AkiSelect(title: "Orientation", selection: $prefs.orientation, options: OrientationLock.allCases.map { ($0, $0.title) })
                        AkiSwitch(title: "Swap tap sides", subtitle: "In paged reading the left edge goes forward.", isOn: $prefs.invertTaps)
                        AkiSwitch(title: "Keep the screen on", isOn: Binding(get: { prefs.keepAwake }, set: { prefs.keepAwake = $0; UIApplication.shared.isIdleTimerDisabled = $0 }))
                    }
                }
            }
            .padding(24)
        }
    }
}
