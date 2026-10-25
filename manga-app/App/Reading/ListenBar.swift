import SwiftUI

/// The controls for reading aloud, over the page: sentence back, play or pause, sentence forward, speed, stop.
struct ListenBar: View {
    let listener: BookListener

    private static let rates: [(value: Double, label: String)] = [0.75, 1, 1.25, 1.5, 1.75, 2].map { ($0, "\($0.formatted())×") }

    var body: some View {
        AkiGlassGroup(spacing: 12) {
            HStack(spacing: 12) {
                AkiIconButton(symbol: "previous", label: "Previous sentence") { listener.skip(forward: false) }
                if listener.state == .loading {
                    AkiSpinner().frame(width: 44, height: 44).akiGlass(.circle, interactive: false)
                } else {
                    AkiIconButton(symbol: listener.state == .playing ? "pause" : "play", label: listener.state == .playing ? "Pause" : "Play", tint: Aki.primary) { listener.toggle() }
                }
                AkiIconButton(symbol: "next", label: "Next sentence") { listener.skip(forward: true) }
                AkiMenuPill(selection: Bindable(listener).rate, options: Self.rates)
                AkiIconButton(symbol: "close", label: "Stop reading aloud") { listener.stop() }
            }
        }
        .padding(.horizontal, 16)
        .akiBar()
    }
}
