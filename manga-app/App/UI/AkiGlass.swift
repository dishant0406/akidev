import SwiftUI

enum AkiShape {
    case capsule, circle
    case card
}

extension View {
    /// Liquid Glass on a control or a bar. A tint (the yellow) marks the current choice or the main action.
    @ViewBuilder
    func akiGlass(_ shape: AkiShape = .capsule, tint: Color? = nil, interactive: Bool = true) -> some View {
        let base: Glass = tint.map { Glass.regular.tint($0) } ?? .regular
        let glass = interactive ? base.interactive() : base
        switch shape {
        case .capsule: glassEffect(glass, in: .capsule)
        case .circle: glassEffect(glass, in: .circle)
        case .card: glassEffect(glass, in: .rect(cornerRadius: Aki.cardRadius))
        }
    }

    /// Glass only when `on`: the current tab or pill is a glass shape of its own, the others are not.
    @ViewBuilder
    func akiGlassIf(_ on: Bool, _ shape: AkiShape = .capsule, tint: Color? = nil) -> some View {
        if on { akiGlass(shape, tint: tint) } else { glassEffect(.identity, in: .capsule) }
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
