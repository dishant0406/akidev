import SwiftUI

extension View {
    /// Pull down to run `action`. The system's small spinner does the showing, and a light tap of the haptic engine
    /// confirms the pull took; nothing else changes on the screen.
    func akiRefreshable(_ action: @escaping () async -> Void) -> some View {
        refreshable {
            UIImpactFeedbackGenerator(style: .light).impactOccurred()
            await action()
        }
    }
}
