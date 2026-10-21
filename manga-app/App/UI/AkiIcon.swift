import SwiftUI

/// A symbol. The size scales with Dynamic Type.
struct AkiIcon: View {
    let name: String
    var size: CGFloat = 18
    var tone: AkiTone = .primary
    @ScaledMetric private var scale: CGFloat = 1

    init(_ name: String, size: CGFloat = 18, tone: AkiTone = .primary) {
        self.name = name
        self.size = size
        self.tone = tone
    }

    var body: some View {
        Image(systemName: name)
            .font(.system(size: size * scale, weight: .semibold))
            .foregroundStyle(tone.color)
    }
}
