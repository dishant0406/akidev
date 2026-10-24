import CoreText
import SwiftUI
import UIKit

/// A page of a book: a chapter and a page in it. A page of -1 is the chapter's last.
struct BookPlace: Equatable {
    var chapter: Int
    var page: Int
}

/// What the reader's pages ask of the book they show.
@MainActor protocol BookPaging: AnyObject {
    var chapterCount: Int { get }
    var theme: BookTheme { get }
    /// Where the text area sits on the screen.
    var pageInsets: UIEdgeInsets { get }
    /// The chapter's type, or nil while it is being set (it then tells the pages with `.bookLayoutReady`).
    func layout(chapter: Int) -> ChapterLayout?
    func footer(for place: BookPlace, of layout: ChapterLayout) -> (title: String, number: String)
    /// A picture of the book at about the size wanted, nil until it is decoded (`ready` is called then).
    func picture(path: String, width: CGFloat, ready: @escaping @MainActor @Sendable () -> Void) -> CGImage?
    func turned(to place: BookPlace)
    func tapped(_ zone: BookTapZone, link: String?)
}

enum BookTapZone { case back, forward, middle }

extension Notification.Name {
    static let bookLayoutReady = Notification.Name("aki.book.layout.ready")
}

/// Draws one page: the text, the pictures and the foot line.
final class BookPageView: UIView {
    var layout: ChapterLayout?
    var page = 0
    var theme = BookTheme.paper
    var footer: (title: String, number: String) = ("", "")
    var insets = UIEdgeInsets.zero
    weak var paging: (any BookPaging)?

    override init(frame: CGRect) {
        super.init(frame: frame)
        isOpaque = true
        contentMode = .redraw
    }

    required init?(coder: NSCoder) { fatalError() }

    override func draw(_ rect: CGRect) {
        guard let context = UIGraphicsGetCurrentContext() else { return }
        theme.paper.setFill()
        context.fill(rect)
        guard let layout else { return }
        context.saveGState()
        // Core Text draws from the bottom left with y up: the text area is put there.
        context.translateBy(x: insets.left, y: insets.top + layout.size.height)
        context.scaleBy(x: 1, y: -1)
        context.textMatrix = .identity
        context.setFillColor(theme.ink.cgColor)
        let frame = layout.frame(page: page)
        CTFrameDraw(frame, context)
        drawAttachments(of: frame, in: context)
        context.restoreGState()
        drawFooter()
    }

    /// Pictures, rules and strike-throughs: what Core Text leaves a place for and does not draw.
    private func drawAttachments(of frame: CTFrame, in context: CGContext) {
        let lines = CTFrameGetLines(frame) as! [CTLine]
        var origins = [CGPoint](repeating: .zero, count: lines.count)
        CTFrameGetLineOrigins(frame, CFRange(location: 0, length: 0), &origins)
        for (line, origin) in zip(lines, origins) {
            drawHyphen(ending: line, at: origin, in: context)
            for run in CTLineGetGlyphRuns(line) as! [CTRun] {
                let attributes = CTRunGetAttributes(run) as NSDictionary
                var position = CGPoint.zero
                CTRunGetPositions(run, CFRange(location: 0, length: 1), &position)
                var ascent: CGFloat = 0, descent: CGFloat = 0
                let width = CGFloat(CTRunGetTypographicBounds(run, CFRange(location: 0, length: 0), &ascent, &descent, nil))
                if let attachment = attributes[NSAttributedString.Key.bookAttachment] as? BookAttachment {
                    let area = CGRect(x: origin.x + position.x, y: origin.y + position.y, width: attachment.size.width, height: attachment.size.height)
                    switch attachment.kind {
                    case .image(let path, let ink):
                        guard let picture = paging?.picture(path: path, width: area.width, ready: { [weak self] in self?.setNeedsDisplay() }) else { break }
                        if ink {
                            // Dark ink on clear: drawn in the colour of the text, so it shows on a dark page too.
                            context.beginTransparencyLayer(in: area, auxiliaryInfo: nil)
                            context.draw(picture, in: area)
                            context.setBlendMode(.sourceIn)
                            context.setFillColor(theme.ink.cgColor)
                            context.fill(area)
                            context.endTransparencyLayer()
                        } else {
                            context.draw(picture, in: area)
                        }
                    case .rule:
                        context.setFillColor(theme.quiet.cgColor)
                        context.fill(CGRect(x: area.minX, y: area.midY, width: area.width, height: 1))
                    }
                } else if attributes[NSAttributedString.Key.bookStrike] != nil {
                    context.setFillColor(theme.ink.cgColor)
                    context.fill(CGRect(x: origin.x + position.x, y: origin.y + position.y + ascent * 0.3, width: width, height: 1))
                }
            }
        }
    }

