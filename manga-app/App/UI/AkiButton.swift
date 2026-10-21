import SwiftUI

enum AkiButtonVariant {
    /// The main action: yellow glass.
    case primary
    case glass
    case ghost
    case destructive
}

enum AkiButtonSize {
    case small, regular, large

    var height: CGFloat {
        switch self {
        case .small: 38
        case .regular: 46
        case .large: 54
        }
    }
}

struct AkiButton: View {
    let title: String
    var icon: String?
    var variant: AkiButtonVariant = .glass
    var size: AkiButtonSize = .regular
    var fullWidth = false
    let action: () -> Void

    init(_ title: String, icon: String? = nil, variant: AkiButtonVariant = .glass, size: AkiButtonSize = .regular, fullWidth: Bool = false, action: @escaping () -> Void) {
        self.title = title
        self.icon = icon
        self.variant = variant
        self.size = size
        self.fullWidth = fullWidth
        self.action = action
    }

    var body: some View {
        Button(action: action) {
            HStack(spacing: 8) {
                if let icon { AkiIcon(icon, size: 15, tone: tone) }
                AkiText(title, .bodyStrong, tone: tone, lines: 1)
            }
            .padding(.horizontal, 20)
            .frame(maxWidth: fullWidth ? .infinity : nil)
            .frame(height: size.height)
            .contentShape(Capsule())
        }
        .buttonStyle(.plain)
        .akiGlassIf(variant != .ghost, tint: variant == .primary ? Aki.yellow : nil)
    }

    private var tone: AkiTone {
        switch variant {
        case .primary: .onYellow
        case .destructive: .danger
        case .glass, .ghost: .primary
        }
    }
}
