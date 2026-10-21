import SwiftUI

/// How things move. One critically damped spring for almost everything (it settles without bouncing and can be
/// interrupted and redirected from where it is); a little bounce only for motion that began with a flick. With
/// Reduce Motion on, every move becomes a short cross-fade instead.
enum AkiMotion {
    static func settle(_ reduceMotion: Bool) -> Animation {
        reduceMotion ? .easeInOut(duration: 0.2) : .spring(duration: 0.35, bounce: 0)
    }

    static func quick(_ reduceMotion: Bool) -> Animation {
        reduceMotion ? .easeInOut(duration: 0.15) : .spring(duration: 0.25, bounce: 0)
    }
}

/// Animates changes of `value` with the app's spring (or a cross-fade under Reduce Motion).
private struct AkiAnimation<Value: Equatable>: ViewModifier {
    let quick: Bool
    let value: Value
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    func body(content: Content) -> some View {
        content.animation(quick ? AkiMotion.quick(reduceMotion) : AkiMotion.settle(reduceMotion), value: value)
    }
}

/// Feedback the instant a press begins, not when it ends: the content gives a little under the finger.
struct AkiPressStyle: ButtonStyle {
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .scaleEffect(configuration.isPressed && !reduceMotion ? 0.97 : 1)
            .opacity(configuration.isPressed ? 0.85 : 1)
            .animation(AkiMotion.quick(reduceMotion), value: configuration.isPressed)
    }
}

extension View {
    func akiAnimation<Value: Equatable>(quick: Bool = false, value: Value) -> some View {
        modifier(AkiAnimation(quick: quick, value: value))
    }

    /// A tap of the haptic engine when `trigger` changes: a choice made, a save, a download done.
    func akiHaptic<Trigger: Equatable>(_ feedback: SensoryFeedback = .selection, trigger: Trigger) -> some View {
        sensoryFeedback(feedback, trigger: trigger)
    }
}