    /// Core Text breaks a word at a soft hyphen but does not draw the hyphen: it is put just after the line, hanging into the margin.
    private func drawHyphen(ending line: CTLine, at origin: CGPoint, in context: CGContext) {
        guard let text = layout?.text else { return }
        let range = CTLineGetStringRange(line)
        let last = range.location + range.length - 1
        guard range.length > 0, last < text.length, (text.string as NSString).character(at: last) == 0x00AD else { return }
        var attributes = text.attributes(at: last, effectiveRange: nil)
        attributes[.paragraphStyle] = nil
        let hyphen = CTLineCreateWithAttributedString(NSAttributedString(string: "-", attributes: attributes))
        let width = CTLineGetTypographicBounds(line, nil, nil, nil)
        context.textPosition = CGPoint(x: origin.x + width, y: origin.y)
        CTLineDraw(hyphen, context)
    }

    private func drawFooter() {
        let font = UIFont.systemFont(ofSize: 12, weight: .medium)
        let attributes: [NSAttributedString.Key: Any] = [.font: font, .foregroundColor: theme.quiet]
        let y = bounds.height - insets.bottom + 6
        let width = bounds.width - insets.left - insets.right
        let number = NSAttributedString(string: footer.number, attributes: attributes)
        let numberWidth = number.size().width
        number.draw(at: CGPoint(x: bounds.width - insets.right - numberWidth, y: y))
        let title = NSAttributedString(string: footer.title, attributes: attributes)
        title.draw(with: CGRect(x: insets.left, y: y, width: max(width - numberWidth - 16, 0), height: 16), options: [.truncatesLastVisibleLine, .usesLineFragmentOrigin], context: nil)
    }
}

/// One page in the pager. While its chapter is being set in type it shows only the paper, then draws itself.
final class BookPageController: UIViewController {
    let place: BookPlace
    private unowned let paging: any BookPaging
    private let canvas = BookPageView()

    init(place: BookPlace, paging: any BookPaging) {
        self.place = place
        self.paging = paging
        super.init(nibName: nil, bundle: nil)
    }

    required init?(coder: NSCoder) { fatalError() }

    /// The place with a last page counted: nil until its chapter has been set in type.
    var resolved: BookPlace? {
        guard let layout = paging.layout(chapter: place.chapter) else { return nil }
        return BookPlace(chapter: place.chapter, page: place.page < 0 ? layout.count - 1 : min(place.page, layout.count - 1))
    }

