import SwiftUI

/// A picture from an address, with the headers its source needs, decoded at the size it is shown.
struct AkiImage: View {
    let url: String?
    var headers: [String: String] = [:]
    /// Height as a multiple of width while it loads (and for ever when `fill`).
    var placeholder: CGFloat = 1.5
    /// Fill a box of that shape and crop, instead of keeping the picture's own shape.
    var fill = false
    /// Fill whatever space it is given.
    var free = false
    /// Told the width-to-height ratio once known (the reader needs it to restore a position).
    var onAspect: ((CGFloat) -> Void)?
    @State private var image: UIImage?
    @State private var failed = false
    @Environment(\.displayScale) private var scale

    var body: some View {
        GeometryReader { geometry in
            ZStack {
                Aki.chip
                if let image {
                    if fill || free {
                        Image(uiImage: image).resizable().scaledToFill().frame(width: geometry.size.width, height: geometry.size.height).clipped()
                    } else {
                        Image(uiImage: image).resizable().scaledToFit().frame(width: geometry.size.width)
                    }
                } else if failed {
                    AkiIcon("image", tone: .dim)
                }
            }
            .frame(width: geometry.size.width, height: geometry.size.height)
            .task(id: url) { await load(widthPoints: geometry.size.width) }
        }
        .modifier(KeepsShape(ratio: free ? nil : (fill ? 1 / placeholder : (image.map { $0.size.width / $0.size.height } ?? (1 / placeholder)))))
    }

    private func load(widthPoints: CGFloat) async {
        guard let url, let parsed = URL(string: url) else { failed = true; return }
        failed = false
        let loaded = await ImageLoader.shared.image(parsed, headers: headers, widthPx: max(widthPoints, 1) * scale)
        if let loaded {
            image = loaded
            onAspect?(loaded.size.width / loaded.size.height)
        } else {
            failed = true
        }
    }
}

private struct KeepsShape: ViewModifier {
    let ratio: CGFloat?
    func body(content: Content) -> some View {
        if let ratio { content.aspectRatio(ratio, contentMode: .fit) } else { content }
    }
}
