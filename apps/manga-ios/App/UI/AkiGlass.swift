import SwiftUI

enum AkiShape {
    case capsule, circle
    case card
    /// A rounded rectangle of the given corner radius.
    case rounded(CGFloat)
}

extension View {
    /// Liquid Glass on a control or a bar. A tint (the yellow) marks the current choice or the main action.
    func akiGlass(_ shape: AkiShape = .capsule, tint: Color? = nil, interactive: Bool = true) -> some View {
        modifier(AkiGlassModifier(shape: shape, tint: tint, interactive: interactive))
    }

    /// Glass only when `on`: the current tab or pill is a glass shape of its own, the others are not.
    @ViewBuilder
    func akiGlassIf(_ on: Bool, _ shape: AkiShape = .capsule, tint: Color? = nil) -> some View {
        if on { akiGlass(shape, tint: tint) } else { glassEffect(.identity, in: .capsule) }
    }
}

/// Glass, or a plain fill when the person asked to reduce transparency.
private struct AkiGlassModifier: ViewModifier {
    let shape: AkiShape
    let tint: Color?
    let interactive: Bool
    @Environment(\.accessibilityReduceTransparency) private var reduceTransparency

    func body(content: Content) -> some View {
        if reduceTransparency {
            switch shape {
            case .capsule: content.background(tint ?? Aki.secondary, in: .capsule)
            case .circle: content.background(tint ?? Aki.secondary, in: .circle)
            case .card: content.background(tint ?? Aki.surface, in: .rect(cornerRadius: Aki.cardRadius))
            case .rounded(let radius): content.background(tint ?? Aki.secondary, in: .rect(cornerRadius: radius))
            }
        } else {
            let base: Glass = tint.map { Glass.regular.tint($0) } ?? .regular
            let glass = interactive ? base.interactive() : base
            switch shape {
            case .capsule: content.glassEffect(glass, in: .capsule)
            case .circle: content.glassEffect(glass, in: .circle)
            case .card: content.glassEffect(glass, in: .rect(cornerRadius: Aki.cardRadius))
            case .rounded(let radius): content.glassEffect(glass, in: .rect(cornerRadius: radius))
            }
        }
    }
}

/// Groups glass shapes so they blend and do not sample each other.
struct AkiGlassGroup<Content: View>: View {
    var spacing: CGFloat = 8
    @ViewBuilder var content: () -> Content

    var body: some View {
        GlassEffectContainer(spacing: spacing, content: content)
    }
}
