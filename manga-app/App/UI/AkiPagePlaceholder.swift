import SwiftUI

/// What stands in for a reader page until its picture arrives: a plain dark panel with a soft light sweeping across it and
/// the page number. A failed page says so and can be tapped to try again.
struct AkiPagePlaceholder: View {
    var number: Int?
    var failed = false
    var retry: () -> Void = {}
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        ZStack {
            Aki.stageAlt
            if failed {
                VStack(spacing: 10) {
                    AkiIcon("refresh", size: 22, tone: .onMedia)
                    AkiText("Tap to try again", .captionStrong, tone: .onMedia)
                }
            } else {
                shimmer
                if let number { AkiText("\(number)", .heading, tone: .onMedia).opacity(0.3) }
            }
        }
        .contentShape(Rectangle())
        .onTapGesture { if failed { retry() } }
        .accessibilityLabel(failed ? "Page failed to load. Tap to try again." : "Loading page")
    }

    /// A band of light crossing left to right, slow and faint, drawn 30 times a second (it is slow, and several can be on screen
    /// while the strip scrolls at the display's full rate); the clock gives its place so nothing is started or stopped.
    private var shimmer: some View {
        TimelineView(.animation(minimumInterval: 1 / 30, paused: reduceMotion)) { timeline in
            GeometryReader { geometry in
                let phase = reduceMotion ? 0.5 : (timeline.date.timeIntervalSinceReferenceDate / 1.6).truncatingRemainder(dividingBy: 1)
                LinearGradient(colors: [.clear, Aki.onMedia.opacity(0.08), .clear], startPoint: .leading, endPoint: .trailing)
                    .frame(width: geometry.size.width * 0.7)
                    .offset(x: (phase * 1.7 - 0.7) * geometry.size.width)
            }
        }
        .allowsHitTesting(false)
    }
}

/// What the reader shows while a chapter opens and finds the place you were at: a ring and the chapter's name, on the
/// reader's own background. Nothing else moves until the page is where it should be.
struct AkiOpeningLoader: View {
    var label: String?
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        VStack(spacing: 14) {
            TimelineView(.animation(paused: reduceMotion)) { timeline in
                Circle().trim(from: 0, to: 0.28)
                    .stroke(Aki.primary, style: StrokeStyle(lineWidth: 3, lineCap: .round))
                    .rotationEffect(.degrees(reduceMotion ? 0 : timeline.date.timeIntervalSinceReferenceDate * 360))
            }
            .frame(width: 30, height: 30)
            if let label { AkiText(label, .caption, tone: .onMedia).opacity(0.6) }
        }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel("Opening chapter")
    }
}
