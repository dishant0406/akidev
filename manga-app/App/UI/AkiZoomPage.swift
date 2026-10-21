import SwiftUI
import UIKit

/// One page of the paged reader: the image fitted to the screen, pinch to zoom up to 4×, double tap to
/// zoom in on a point and out again, drag to move around when zoomed. A single tap reports where on the
/// screen it landed (0 to 1 across), so the reader can turn the page or show its controls.
struct AkiZoomPage: UIViewRepresentable {
    let url: String
    let headers: [String: String]
    let onTap: (CGFloat) -> Void
    @Environment(\.displayScale) private var scale

    func makeUIView(context: Context) -> ZoomScrollView {
        let view = ZoomScrollView()
        view.delegate = context.coordinator
        view.onTap = { context.coordinator.parent.onTap($0) }
        return view
    }

    func updateUIView(_ view: ZoomScrollView, context: Context) {
        context.coordinator.parent = self
        guard context.coordinator.loaded != url else { return }
        context.coordinator.loaded = url
        view.show(nil)
        guard let link = URL(string: url) else { return }
        let widthPx = UIScreen.main.bounds.width * scale * 2
        let headers = headers
        context.coordinator.task?.cancel()
        context.coordinator.task = Task { @MainActor in
            let image = await ImageLoader.shared.image(link, headers: headers, widthPx: widthPx)
            guard !Task.isCancelled else { return }
            view.show(image)
        }
    }

    func makeCoordinator() -> Coordinator { Coordinator(self) }

    final class Coordinator: NSObject, UIScrollViewDelegate {
        var parent: AkiZoomPage
        var loaded: String?
        var task: Task<Void, Never>?
        init(_ parent: AkiZoomPage) { self.parent = parent }
        func viewForZooming(in scrollView: UIScrollView) -> UIView? { (scrollView as? ZoomScrollView)?.imageView }
        func scrollViewDidZoom(_ scrollView: UIScrollView) { (scrollView as? ZoomScrollView)?.centre() }
        deinit { task?.cancel() }
    }
}

final class ZoomScrollView: UIScrollView {
    let imageView = UIImageView()
    var onTap: ((CGFloat) -> Void)?
    private var laidOutFor: CGSize = .zero

    override init(frame: CGRect) {
        super.init(frame: frame)
        minimumZoomScale = 1
        maximumZoomScale = 4
        showsVerticalScrollIndicator = false
        showsHorizontalScrollIndicator = false
        contentInsetAdjustmentBehavior = .never
        bouncesZoom = true
        imageView.contentMode = .scaleAspectFit
        alwaysBounceVertical = false
        addSubview(imageView)
        let double = UITapGestureRecognizer(target: self, action: #selector(doubleTapped(_:)))
        double.numberOfTapsRequired = 2
        let single = UITapGestureRecognizer(target: self, action: #selector(tapped(_:)))
        single.require(toFail: double)
        addGestureRecognizer(double)
        addGestureRecognizer(single)
    }

    required init?(coder: NSCoder) { fatalError() }

    func show(_ image: UIImage?) {
        imageView.image = image
        zoomScale = 1
        laidOutFor = .zero
        setNeedsLayout()
    }

    /// A page much taller than the screen (a webtoon strip shown in page mode) is fitted to the width and
    /// scrolled down; any other page is fitted whole.
    override func layoutSubviews() {
        super.layoutSubviews()
        if laidOutFor != bounds.size {
            laidOutFor = bounds.size
            zoomScale = 1
            var fitted = bounds.size
            if let size = imageView.image?.size, size.width > 0, size.width / size.height < 0.5 {
                fitted = CGSize(width: bounds.width, height: bounds.width * size.height / size.width)
            }
            imageView.frame = CGRect(origin: .zero, size: fitted)
            contentSize = fitted
        }
        centre()
    }

    func centre() {
        let dx = max((bounds.width - contentSize.width) / 2, 0)
        let dy = max((bounds.height - contentSize.height) / 2, 0)
        imageView.frame.origin = CGPoint(x: dx, y: dy)
    }

    @objc private func tapped(_ tap: UITapGestureRecognizer) {
        onTap?(tap.location(in: self).x / max(bounds.width, 1))
    }

    @objc private func doubleTapped(_ tap: UITapGestureRecognizer) {
        if zoomScale > 1.01 {
            setZoomScale(1, animated: true)
        } else {
            let point = tap.location(in: imageView)
            let size = CGSize(width: bounds.width / 2.5, height: bounds.height / 2.5)
            zoom(to: CGRect(x: point.x - size.width / 2, y: point.y - size.height / 2, width: size.width, height: size.height), animated: true)
        }
    }
}
