import SwiftUI

/// An icon from the Solar set (see scripts/fetch-icons.py): `name` is the outline, `name-solid` the filled one.
/// The size follows Dynamic Type, up to 1.4x so bars and buttons keep their shape.
struct AkiIcon: View {
    let name: String
    var size: CGFloat = 18
    var tone: AkiTone = .text
    @ScaledMetric private var scale: CGFloat = 1

    init(_ name: String, size: CGFloat = 18, tone: AkiTone = .text) {
        self.name = name
        self.size = size
        self.tone = tone
    }

    var body: some View {
        Image("solar-\(name)")
            .renderingMode(.template)
            .resizable()
            .scaledToFit()
            .frame(width: size * min(scale, 1.4), height: size * min(scale, 1.4))
            .foregroundStyle(tone.color)
    }
}
