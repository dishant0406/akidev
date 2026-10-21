import SwiftUI

/// The Akidev palette, as in `design/DESIGN_PLAN.md` and `desktop/app/src/design/tokens.rs`:
/// one warm ink canvas, amber only for state (progress, active, focus).
enum Aki {
    static let canvas = Color(red: 0.047, green: 0.043, blue: 0.039)      // 0c0b0a
    static let surface = Color(red: 0.082, green: 0.078, blue: 0.075)     // 151413
    static let raised = Color(red: 0.118, green: 0.110, blue: 0.102)      // 1e1c1a
    static let line = Color(red: 0.165, green: 0.157, blue: 0.145)        // 2a2825
    static let text = Color(red: 0.945, green: 0.929, blue: 0.902)        // f1ede6
    static let dim = Color(red: 0.639, green: 0.616, blue: 0.573)         // a39d92
    static let faint = Color(red: 0.459, green: 0.435, blue: 0.400)       // 756f66
    static let accent = Color(red: 0.902, green: 0.706, blue: 0.314)      // e6b450
    static let danger = Color(red: 0.910, green: 0.471, blue: 0.431)      // e8786e

    /// Radius scale: badge, button, panel, popup.
    static let radius: [CGFloat] = [4, 8, 12, 16]

    /// Titles use a serif; the interface keeps the system face.
    static func title(_ size: CGFloat) -> Font { .system(size: size, weight: .semibold, design: .serif) }
}

/// Tiny tracked capitals for metadata.
struct Eyebrow: View {
    let text: String
    init(_ text: String) { self.text = text }
    var body: some View {
        Text(text.uppercased()).font(.caption2.weight(.semibold)).tracking(1.4).foregroundStyle(Aki.faint)
    }
}

/// An empty or error screen: a serif line, a quiet line, and a next step that can help.
struct StateCard: View {
    let title: String
    let message: String
    var action: (label: String, run: () -> Void)?
    var body: some View {
        VStack(spacing: 10) {
            Text(title).font(Aki.title(24)).foregroundStyle(Aki.text).multilineTextAlignment(.center)
            Text(message).font(.subheadline).foregroundStyle(Aki.dim).multilineTextAlignment(.center)
            if let action {
                Button(action.label, action: action.run).buttonStyle(.glass).padding(.top, 8)
            }
        }
        .padding(32)
        .frame(maxWidth: 420)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}
