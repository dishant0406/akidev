import SwiftUI

extension View {
    /// Pull down to run `action`, with Akidev's own indicator instead of the system spinner: a glass bead drops in as you
    /// pull, a yellow ring fills as you near the line, it gives a tap when it is armed, spins while `action` runs, and
    /// pops a tick when it is done. `inset` is the space under the status bar when the scroll view runs beneath it.
    func akiRefreshable(inset: CGFloat = 0, _ action: @escaping () async -> Void) -> some View {
        modifier(AkiPullToRefresh(inset: inset, action: action))
    }
}

private struct AkiPullToRefresh: ViewModifier {
    let inset: CGFloat
    let action: () async -> Void
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private enum Phase { case idle, loading, done }
    @State private var pull: CGFloat = 0
    @State private var phase = Phase.idle
    @State private var armed = false
    /// Whether a finger is on the page: once it lifts, the page springs back and the pull falls to nothing, so what was
    /// decided while the finger was down is what counts.
    @State private var held = false
    private let line: CGFloat = 78

    func body(content: Content) -> some View {
        content
            .scrollBounceBehavior(.always, axes: .vertical)
            .onScrollGeometryChange(for: CGFloat.self) { max(-($0.contentOffset.y + $0.contentInsets.top), 0) } action: { _, new in
                pull = new
                if phase == .idle, held { armed = new >= line }
            }
            .onScrollPhaseChange { old, new in
                held = new == .interacting
                // Letting go past the line starts the refresh.
                if old == .interacting, new != .interacting {
                    if armed, phase == .idle { start() } else { armed = false }
                }
            }
            .overlay(alignment: .top) { bead.padding(.top, inset).allowsHitTesting(false) }
            .sensoryFeedback(.impact(weight: .light), trigger: armed) { _, isArmed in isArmed }
            .sensoryFeedback(.success, trigger: phase) { _, new in new == .done }
    }

    private func start() {
        armed = false
        withAnimation(AkiMotion.settle(reduceMotion)) { phase = .loading }
        Task {
            await action()
            withAnimation(AkiMotion.settle(reduceMotion)) { phase = .done }
            try? await Task.sleep(for: .milliseconds(700))
            withAnimation(AkiMotion.settle(reduceMotion)) { phase = .idle }
        }
    }

    // MARK: the bead

    private var progress: CGFloat { phase == .idle ? min(pull / line, 1) : 1 }

    /// Where the bead sits below the top: dragged along with the pull (slower than the finger), then held in place.
    private var drop: CGFloat {
        switch phase {
        case .idle: min(pull * 0.5, 52) - 44
        case .loading, .done: 10
        }
    }

    private var bead: some View {
        ZStack {
            Circle().stroke(Aki.secondary, lineWidth: 3)
            ring
            if phase == .done {
                AkiIcon("tick", size: 17, tone: .text).transition(.scale(scale: 0.4).combined(with: .opacity))
            } else if phase == .idle {
                AkiIcon("refresh", size: 15, tone: .muted).rotationEffect(.degrees(progress * 180)).opacity(0.4 + 0.6 * progress)
            }
        }
        .padding(9)
        .frame(width: 46, height: 46)
        .akiGlass(.circle, interactive: false)
        .scaleEffect(phase == .idle ? 0.55 + 0.45 * progress : 1)
        .opacity(phase == .idle ? min(pull / 36, 1) : 1)
        .offset(y: drop)
        .animation(phase == .idle ? nil : AkiMotion.settle(reduceMotion), value: phase)
    }

    @ViewBuilder private var ring: some View {
        switch phase {
        case .idle:
            Circle().trim(from: 0, to: progress).stroke(Aki.primary, style: StrokeStyle(lineWidth: 3, lineCap: .round)).rotationEffect(.degrees(-90))
        case .loading:
            // A quarter of a ring, going round: the clock gives the angle, so nothing has to be started or stopped.
            TimelineView(.animation(paused: reduceMotion)) { timeline in
                Circle().trim(from: 0, to: 0.28).stroke(Aki.primary, style: StrokeStyle(lineWidth: 3, lineCap: .round))
                    .rotationEffect(.degrees(reduceMotion ? 0 : timeline.date.timeIntervalSinceReferenceDate * 400))
            }
        case .done:
            Circle().stroke(Aki.primary, lineWidth: 3)
        }
    }
}
