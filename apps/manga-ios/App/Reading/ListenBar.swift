import SwiftUI

/// The controls for reading aloud, over the page: sentence back, play or pause, sentence forward, speed, stop.
struct ListenBar: View {
    let listener: BookListener

    private static let rates: [(value: Double, label: String)] = [0.75, 1, 1.25, 1.5, 1.75, 2].map { ($0, "\($0.formatted())×") }

    var body: some View {
        AkiGlassGroup(spacing: 12) {
            HStack(spacing: 12) {
                AkiIconButton(symbol: "previous", label: Copy.Listen.previousSentence) { listener.skip(forward: false) }
                if listener.state == .loading {
                    AkiSpinner().frame(width: 44, height: 44).akiGlass(.circle, interactive: false).accessibilityLabel(Copy.Listen.gettingReadyToReadAloud)
                } else {
                    AkiIconButton(symbol: listener.state == .playing ? "pause" : "play", label: listener.state == .playing ? Copy.Listen.pause : Copy.Listen.play, tint: Aki.primary) { listener.toggle() }
                }
                AkiIconButton(symbol: "next", label: Copy.Listen.nextSentence) { listener.skip(forward: true) }
                AkiMenuPill(selection: Bindable(listener).rate, options: Self.rates)
                    .accessibilityHint(Copy.Listen.readingSpeed)
                AkiIconButton(symbol: "close", label: Copy.Listen.stopReadingAloud) { listener.stop() }
            }
        }
        .padding(.horizontal, 16)
        .akiBar()
    }
}
