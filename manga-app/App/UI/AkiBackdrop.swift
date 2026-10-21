import SwiftUI

/// The page behind every screen: white with a soft yellow glow, so the glass above has something to refract.
struct AkiBackdrop: View {
    var body: some View {
        ZStack {
            Aki.background
            Circle().fill(Aki.accent).frame(width: 420).blur(radius: 100).offset(x: 170, y: -300)
            Circle().fill(Aki.accentSoft).frame(width: 360).blur(radius: 100).offset(x: -190, y: 320)
        }
        .ignoresSafeArea()
    }
}
