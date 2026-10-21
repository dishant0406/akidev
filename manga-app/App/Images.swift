import SwiftUI
import ImageIO

/// Loads covers and pages: a request with the headers a source needs, decoded off the main thread at
/// the size it will be shown (a 12,000 px page is never held at full size).
actor ImageLoader {
    static let shared = ImageLoader()
    private let cache = NSCache<NSString, UIImage>()
    private var inflight: [String: Task<UIImage?, Never>] = [:]

    init() {
        cache.totalCostLimit = 160 * 1024 * 1024
    }

    func image(_ url: URL, headers: [String: String], widthPx: CGFloat) async -> UIImage? {
        let key = "\(url.absoluteString)@\(Int(widthPx))"
        if let hit = cache.object(forKey: key as NSString) { return hit }
        if let running = inflight[key] { return await running.value }
        let task = Task<UIImage?, Never> {
            var request = URLRequest(url: url)
            for (name, value) in headers { request.setValue(value, forHTTPHeaderField: name) }
            guard let (data, response) = try? await URLSession.shared.data(for: request),
                  (response as? HTTPURLResponse).map({ (200..<300).contains($0.statusCode) }) ?? true else { return nil }
            return Self.downsample(data, widthPx: widthPx)
        }
        inflight[key] = task
        let image = await task.value
        inflight[key] = nil
        if let image {
            cache.setObject(image, forKey: key as NSString, cost: Int(image.size.width * image.size.height * image.scale * image.scale * 4))
        }
        return image
    }

    static func downsample(_ data: Data, widthPx: CGFloat) -> UIImage? {
        guard let source = CGImageSourceCreateWithData(data as CFData, nil),
              let properties = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any],
              let width = properties[kCGImagePropertyPixelWidth] as? CGFloat,
              let height = properties[kCGImagePropertyPixelHeight] as? CGFloat, width > 0 else { return nil }
        let scale = min(1, widthPx / width)
        let options: [CFString: Any] = [
            kCGImageSourceCreateThumbnailFromImageAlways: true,
            kCGImageSourceShouldCacheImmediately: true,
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceThumbnailMaxPixelSize: max(width, height) * scale,
        ]
        guard let image = CGImageSourceCreateThumbnailAtIndex(source, 0, options as CFDictionary) else { return nil }
        return UIImage(cgImage: image)
    }
}

/// An image that fills its width and keeps its own shape, or a placeholder of `placeholder` height
/// (as a multiple of the width) until it has loaded.
struct RemoteImage: View {
    let url: String?
    var headers: [String: String] = [:]
    var placeholder: CGFloat = 1.5
    var fill = false
    /// Told the width-to-height ratio once the image is known (the reader needs it to restore a position).
    var onAspect: ((CGFloat) -> Void)?
    @State private var image: UIImage?
    @State private var failed = false
    @Environment(\.displayScale) private var scale

    var body: some View {
        GeometryReader { geometry in
            ZStack {
                Aki.surface
                if let image {
                    if fill {
                        Image(uiImage: image).resizable().scaledToFill().frame(width: geometry.size.width, height: geometry.size.height).clipped()
                    } else {
                        Image(uiImage: image).resizable().scaledToFit().frame(width: geometry.size.width)
                    }
                } else if failed {
                    Image(systemName: "photo").foregroundStyle(Aki.faint)
                }
            }
            .frame(width: geometry.size.width, height: geometry.size.height)
            .task(id: url) { await load(widthPoints: geometry.size.width) }
        }
        .aspectRatio(fill ? 1 / placeholder : (image.map { $0.size.width / $0.size.height } ?? (1 / placeholder)), contentMode: .fit)
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
