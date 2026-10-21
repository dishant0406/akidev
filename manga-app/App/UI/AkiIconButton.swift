import SwiftUI

/// A round glass button with one symbol: back, save, refresh.
struct AkiIconButton: View {
    let symbol: String
    let label: String
    var tint: Color?
    /// Over a picture the symbol is white.
    var onImage = false
    var size: CGFloat = 44
    let action: () -> Void

    var body: some View {
        Button(action: action) { face }
            .buttonStyle(.plain)
            .accessibilityLabel(label)
            .akiGlass(.circle, tint: tint)
    }

    /// The round face alone, for a menu that supplies its own button.
    var face: some View {
        AkiIcon(symbol, size: size * 0.38, tone: tint != nil ? .onYellow : (onImage ? .onImage : .primary))
            .frame(width: size, height: size)
            .contentShape(Circle())
    }
}
