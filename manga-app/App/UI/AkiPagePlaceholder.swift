import SwiftUI

/// What stands in for a reader page until its picture arrives: a dark panel with a soft light sweeping across it, the
/// page number, and a small ring. A failed page says so and can be tapped to try again.
struct AkiPagePlaceholder: View {
    var number: Int?
    var failed = false
    var retry: () -> Void = {}
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        ZStack {
            Aki.stageAlt
            if !failed { shimmer }
            VStack(spacing: 10) {
                if failed {
                    AkiIcon("refresh", size: 22, tone: .onMedia)
                    AkiText("Tap to try again", .captionStrong, tone: .onMedia)
                } else {
                    if let number { AkiText("\(number)", .heading, tone: .onMedia).opacity(0.5) }
                    ring
                }
            }
        }
        .contentShape(Rectangle())
        .onTapGesture { if failed { retry() } }
        .accessibilityLabel(failed ? "Page failed to load. Tap to try again." : "Loading page")
    }

    /// A band of light crossing left to right, slow and faint; the clock gives its place so nothing is started or stopped.
    private var shimmer: some View {
        TimelineView(.animation(paused: reduceMotion)) { timeline in
            GeometryReader { geometry in
                let phase = reduceMotion ? 0.5 : (timeline.date.timeIntervalSinceReferenceDate / 1.6).truncatingRemainder(dividingBy: 1)
                LinearGradient(colors: [.clear, Aki.onMedia.opacity(0.07), .clear], startPoint: .leading, endPoint: .trailing)
                    .frame(width: geometry.size.width * 0.7)
                    .offset(x: (phase * 1.7 - 0.7) * geometry.size.width)
            }
        }
        .allowsHitTesting(false)
    }

    private var ring: some View {
        TimelineView(.animation(paused: reduceMotion)) { timeline in
            Circle().trim(from: 0, to: 0.28)
                .stroke(Aki.primary, style: StrokeStyle(lineWidth: 2.5, lineCap: .round))
                .rotationEffect(.degrees(reduceMotion ? 0 : timeline.date.timeIntervalSinceReferenceDate * 360))
        }
        .frame(width: 24, height: 24)
        .opacity(0.9)
    }
}
