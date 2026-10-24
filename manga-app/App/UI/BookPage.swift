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
    /// The book is read by scrolling: pages are stacked without their feet and with no space above and below.
    var scrolling: Bool { get }
    /// The layout of a chapter if it has been set in type already; asking does not start it.
    func peek(chapter: Int) -> ChapterLayout?
    /// The chapter's type, or nil while it is being set (it then tells the pages with `.bookLayoutReady`).
    func layout(chapter: Int) -> ChapterLayout?
    func footer(for place: BookPlace, of layout: ChapterLayout) -> (title: String, number: String)
    /// A picture of the book at about the size wanted, nil until it is decoded (`ready` is called then).
    func picture(path: String, width: CGFloat, ready: @escaping @MainActor @Sendable () -> Void) -> CGImage?
    func turned(to place: BookPlace)
    /// A tap on the page: where, the link under it if any (`isNote` when that link is a raised mark, a footnote's number),
    /// and the word under it if any.
    func tapped(_ zone: BookTapZone, link: String?, isNote: Bool, word: BookSpan?)
    /// One page forward or back, without the page-turn motion (a swipe in ink mode).
    func turnPage(forward: Bool)
    var overlays: BookOverlays { get }
    /// The book is read from right to left: the pages turn the other way.
    var rightToLeft: Bool { get }
    /// The selection moved (nil to drop it); a selection is made by a long press and moved by its handles.
    func select(_ span: BookSpan?)
    /// The words of a stretch of a chapter, from the page that is showing it.
    func perform(_ action: BookAction, on span: BookSpan, mark: Int64?, words: String)
    /// A voice model is on the phone, so "Read from here" is offered.
    var canRead: Bool { get }
}

enum BookTapZone { case back, forward, middle }

extension Notification.Name {
    static let bookLayoutReady = Notification.Name("aki.book.layout.ready")
    /// Highlights, the selection or the place found changed.
    static let bookOverlaysChanged = Notification.Name("aki.book.overlays.changed")
}

