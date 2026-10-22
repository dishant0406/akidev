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
    /// A reader page: the dark placeholder with its number and a way to retry, instead of the plain cover block.
    var pageNumber: Int?
    @State private var image: UIImage?
    @State private var failed = false
    /// Bumped to ask for the picture again.
    @State private var attempt = 0
    @Environment(\.displayScale) private var scale

    var body: some View {
        GeometryReader { geometry in
            ZStack {
                if let pageNumber, image == nil {
                    AkiPagePlaceholder(number: pageNumber + 1, failed: failed) { attempt += 1 }
                } else {
                    Aki.secondary
                }
                if let image {
                    Group {
                        if fill || free {
                            Image(uiImage: image).resizable().scaledToFill().frame(width: geometry.size.width, height: geometry.size.height).clipped()
                        } else {
                            Image(uiImage: image).resizable().scaledToFit().frame(width: geometry.size.width)
                        }
                    }
                    .transition(.opacity)
                } else if failed, pageNumber == nil {
                    AkiIcon("image", tone: .muted)
                }
            }
            .frame(width: geometry.size.width, height: geometry.size.height)
            // A picture filled into a box spills past it; clipping hides the spill but would still let it catch taps
            // meant for what is next to it (the search field above a featured cover).
            .clipped()
            .contentShape(Rectangle())
            .task(id: "\(url ?? "")|\(attempt)") { await load(widthPoints: geometry.size.width) }
        }
        // It comes in rather than popping in.
        .animation(.easeOut(duration: 0.25), value: image != nil)
        .modifier(KeepsShape(ratio: free ? nil : (fill ? 1 / placeholder : (image.map { $0.size.width / $0.size.height } ?? (1 / placeholder)))))
    }

    private func load(widthPoints: CGFloat) async {
        // No address at all is a cover the source does not have (its lists carry none): show the plain placeholder,
        // not the broken-image mark. A failed fetch of an address it did give is the broken case.
        guard let url, let parsed = URL(string: url) else { image = nil; failed = false; return }
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
