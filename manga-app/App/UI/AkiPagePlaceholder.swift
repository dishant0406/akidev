import SwiftUI

/// What stands in for a reader page until its picture arrives: a skeleton shaped like a page (a speech bubble, a panel,
/// two lines of lettering) with a soft light sweeping across it. A failed page says so and can be tapped to try again.
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
                skeleton
                shimmer
            }
        }
        .contentShape(Rectangle())
        .onTapGesture { if failed { retry() } }
        .accessibilityLabel(failed ? "Page failed to load. Tap to try again." : "Loading page")
    }

    /// Blocks laid out in proportion to whatever room the page is given, so a tall strip page and a screen-sized one
    /// both read as a page.
    private var skeleton: some View {
        GeometryReader { geometry in
            let w = geometry.size.width, h = geometry.size.height
            ZStack {
                block(width: w * 0.62, height: min(h * 0.13, 150), radius: min(w, h) * 0.08).position(x: w * 0.5, y: h * 0.14)
                block(width: w * 0.88, height: h * 0.46, radius: 8).position(x: w * 0.5, y: h * 0.52)
                block(width: w * 0.5, height: 10, radius: 5).position(x: w * 0.3, y: h * 0.84)
                block(width: w * 0.34, height: 10, radius: 5).position(x: w * 0.23, y: h * 0.88)
            }
        }
        .allowsHitTesting(false)
    }

    private func block(width: CGFloat, height: CGFloat, radius: CGFloat) -> some View {
        RoundedRectangle(cornerRadius: radius, style: .continuous)
            .fill(Aki.onMedia.opacity(0.06))
            .frame(width: width, height: height)
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