    override func viewDidLoad() {
        super.viewDidLoad()
        canvas.paging = paging
        canvas.frame = view.bounds
        canvas.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        view.addSubview(canvas)
        view.addGestureRecognizer(UITapGestureRecognizer(target: self, action: #selector(tapped(_:))))
        NotificationCenter.default.addObserver(self, selector: #selector(layoutReady(_:)), name: .bookLayoutReady, object: nil)
        show()
    }

    @objc private func layoutReady(_ note: Notification) {
        if note.object as? Int == place.chapter { show() }
    }

    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        show()
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        show()
    }

    func show() {
        canvas.theme = paging.theme
        canvas.insets = paging.pageInsets
        guard let layout = paging.layout(chapter: place.chapter), let resolved else {
            canvas.layout = nil
            canvas.setNeedsDisplay()
            return
        }
        canvas.layout = layout
        canvas.page = resolved.page
        canvas.footer = paging.footer(for: resolved, of: layout)
        canvas.setNeedsDisplay()
    }

    @objc private func tapped(_ tap: UITapGestureRecognizer) {
        let point = tap.location(in: view)
        let insets = canvas.insets
        let link = resolved.flatMap { paging.layout(chapter: $0.chapter)?.link(at: CGPoint(x: point.x - insets.left, y: point.y - insets.top), page: $0.page) }
        let zone: BookTapZone = point.x < view.bounds.width * 0.3 ? .back : point.x > view.bounds.width * 0.7 ? .forward : .middle
        paging.tapped(zone, link: link)
    }
}

/// Where text goes on a page, so that what is set in type and what is drawn agree.
enum BookGeometry {
    static func top(safe: UIEdgeInsets) -> CGFloat { max(safe.top, 20) + 12 }
    static func bottom(safe: UIEdgeInsets) -> CGFloat { max(safe.bottom, 12) + 30 }

    /// The text area of a screen of `size` with `safe` insets and the style's margins, and its place on the screen.
    static func insets(safe: UIEdgeInsets, margin: BookMargin) -> UIEdgeInsets {
        UIEdgeInsets(top: top(safe: safe), left: margin.points, bottom: bottom(safe: safe), right: margin.points)
    }

    static func textSize(screen size: CGSize, safe: UIEdgeInsets, margin: BookMargin) -> CGSize {
        let insets = insets(safe: safe, margin: margin)
        return CGSize(width: size.width - insets.left - insets.right, height: size.height - insets.top - insets.bottom)
    }

    /// The safe area of the app's window.
    static var safe: UIEdgeInsets {
        let windows = UIApplication.shared.connectedScenes.compactMap { ($0 as? UIWindowScene)?.keyWindow }
        return windows.first?.safeAreaInsets ?? UIEdgeInsets(top: 59, left: 0, bottom: 34, right: 0)
    }
}

/// The pages of a book, turned by swiping or by tapping the edges. A system page controller, so the turn is the system's own.
struct BookPager: UIViewControllerRepresentable {
    let paging: any BookPaging
    /// Changes when the reader should show `place` now (a jump, or the type changing) rather than wait to be turned.
    let jump: BookJump?

    func makeUIViewController(context: Context) -> UIPageViewController {
        let controller = UIPageViewController(transitionStyle: .scroll, navigationOrientation: .horizontal, options: [.interPageSpacing: 0])
        controller.dataSource = context.coordinator
        controller.delegate = context.coordinator
        controller.view.backgroundColor = paging.theme.paper
        return controller
    }

    func updateUIViewController(_ controller: UIPageViewController, context: Context) {
        let coordinator = context.coordinator
        coordinator.paging = paging
        controller.view.backgroundColor = paging.theme.paper
        if let jump, jump.id != coordinator.shown {
            coordinator.shown = jump.id
            let page = BookPageController(place: jump.place, paging: paging)
            let direction: UIPageViewController.NavigationDirection = jump.forward ? .forward : .reverse
            controller.setViewControllers([page], direction: direction, animated: jump.animated)
        } else if let page = controller.viewControllers?.first as? BookPageController {
            page.show()
        }
    }

    func makeCoordinator() -> Coordinator { Coordinator(paging: paging) }

    @MainActor final class Coordinator: NSObject, UIPageViewControllerDataSource, UIPageViewControllerDelegate {
        var paging: any BookPaging
        var shown = -1

        init(paging: any BookPaging) { self.paging = paging }

        func pageViewController(_ controller: UIPageViewController, viewControllerAfter viewController: UIViewController) -> UIViewController? {
            guard let page = viewController as? BookPageController, let place = page.resolved else { return nil }
            guard let layout = paging.layout(chapter: place.chapter) else { return nil }
            if place.page + 1 < layout.count { return BookPageController(place: BookPlace(chapter: place.chapter, page: place.page + 1), paging: paging) }
            guard place.chapter + 1 < paging.chapterCount else { return nil }
            return BookPageController(place: BookPlace(chapter: place.chapter + 1, page: 0), paging: paging)
        }

        func pageViewController(_ controller: UIPageViewController, viewControllerBefore viewController: UIViewController) -> UIViewController? {
            guard let page = viewController as? BookPageController, let place = page.resolved else { return nil }
            if place.page > 0 { return BookPageController(place: BookPlace(chapter: place.chapter, page: place.page - 1), paging: paging) }
            guard place.chapter > 0 else { return nil }
            return BookPageController(place: BookPlace(chapter: place.chapter - 1, page: -1), paging: paging)
        }

        func pageViewController(_ controller: UIPageViewController, didFinishAnimating finished: Bool, previousViewControllers: [UIViewController], transitionCompleted completed: Bool) {
            guard completed, let page = controller.viewControllers?.first as? BookPageController else { return }
            // A chapter not yet set in type is told by its own place: the last page is counted when it is ready.
            paging.turned(to: page.resolved ?? page.place)
        }
    }
}

/// An order to show a place now. `id` changes with every order, so the same place can be asked for twice.
struct BookJump: Equatable {
    var id: Int
    var place: BookPlace
    var forward = true
    var animated = false
}
