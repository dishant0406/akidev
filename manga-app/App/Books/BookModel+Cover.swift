import AkiManga
import SwiftUI

extension BookModel {
    /// The page being read, as it is drawn on screen (without the highlights and the foot line).
    private func pageImage() -> UIImage? {
        guard let place, place.page >= 0, let layout = layouts[place.chapter], screen != .zero else { return nil }
        let canvas = BookPageView(frame: CGRect(origin: .zero, size: screen))
        canvas.paging = self
        canvas.layout = layout
        canvas.page = min(place.page, layout.count - 1)
        canvas.theme = theme
        canvas.insets = pageInsets
        canvas.scrolling = scrolling
        if scrolling { canvas.insets.top = layout.leading(of: canvas.page) }
        return UIGraphicsImageRenderer(size: screen).image { canvas.layer.render(in: $0.cgContext) }
    }

    /// The picture a cover or picture page is made of, whole: drawing the page would show only paper until it is decoded.
    private func pictureOfPage() async -> UIImage? {
        guard let place, let path = layouts[place.chapter]?.coverPicture else { return nil }
        let reader = reader
        return await Task.detached { (try? reader.resource(path: path)).flatMap(UIImage.init(data:)) }.value
    }

    /// Makes the page on screen the book's cover, in a cover's shape.
    func useCurrentPageAsCover() async {
        let picture = await pictureOfPage()
        guard let image = picture ?? pageImage(), let jpeg = coverJPEG(image, width: 600, centre: picture == nil ? 0.5 : 0) else {
            return failure = ("The cover was not changed", "This page could not be made into a cover.")
        }
        setCover(jpeg)
    }

    private func setCover(_ jpeg: Data) {
        do { try engine.setBookCustomCover(id: info.id, jpeg: jpeg) } catch { return failure = describe(error) }
        reloadInfo()
    }

    /// Goes back to the cover the book came with.
    func useOwnCover() {
        do { try engine.clearBookCustomCover(id: info.id) } catch { return failure = describe(error) }
        reloadInfo()
    }

    private func reloadInfo() {
        if let found = try? engine.bookInfo(id: info.id) { info = found }
    }
}