/// Draws one page: the text, the pictures and the foot line.
final class BookPageView: UIView {
    var layout: ChapterLayout?
    var page = 0
    var theme = BookTheme.paper
    var footer: (title: String, number: String) = ("", "")
    var insets = UIEdgeInsets.zero
    weak var paging: (any BookPaging)?
    var overlays = BookOverlays()
    /// Part of a scrolled book: there is no foot line.
    var scrolling = false

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
        if let cover = layout.coverPicture {
            // A cover or a picture page: the whole screen, the picture as large as it can be without being cut.
            if let picture = paging?.picture(path: cover, width: max(bounds.width, bounds.height), ready: { [weak self] in self?.setNeedsDisplay() }) {
                let scale = min(bounds.width / CGFloat(picture.width), bounds.height / CGFloat(picture.height))
                let size = CGSize(width: CGFloat(picture.width) * scale, height: CGFloat(picture.height) * scale)
                let area = CGRect(x: (bounds.width - size.width) / 2, y: (bounds.height - size.height) / 2, width: size.width, height: size.height)
                context.saveGState()
                context.translateBy(x: 0, y: bounds.height)
                context.scaleBy(x: 1, y: -1)
                context.draw(picture, in: CGRect(x: area.minX, y: bounds.height - area.maxY, width: area.width, height: area.height))
                context.restoreGState()
            }
            return
        }
        context.saveGState()
        // Core Text draws from the bottom left with y up: the text area is put there.
        context.translateBy(x: insets.left, y: insets.top + layout.size.height)
        context.scaleBy(x: 1, y: -1)
        context.textMatrix = .identity
        context.setFillColor(theme.ink.cgColor)
        let frame = layout.frame(page: page)
        drawOverlays(frame, layout: layout, in: context)
        context.setFillColor(theme.ink.cgColor)
        CTFrameDraw(frame, context)
        drawAttachments(of: frame, in: context)
        context.restoreGState()
        if !scrolling { drawFooter() }
    }

    /// Highlights, the selection and the place a search found, under the text. The context has y up here, so each box is turned over.
    private func drawOverlays(_ frame: CTFrame, layout: ChapterLayout, in context: CGContext) {
        func fill(_ span: BookSpan, _ colour: UIColor) {
            guard span.chapter == layout.index else { return }
            context.setFillColor(colour.cgColor)
            for box in layout.boxes(from: span.start, to: span.end, page: page, in: frame) {
                context.fill(CGRect(x: box.minX, y: layout.size.height - box.maxY, width: box.width, height: box.height).insetBy(dx: -1, dy: 1))
            }
        }
        for mark in overlays.marks { fill(mark.span, HighlightColour.fill(mark.colour)) }
        if let found = overlays.found { fill(found, HighlightColour.fill(0)) }
        if let speaking = overlays.speaking { fill(speaking, HighlightColour.fill(0)) }
        if let selection = overlays.selection { fill(selection, HighlightColour.selection.withAlphaComponent(0.28)) }
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
///
/// A long press selects a word, the handles move the ends, and a menu offers what can be done with it; a tap on a highlight opens
/// the same menu for it. All of that is drawn and worked out here and in `ChapterLayout`: there is no text view.
final class BookPageController: UIViewController, @preconcurrency UIEditMenuInteractionDelegate, UIGestureRecognizerDelegate {
    let place: BookPlace
    private unowned let paging: any BookPaging
    private let canvas = BookPageView()
    private let handles = [BookHandle(atStart: true), BookHandle(atStart: false)]
    private var menu: UIEditMenuInteraction?
    /// What the menu now open is about: a selection, or a highlight tapped.
    private var target: (span: BookSpan, mark: Int64?)?

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
        let tap = UITapGestureRecognizer(target: self, action: #selector(tapped(_:)))
        tap.delegate = self
        view.addGestureRecognizer(tap)
        let press = UILongPressGestureRecognizer(target: self, action: #selector(pressed(_:)))
        press.minimumPressDuration = 0.4
        press.delegate = self
        view.addGestureRecognizer(press)
        for handle in handles {
            handle.isHidden = true
            handle.addGestureRecognizer(UIPanGestureRecognizer(target: self, action: #selector(dragged(_:))))
            view.addSubview(handle)
        }
        let menu = UIEditMenuInteraction(delegate: self)
        view.addInteraction(menu)
        self.menu = menu
        NotificationCenter.default.addObserver(self, selector: #selector(layoutReady(_:)), name: .bookLayoutReady, object: nil)
        NotificationCenter.default.addObserver(self, selector: #selector(overlaysChanged), name: .bookOverlaysChanged, object: nil)
        show()
    }

    @objc private func layoutReady(_ note: Notification) {
        if note.object as? Int == place.chapter { show() }
    }

    @objc private func overlaysChanged() { show() }

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
        canvas.scrolling = paging.scrolling
        canvas.overlays = paging.overlays
        guard let layout = paging.layout(chapter: place.chapter), let resolved else {
            canvas.layout = nil
            canvas.setNeedsDisplay()
            handles.forEach { $0.isHidden = true }
            return
        }
        canvas.layout = layout
        canvas.page = resolved.page
        if paging.scrolling { canvas.insets.top = layout.leading(of: resolved.page) }
        canvas.footer = paging.footer(for: resolved, of: layout)
        canvas.setNeedsDisplay()
        placeHandles(layout: layout, page: resolved.page)
    }

    private func placeHandles(layout: ChapterLayout, page: Int) {
        guard let selection = paging.overlays.selection, selection.chapter == layout.index else {
            handles.forEach { $0.isHidden = true }
            return
        }
        let boxes = layout.boxes(from: selection.start, to: selection.end, page: page, in: layout.frame(page: page))
        guard let first = boxes.first, let last = boxes.last else {
            handles.forEach { $0.isHidden = true }
            return
        }
        let insets = canvas.insets
        handles[0].center = CGPoint(x: insets.left + first.minX, y: insets.top + first.minY - 6)
        handles[1].center = CGPoint(x: insets.left + last.maxX, y: insets.top + last.maxY + 10)
        handles.forEach { $0.isHidden = false }
    }

    /// A touch on a selection handle belongs to the handle, not to the page under it.
    func gestureRecognizer(_ gestureRecognizer: UIGestureRecognizer, shouldReceive touch: UITouch) -> Bool {
        !(touch.view is BookHandle)
    }

    /// A point of the page in the text area's own coordinates.
    private func inText(_ point: CGPoint) -> CGPoint {
        CGPoint(x: point.x - canvas.insets.left, y: point.y - canvas.insets.top)
    }

    @objc private func tapped(_ tap: UITapGestureRecognizer) {
        let point = tap.location(in: view)
        guard let resolved, let layout = paging.layout(chapter: resolved.chapter) else { return }
        if paging.overlays.selection != nil { return paging.select(nil) }
        let inside = inText(point)
        let frame = layout.frame(page: resolved.page)
        if let mark = paging.overlays.marks.last(where: { mark in
            mark.span.chapter == layout.index && layout.boxes(from: mark.span.start, to: mark.span.end, page: resolved.page, in: frame).contains { $0.insetBy(dx: -4, dy: -4).contains(inside) }
        }) {
            return open(menu: (mark.span, mark.id), at: point)
        }
        let link = layout.link(at: inside, page: resolved.page)
        let zone: BookTapZone = point.x < view.bounds.width * 0.3 ? .back : point.x > view.bounds.width * 0.7 ? .forward : .middle
        // The word the finger is on: the point must be on its letters, not between the lines.
        let word = layout.position(at: inside, page: resolved.page).flatMap { layout.word(around: $0) }.flatMap { word in
            layout.boxes(from: word.start, to: word.end, page: resolved.page, in: frame).contains { $0.insetBy(dx: -4, dy: -4).contains(inside) }
                ? BookSpan(chapter: layout.index, start: word.start, end: word.end) : nil
        }
        paging.tapped(zone, link: link?.link, isNote: link?.raised ?? false, word: word)
    }

    @objc private func pressed(_ press: UILongPressGestureRecognizer) {
        guard press.state == .began, let resolved, let layout = paging.layout(chapter: resolved.chapter) else { return }
        let point = press.location(in: view)
        guard let at = layout.position(at: inText(point), page: resolved.page), let word = layout.word(around: at) else { return }
        let span = BookSpan(chapter: layout.index, start: word.start, end: word.end)
        paging.select(span)
        open(menu: (span, nil), at: point)
    }

    @objc private func dragged(_ pan: UIPanGestureRecognizer) {
        guard let handle = pan.view as? BookHandle, let selection = paging.overlays.selection, let resolved,
              let layout = paging.layout(chapter: resolved.chapter) else { return }
        if pan.state == .ended { return open(menu: (selection, nil), at: handle.center) }
        // The dot is above or below the line it belongs to: the point read is the line's own.
        let point = CGPoint(x: pan.location(in: view).x, y: pan.location(in: view).y + (handle.atStart ? 14 : -14))
        guard let at = layout.position(at: inText(point), page: resolved.page, end: !handle.atStart) else { return }
        var span = selection
        if handle.atStart { span.start = min(at, selection.end - 1) } else { span.end = max(at, selection.start + 1) }
        paging.select(span)
    }

    private func open(menu target: (span: BookSpan, mark: Int64?), at point: CGPoint) {
        self.target = target
        menu?.presentEditMenu(with: UIEditMenuConfiguration(identifier: nil, sourcePoint: point))
    }

    func editMenuInteraction(_ interaction: UIEditMenuInteraction, menuFor configuration: UIEditMenuConfiguration, suggestedActions: [UIMenuElement]) -> UIMenu? {
        guard let target, let layout = paging.layout(chapter: target.span.chapter) else { return nil }
        let words = layout.words(from: target.span.start, to: target.span.end)
        func act(_ action: BookAction) { paging.perform(action, on: target.span, mark: target.mark, words: words) }
        let colours = UIMenu(title: "Colour", image: HighlightColour.swatch(0), children: HighlightColour.names.indices.map { index in
            UIAction(title: HighlightColour.names[index], image: HighlightColour.swatch(index)) { _ in act(.highlight(colour: index)) }
        })
        var actions: [UIMenuElement] = []
        if paging.canRead { actions.append(UIAction(title: "Read from here", image: UIImage(systemName: "play.circle")) { _ in act(.read) }) }
        if target.mark == nil { actions.append(UIAction(title: "Highlight", image: UIImage(systemName: "highlighter")) { _ in act(.highlight(colour: 0)) }) }
        actions += [colours, UIAction(title: "Note", image: UIImage(systemName: "square.and.pencil")) { _ in act(.note) },
                    UIAction(title: "Copy", image: UIImage(systemName: "doc.on.doc")) { _ in act(.copy) }]
        if target.mark != nil { actions.append(UIAction(title: "Remove highlight", image: UIImage(systemName: "trash"), attributes: .destructive) { _ in act(.remove) }) }
        return UIMenu(children: actions)
    }
}

/// Where text goes on a page, so that what is set in type and what is drawn agree.
enum BookGeometry {
    static func top(safe: UIEdgeInsets) -> CGFloat { max(safe.top, 20) + 12 }
    static func bottom(safe: UIEdgeInsets) -> CGFloat { max(safe.bottom, 12) + 30 }

    /// The text area of a screen of `size` with `safe` insets and the style's margins, and its place on the screen.
    static func insets(width: CGFloat, safe: UIEdgeInsets, margin: BookMargin, typeSize: Double) -> UIEdgeInsets {
        // A line of more than about seventy characters is tiring to follow: on a wide screen the margins grow to keep it shorter.
        let side = max(margin.points, (width - CGFloat(typeSize) * 34) / 2)
        return UIEdgeInsets(top: top(safe: safe), left: side, bottom: bottom(safe: safe), right: side)
    }

    static func textSize(screen size: CGSize, safe: UIEdgeInsets, margin: BookMargin, typeSize: Double) -> CGSize {
        let insets = insets(width: size.width, safe: safe, margin: margin, typeSize: typeSize)
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
    /// A still display: no page-turn motion.
    let still: Bool
    /// A selection is being made: turning pages by touch waits.
    let locked: Bool

    func makeUIViewController(context: Context) -> UIPageViewController {
        let controller = UIPageViewController(transitionStyle: .scroll, navigationOrientation: .horizontal, options: [.interPageSpacing: 0])
        controller.dataSource = context.coordinator
        controller.delegate = context.coordinator
        controller.view.backgroundColor = paging.theme.paper
        for direction in [UISwipeGestureRecognizer.Direction.left, .right] {
            let swipe = UISwipeGestureRecognizer(target: context.coordinator, action: #selector(Coordinator.swiped(_:)))
            swipe.direction = direction
            controller.view.addGestureRecognizer(swipe)
        }
        return controller
    }

    func updateUIViewController(_ controller: UIPageViewController, context: Context) {
        let coordinator = context.coordinator
        coordinator.paging = paging
        controller.view.backgroundColor = paging.theme.paper
        // Still: the system's sliding scroll is turned off, and a swipe turns the page at once.
        for case let scroll as UIScrollView in controller.view.subviews { scroll.isScrollEnabled = !still && !locked }
        controller.view.gestureRecognizers?.compactMap { $0 as? UISwipeGestureRecognizer }.forEach { $0.isEnabled = still && !locked }
        if let jump, jump.id != coordinator.shown {
            coordinator.shown = jump.id
            let page = BookPageController(place: jump.place, paging: paging)
            // In a book read from right to left the next page comes in from the left.
            let direction: UIPageViewController.NavigationDirection = jump.forward != paging.rightToLeft ? .forward : .reverse
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

        @objc func swiped(_ swipe: UISwipeGestureRecognizer) {
            // Forward is to the left in a book read from left to right, and to the right in the other.
            paging.turnPage(forward: (swipe.direction == .left) != paging.rightToLeft)
        }

        // The system's "after" is to the right of the page. In a book read from right to left that is the page before.
        func pageViewController(_ controller: UIPageViewController, viewControllerAfter viewController: UIViewController) -> UIViewController? {
            paging.rightToLeft ? previous(viewController) : next(viewController)
        }

        func pageViewController(_ controller: UIPageViewController, viewControllerBefore viewController: UIViewController) -> UIViewController? {
            paging.rightToLeft ? next(viewController) : previous(viewController)
        }

        private func next(_ viewController: UIViewController) -> UIViewController? {
            guard let page = viewController as? BookPageController, let place = page.resolved else { return nil }
            guard let layout = paging.layout(chapter: place.chapter) else { return nil }
            if place.page + 1 < layout.count { return BookPageController(place: BookPlace(chapter: place.chapter, page: place.page + 1), paging: paging) }
            guard place.chapter + 1 < paging.chapterCount else { return nil }
            return BookPageController(place: BookPlace(chapter: place.chapter + 1, page: 0), paging: paging)
        }

        private func previous(_ viewController: UIViewController) -> UIViewController? {
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

/// A book read by scrolling: every chapter's pages one under another, the next chapter joined on as the end nears.
struct BookScroller: UIViewControllerRepresentable {
    let paging: any BookPaging
    let jump: BookJump?

    func makeUIViewController(context: Context) -> BookScrollController { BookScrollController(paging: paging) }

    func updateUIViewController(_ controller: BookScrollController, context: Context) {
        controller.paging = paging
        controller.refresh()
        controller.apply(jump)
    }
}

final class BookScrollController: UIViewController, UICollectionViewDataSource, UICollectionViewDelegateFlowLayout {
    var paging: any BookPaging
    private let collection: UICollectionView
    private var shownJump = -1
    /// A place to scroll to as soon as its chapter has been set in type.
    private var waiting: (place: BookPlace, animated: Bool)?
    /// What each chapter's pages came to once set in type, kept when the chapter is let go, so the scroll does not jump.
    private var tiles: [Int: [CGFloat]] = [:]
    private var reported: IndexPath?
    private static let placeholder: CGFloat = 480

    init(paging: any BookPaging) {
        self.paging = paging
        let flow = UICollectionViewFlowLayout()
        flow.minimumLineSpacing = 0
        flow.minimumInteritemSpacing = 0
        collection = UICollectionView(frame: .zero, collectionViewLayout: flow)
        super.init(nibName: nil, bundle: nil)
    }

    required init?(coder: NSCoder) { fatalError() }

    override func viewDidLoad() {
        super.viewDidLoad()
        collection.frame = view.bounds
        collection.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        collection.dataSource = self
        collection.delegate = self
        collection.register(BookTileCell.self, forCellWithReuseIdentifier: "tile")
        collection.contentInsetAdjustmentBehavior = .never
        collection.showsVerticalScrollIndicator = false
        view.addSubview(collection)
        NotificationCenter.default.addObserver(self, selector: #selector(layoutReady(_:)), name: .bookLayoutReady, object: nil)
        refresh()
    }

    /// The colours and the room above and below the text, which the reader can change.
    func refresh() {
        guard isViewLoaded else { return }
        view.backgroundColor = paging.theme.paper
        collection.backgroundColor = paging.theme.paper
        collection.contentInset = UIEdgeInsets(top: BookGeometry.top(safe: BookGeometry.safe), left: 0, bottom: BookGeometry.bottom(safe: BookGeometry.safe), right: 0)
    }

    func apply(_ jump: BookJump?) {
        guard let jump, jump.id != shownJump else { return }
        shownJump = jump.id
        scroll(to: jump.place, animated: jump.animated)
    }

    private func scroll(to place: BookPlace, animated: Bool) {
        guard isViewLoaded, place.chapter < collection.numberOfSections else { return }
        if tiles[place.chapter] == nil, let layout = paging.layout(chapter: place.chapter) { learn(layout) }
        guard let found = tiles[place.chapter] else {
            waiting = (place, animated)
            return
        }
        waiting = nil
        let item = place.page < 0 ? found.count - 1 : min(place.page, found.count - 1)
        collection.layoutIfNeeded()
        collection.scrollToItem(at: IndexPath(item: item, section: place.chapter), at: .top, animated: animated)
        if !animated { collection.layoutIfNeeded() }
    }

    /// The heights of a chapter's pages, kept.
    private func measure(_ layout: ChapterLayout) {
        tiles[layout.index] = (0..<layout.count).map { layout.leading(of: $0) + layout.extent(of: $0) }
    }

    /// A chapter has been set in type: its pages are measured and its section, which held one placeholder, is reloaded. Measuring
    /// anywhere else would change the number of pages the collection view is told about without telling it, and it stops the app.
    /// A chapter above the one being read grows: the collection view keeps what is on screen where it is.
    private func learn(_ layout: ChapterLayout) {
        measure(layout)
        UIView.performWithoutAnimation {
            collection.reloadSections(IndexSet(integer: layout.index))
            collection.layoutIfNeeded()
        }
    }

    @objc private func layoutReady(_ note: Notification) {
        guard let chapter = note.object as? Int, isViewLoaded, tiles[chapter] == nil, let layout = paging.peek(chapter: chapter) else { return }
        learn(layout)
        if let waiting, waiting.place.chapter == chapter { scroll(to: waiting.place, animated: waiting.animated) }
    }

    // MARK: Data

    func numberOfSections(in collectionView: UICollectionView) -> Int { paging.chapterCount }

    func collectionView(_ collectionView: UICollectionView, numberOfItemsInSection section: Int) -> Int {
        tiles[section]?.count ?? 1
    }

    func collectionView(_ collectionView: UICollectionView, cellForItemAt indexPath: IndexPath) -> UICollectionViewCell {
        let cell = collectionView.dequeueReusableCell(withReuseIdentifier: "tile", for: indexPath) as! BookTileCell
        cell.show(BookPlace(chapter: indexPath.section, page: indexPath.item), paging: paging, in: self)
        return cell
    }

    func collectionView(_ collectionView: UICollectionView, layout collectionViewLayout: UICollectionViewLayout, sizeForItemAt indexPath: IndexPath) -> CGSize {
        let known = tiles[indexPath.section]
        return CGSize(width: collectionView.bounds.width, height: known.map { $0[min(indexPath.item, $0.count - 1)] } ?? Self.placeholder)
    }

    func collectionView(_ collectionView: UICollectionView, willDisplay cell: UICollectionViewCell, forItemAt indexPath: IndexPath) {
        // The chapters around the one on screen are set in type before they are reached.
        for chapter in [indexPath.section - 1, indexPath.section, indexPath.section + 1] where chapter >= 0 && chapter < paging.chapterCount {
            _ = paging.layout(chapter: chapter)
        }
    }

    // MARK: Where the person is

    func scrollViewDidScroll(_ scrollView: UIScrollView) {
        let probe = CGPoint(x: scrollView.bounds.midX, y: scrollView.contentOffset.y + scrollView.contentInset.top + 24)
        guard let at = collection.indexPathForItem(at: probe), at != reported, tiles[at.section] != nil else { return }
        reported = at
        paging.turned(to: BookPlace(chapter: at.section, page: at.item))
    }
}

/// One page of a scrolled book: the page's own controller inside a cell.
final class BookTileCell: UICollectionViewCell {
    private var host: BookPageController?

    func show(_ place: BookPlace, paging: any BookPaging, in parent: UIViewController) {
        if host?.place == place { return host?.show() ?? () }
        clear()
        let page = BookPageController(place: place, paging: paging)
        parent.addChild(page)
        page.view.frame = contentView.bounds
        page.view.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        contentView.addSubview(page.view)
        page.didMove(toParent: parent)
        host = page
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        host?.view.frame = contentView.bounds
    }

    private func clear() {
        host?.willMove(toParent: nil)
        host?.view.removeFromSuperview()
        host?.removeFromParent()
        host = nil
    }

    override func prepareForReuse() {
        super.prepareForReuse()
        clear()
    }
}
