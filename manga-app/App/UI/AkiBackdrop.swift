import SwiftUI

/// The page behind every screen: white with a soft yellow glow, so the glass above has something to refract.
struct AkiBackdrop: View {
    var body: some View {
        ZStack {
            Aki.canvas
            Circle().fill(Aki.yellow.opacity(0.30)).frame(width: 420).blur(radius: 100).offset(x: 170, y: -300)
            Circle().fill(Aki.yellow.opacity(0.16)).frame(width: 360).blur(radius: 100).offset(x: -190, y: 320)
        }
        .ignoresSafeArea()
    }
}
