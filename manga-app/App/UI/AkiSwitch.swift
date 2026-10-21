import SwiftUI

/// A setting that is on or off, with a title and a line under it.
struct AkiSwitch: View {
    let title: String
    var subtitle: String?
    @Binding var isOn: Bool

    var body: some View {
        Toggle(isOn: $isOn) {
            VStack(alignment: .leading, spacing: 2) {
                AkiText(title, .bodyStrong)
                if let subtitle { AkiText(subtitle, .caption, tone: .dim) }
            }
        }
        .tint(Aki.yellow)
        .akiHaptic(trigger: isOn)
    }
}
