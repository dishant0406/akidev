import SwiftUI

/// The soft grey card that holds a row.
struct AkiCard<Content: View>: View {
    var padding: CGFloat = 12
    var dimmed = false
    @ViewBuilder var content: () -> Content

    var body: some View {
        content()
            .padding(padding)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(Aki.card, in: RoundedRectangle(cornerRadius: Aki.cardRadius, style: .continuous))
            .opacity(dimmed ? 0.62 : 1)
    }
}
