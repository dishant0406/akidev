import SwiftUI

/// An icon from the Solar set (see scripts/fetch-icons.py): `name` is the outline, `name-solid` the filled one.
/// The size scales with Dynamic Type.
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
        Image("solar-\(name)")
            .renderingMode(.template)
            .resizable()
            .scaledToFit()
            .frame(width: size * scale, height: size * scale)
            .foregroundStyle(tone.color)
    }
}
