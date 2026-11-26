import SwiftUI

/// The Akidev mark, the same picture as the app icon, at any size.
struct AkiLogo: View {
    var size: CGFloat = 112

    var body: some View {
        Image("logo")
            .resizable()
            .scaledToFit()
            .frame(width: size, height: size)
            // iOS's own icon corner is 22.37% of the side.
            .clipShape(RoundedRectangle(cornerRadius: size * 0.2237, style: .continuous))
            .accessibilityHidden(true)
    }
}
