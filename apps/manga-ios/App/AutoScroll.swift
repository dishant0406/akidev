import QuartzCore
import UIKit

/// Calls `tick` with the seconds since the last call once per screen refresh while it is started: up to 120 times a second
/// on an iPhone with a ProMotion screen.
@MainActor
final class FrameClock {
    var tick: (CFTimeInterval) -> Void = { _ in }
    private var link: CADisplayLink?
    private var last: CFTimeInterval = 0

    func start() {
        guard link == nil else { return }
        last = CACurrentMediaTime()
        let target = Target(self)
        let link = CADisplayLink(target: target, selector: #selector(Target.fire))
        link.preferredFrameRateRange = CAFrameRateRange(minimum: 60, maximum: 120, preferred: 120)
        link.add(to: .main, forMode: .common)
        target.link = link
        self.link = link
    }

    func stop() {
        link?.invalidate()
        link = nil
    }

    private func fire() {
        let now = CACurrentMediaTime()
        // A long stall (a page decoding) must not become one big jump.
        tick(min(now - last, 0.1))
        last = now
    }

    /// A display link keeps its target alive until it is stopped; this keeps the clock from being kept alive too, and stops
    /// the link if the clock is gone.
    @MainActor
    private final class Target: NSObject {
        weak var clock: FrameClock?
        weak var link: CADisplayLink?
        init(_ clock: FrameClock) { self.clock = clock }

        @objc func fire() {
            if let clock { clock.fire() } else { link?.invalidate() }
        }
    }
}

/// How fast the strip scrolls by itself, in points a second.
enum AutoScrollSpeed: String, CaseIterable {
    case slow, normal, fast, rapid
    var title: String { rawValue.capitalized }
    var points: CGFloat {
        switch self {
        case .slow: 45
        case .normal: 90
        case .fast: 170
        case .rapid: 300
        }
    }
}
