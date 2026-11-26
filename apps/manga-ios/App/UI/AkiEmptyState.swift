import SwiftUI

/// An empty or error screen: a bold line, a quiet line, and a next step that can help.
struct AkiEmptyState: View {
    let title: String
    let message: String
    var action: (label: String, run: () -> Void)?
    var secondary: (label: String, run: () -> Void)?

    var body: some View {
        VStack(spacing: 10) {
            AkiText(title, .title, align: .center)
            AkiText(message, .body, tone: .muted, align: .center)
            if let action { AkiButton(action.label, variant: .primary, action: action.run).padding(.top, 8) }
            if let secondary { AkiButton(secondary.label, variant: .ghost, size: .small, action: secondary.run) }
        }
        .padding(28)
        .frame(maxWidth: 420)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}
