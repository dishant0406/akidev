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

    func clear() { cache.removeAllObjects() }

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
