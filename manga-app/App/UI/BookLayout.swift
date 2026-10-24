import AkiManga
import CoreText
import UIKit

extension NSAttributedString.Key {
    /// Where a run of text links to, as the book wrote it.
    static let bookLink = NSAttributedString.Key("aki.link")
    static let bookStrike = NSAttributedString.Key("aki.strike")
    static let bookRaised = NSAttributedString.Key("aki.raised")
    /// The picture or rule a placeholder character stands for.
    static let bookAttachment = NSAttributedString.Key("aki.attachment")
}

/// A picture or rule in the text: the page leaves room for it and draws it there.
final class BookAttachment {
    enum Kind { case image(path: String, ink: Bool), rule }
    let kind: Kind
    let size: CGSize

    init(kind: Kind, size: CGSize) {
        self.kind = kind
        self.size = size
    }
}

/// One chapter set in type for one page size: its text, and where each page of it begins and ends.
///
/// Positions in and out are in the book's own units (see `BookBlock.start`); the text drawn has soft hyphens added to long
/// words, so inside this type the two differ, and `display(of:)` and `source(of:)` are the only places that know how.
final class ChapterLayout: @unchecked Sendable {
    let index: Int
    let text: NSAttributedString
    /// The text area of a page.
    let size: CGSize
    /// What each page shows, as ranges of `text`.
    let pages: [NSRange]
    /// Written in vertical columns from the right: the lines are columns, and touching a place in them is not worked out.
    let vertical: Bool
    /// A chapter that is one picture and nothing else (a cover): its path in the book. The page shows it over the whole screen.
    let coverPicture: String?
    private let framesetter: CTFramesetter
    /// Where each soft hyphen was put: the book's index of the character after it, in order.
    private let hyphens: [Int]
    private let anchors: [String: Int]

    var count: Int { pages.count }

    init(index: Int, text: NSAttributedString, size: CGSize, vertical: Bool, coverPicture: String?, breaks: [Int], headings: [NSRange], hyphens: [Int], anchors: [String: Int]) {
        self.index = index
        self.text = text
        self.size = size
        self.vertical = vertical
        self.coverPicture = coverPicture
        self.hyphens = hyphens
        self.anchors = anchors
        framesetter = CTFramesetterCreateWithAttributedString(text)
        pages = ChapterLayout.paginate(text: text, framesetter: framesetter, size: size, vertical: vertical, breaks: breaks, headings: headings)
    }

    // MARK: Positions

    /// Where a position of the book falls in the text drawn.
    func display(of source: Int) -> Int {
        var low = 0, high = hyphens.count
        while low < high {
            let middle = (low + high) / 2
            if hyphens[middle] <= source { low = middle + 1 } else { high = middle }
        }
        return source + low
    }

    /// The position in the book of a place in the text drawn.
    func source(of display: Int) -> Int {
        var low = 0, high = hyphens.count
        while low < high {
            let middle = (low + high) / 2
            if hyphens[middle] + middle < display { low = middle + 1 } else { high = middle }
        }
        return display - low
    }

    func page(containing source: Int) -> Int {
        let place = display(of: source)
        var low = 0, high = pages.count
        while low < high {
            let middle = (low + high) / 2
            if pages[middle].location <= place { low = middle + 1 } else { high = middle }
        }
        return max(low - 1, 0)
    }

    func start(of page: Int) -> Int { source(of: pages[min(max(page, 0), pages.count - 1)].location) }

    func offset(of anchor: String) -> Int? { anchors[anchor] }

    // MARK: Scrolling

    /// How tall a page's text really is: the pages are cut at the foot of the text area, a scrolled book stacks them, and what
    /// is empty under the last line is not kept.
    func extent(of page: Int) -> CGFloat {
        if coverPicture != nil { return Aki.screenSize.height }
        let frame = frame(page: page)
        let lines = CTFrameGetLines(frame) as! [CTLine]
        guard let last = lines.last else { return 0 }
        var origin = CGPoint.zero
        CTFrameGetLineOrigins(frame, CFRange(location: lines.count - 1, length: 1), &origin)
        var descent: CGFloat = 0
        CTLineGetTypographicBounds(last, nil, &descent, nil)
        return ceil(size.height - (origin.y - descent)) + 2
    }

    /// The space a paragraph asks for above it, which the frame leaves out at the top of a page: put back, a scrolled book keeps it.
    func leading(of page: Int) -> CGFloat {
        let start = pages[page].location
        guard page > 0, start > 0, start < text.length, (text.string as NSString).character(at: start - 1) == 0x0A,
              let style = text.attribute(.paragraphStyle, at: start, effectiveRange: nil) as? NSParagraphStyle else { return 0 }
        return style.paragraphSpacingBefore
    }

    // MARK: Drawing

    func frame(page: Int) -> CTFrame {
        let range = pages[page]
        return CTFramesetterCreateFrame(framesetter, CFRange(location: range.location, length: range.length), CGPath(rect: CGRect(origin: .zero, size: size), transform: nil), ChapterLayout.frameAttributes(vertical))
    }

    /// The boxes a stretch of the book's text takes on a page, one for each line it touches, in the text area with y down.
    func boxes(from start: Int, to end: Int, page: Int, in frame: CTFrame) -> [CGRect] {
        guard !vertical else { return [] }
        let range = pages[page]
        let from = max(display(of: start), range.location), to = min(display(of: end), NSMaxRange(range))
        guard from < to else { return [] }
        let lines = CTFrameGetLines(frame) as! [CTLine]
        var origins = [CGPoint](repeating: .zero, count: lines.count)
        CTFrameGetLineOrigins(frame, CFRange(location: 0, length: 0), &origins)
        var boxes: [CGRect] = []
        for (line, origin) in zip(lines, origins) {
            let covered = CTLineGetStringRange(line)
            let low = max(from, covered.location), high = min(to, covered.location + covered.length)
            guard low < high else { continue }
            var ascent: CGFloat = 0, descent: CGFloat = 0
            CTLineGetTypographicBounds(line, &ascent, &descent, nil)
            // In right-to-left text the end of a stretch is to the left of its start.
            let one = origin.x + CTLineGetOffsetForStringIndex(line, low, nil)
            let other = origin.x + CTLineGetOffsetForStringIndex(line, high, nil)
            boxes.append(CGRect(x: min(one, other), y: size.height - origin.y - ascent, width: max(abs(other - one), 2), height: ascent + descent))
        }
        return boxes
    }

    /// Where in the book's text a point (in the text area, y down) is: the nearest letter, on the nearest line.
    func position(at point: CGPoint, page: Int, end: Bool = false) -> Int? {
        guard !vertical else { return nil }
        let frame = frame(page: page)
        let lines = CTFrameGetLines(frame) as! [CTLine]
        guard !lines.isEmpty else { return nil }
        var origins = [CGPoint](repeating: .zero, count: lines.count)
        CTFrameGetLineOrigins(frame, CFRange(location: 0, length: 0), &origins)
        let y = size.height - point.y
        // The line whose middle is nearest: a point in the upper half of a line belongs to that line, not the one above.
        func middle(_ index: Int) -> CGFloat {
            var ascent: CGFloat = 0, descent: CGFloat = 0
            CTLineGetTypographicBounds(lines[index], &ascent, &descent, nil)
            return origins[index].y + (ascent - descent) / 2
        }
        let nearest = lines.indices.min { abs(middle($0) - y) < abs(middle($1) - y) }!
        let at = CTLineGetStringIndexForPosition(lines[nearest], CGPoint(x: point.x - origins[nearest].x, y: 0))
        guard at != kCFNotFound else { return nil }
        let range = pages[page]
        return source(of: min(max(at, range.location), end ? NSMaxRange(range) : max(NSMaxRange(range) - 1, range.location)))
    }

    /// The word around a position, as a stretch of the book's text.
    func word(around position: Int) -> (start: Int, end: Int)? {
        let string = text.string as NSString
        var low = display(of: position), high = low
        guard low < string.length else { return nil }
        func inWord(_ index: Int) -> Bool {
            let c = string.character(at: index)
            guard let scalar = Unicode.Scalar(c) else { return true }
            return CharacterSet.alphanumerics.contains(scalar) || c == 0x00AD || c == 0x2019 || c == 0x27
        }
        // A point between a word and the space after it belongs to the word.
        if !inWord(low), low > 0, inWord(low - 1) { low -= 1; high = low }
        guard inWord(low) else { return nil }
        while low > 0, inWord(low - 1) { low -= 1 }
        while high < string.length, inWord(high) { high += 1 }
        return (source(of: low), source(of: high))
    }

    /// The words of a stretch of the book's text, as a person would read them.
    func words(from start: Int, to end: Int) -> String {
        let string = text.string as NSString
        let from = display(of: start), to = min(display(of: end), string.length)
        guard from < to else { return "" }
        return string.substring(with: NSRange(location: from, length: to - from))
            .replacingOccurrences(of: "\u{00AD}", with: "").replacingOccurrences(of: "\u{FFFC}", with: "")
            .replacingOccurrences(of: "\u{2028}", with: " ").replacingOccurrences(of: "\n", with: " ")
    }

    /// What a tap at `point` (in the text area, y down) lands on: the link under it, if any.
    func link(at point: CGPoint, page: Int) -> (link: String, raised: Bool)? {
        guard !vertical else { return nil }
        let frame = frame(page: page)
        let lines = CTFrameGetLines(frame) as! [CTLine]
        var origins = [CGPoint](repeating: .zero, count: lines.count)
        CTFrameGetLineOrigins(frame, CFRange(location: 0, length: 0), &origins)
        let y = size.height - point.y
        for (line, origin) in zip(lines, origins) {
            var ascent: CGFloat = 0, descent: CGFloat = 0, leading: CGFloat = 0
            let width = CTLineGetTypographicBounds(line, &ascent, &descent, &leading)
            guard y >= origin.y - descent - 4, y <= origin.y + ascent + 4, point.x >= origin.x, point.x <= origin.x + width else { continue }
            let at = CTLineGetStringIndexForPosition(line, CGPoint(x: point.x - origin.x, y: 0))
            guard at != kCFNotFound, at < text.length else { return nil }
            guard let link = text.attribute(.bookLink, at: at, effectiveRange: nil) as? String else { return nil }
            // A raised mark is a note's number; a link in the middle of a line is not.
            return (link, text.attribute(.bookRaised, at: at, effectiveRange: nil) != nil)
        }
        return nil
    }

    // MARK: Pagination

    /// Columns run from the right to the left in a vertical chapter.
    private static func frameAttributes(_ vertical: Bool) -> CFDictionary? {
        vertical ? [kCTFrameProgressionAttributeName: NSNumber(value: CTFrameProgression.rightToLeft.rawValue)] as CFDictionary : nil
    }

    private static func paginate(text: NSAttributedString, framesetter: CTFramesetter, size: CGSize, vertical: Bool, breaks: [Int], headings: [NSRange]) -> [NSRange] {
        let total = text.length
        let path = CGPath(rect: CGRect(origin: .zero, size: size), transform: nil)
        let characters = text.string as NSString
        var pages: [NSRange] = []
        var position = 0
        while position < total {
            let frame = CTFramesetterCreateFrame(framesetter, CFRange(location: position, length: 0), path, frameAttributes(vertical))
            var end = position + max(CTFrameGetVisibleStringRange(frame).length, 1)
            if let forced = breaks.first(where: { $0 > position && $0 < end }) {
                end = forced
            } else if end < total, let heading = headings.last(where: { $0.location < end }),
                      heading.location > position, end <= NSMaxRange(heading) {
                // A heading is not left at the foot of a page with its text on the next.
                end = heading.location
            }
            // What is left after this page is only blank lines: they belong to it.
            if end < total, characters.rangeOfCharacter(from: CharacterSet.whitespacesAndNewlines.inverted, options: [], range: NSRange(location: end, length: total - end)).location == NSNotFound { end = total }
            pages.append(NSRange(location: position, length: end - position))
            position = end
        }
        return pages.isEmpty ? [NSRange(location: 0, length: 0)] : pages
    }
}

// MARK: - Building

/// Sets a chapter in type: blocks of text and pictures in, a `ChapterLayout` out. Slow for a long chapter, so call it off the main thread.
struct ChapterTypesetter {
    let style: BookStyle
    /// The text area of a page.
    let size: CGSize
    let language: String?
    /// How big a picture is in pixels, by its path in the book; nil when it cannot be read.
    let pictureSize: @Sendable (String) -> CGSize?
    /// The book's own fonts, when the reader wants them used.
    let fonts: BookFontBank?

    func layout(index: Int, chapter: BookChapter) -> ChapterLayout {
        let base = CGFloat(style.size)
        let locale = language.flatMap { CFLocaleCreate(nil, CFLocaleCreateCanonicalLanguageIdentifierFromString(nil, $0 as CFString)) }
        let hyphenates = style.hyphenate && locale.map { CFStringIsHyphenationAvailableForLocale($0) } == true
        let out = NSMutableAttributedString()
        var hyphens: [Int] = []
        var breaks: [Int] = []
        var headings: [NSRange] = []
        var source = 0

        // A page that is one picture and nothing else (a comic's page, a cover) fills the page.
        var pictureOnly = false
        var cover: String?
        if chapter.blocks.count == 1, case .image(let path, _) = chapter.blocks[0].kind {
            pictureOnly = true
            cover = path
        }
        for block in chapter.blocks {
            let blockStart = out.length
            if block.pageBreakBefore, blockStart > 0 { breaks.append(blockStart) }
            let paragraph = paragraphStyle(for: block, base: base)
            var lastAttributes: [NSAttributedString.Key: Any] = [.font: style.font.font(size: base, bold: false, italic: false, mono: false), .paragraphStyle: paragraph]
            switch block.kind {
            case .text(let runs):
                for run in runs {
                    let attributes = attributes(for: run, paragraph: paragraph, base: base)
                    lastAttributes = attributes
                    var from = run.text.startIndex
                    let points = hyphenates && !run.mono ? hyphenPoints(in: run.text, locale: locale!) : []
                    for point in points {
                        let at = run.text.utf16.index(run.text.utf16.startIndex, offsetBy: point)
                        out.append(NSAttributedString(string: String(run.text[from..<at]) + "\u{00AD}", attributes: attributes))
                        hyphens.append(source + point)
                        from = at
                    }
                    out.append(NSAttributedString(string: String(run.text[from...]), attributes: attributes))
                    source += run.text.utf16.count
                }
            case .image(let path, let ink):
                let attachment = BookAttachment(kind: .image(path: path, ink: ink), size: fit(pictureSize(path), fill: pictureOnly))
                out.append(placeholder(attachment, paragraph: paragraph, base: base))
                source += 1
            case .rule:
                out.append(placeholder(BookAttachment(kind: .rule, size: CGSize(width: size.width * 0.4, height: base)), paragraph: paragraph, base: base))
                source += 1
            }
            out.append(NSAttributedString(string: "\n", attributes: lastAttributes))
            source += 1
            if block.heading > 0 { headings.append(NSRange(location: blockStart, length: out.length - blockStart)) }
        }
        if chapter.vertical { out.addAttribute(NSAttributedString.Key(kCTVerticalFormsAttributeName as String), value: true, range: NSRange(location: 0, length: out.length)) }
        // Text takes the colour the page is drawn with, so the theme can change without setting the type again.
        out.addAttribute(NSAttributedString.Key(kCTForegroundColorFromContextAttributeName as String), value: true, range: NSRange(location: 0, length: out.length))
        let anchors = Dictionary(chapter.anchors.map { ($0.id, Int($0.offset)) }, uniquingKeysWith: { first, _ in first })
        return ChapterLayout(index: index, text: out, size: size, vertical: chapter.vertical, coverPicture: cover, breaks: breaks, headings: headings, hyphens: hyphens, anchors: anchors)
    }

    /// A picture's size on the page: half its pixels at most, never wider than the page or most of its height. One that cannot be
    /// read still gets a small box, so the text around it keeps its place.
    private func fit(_ pixels: CGSize?, fill: Bool) -> CGSize {
        guard let pixels, pixels.width > 0, pixels.height > 0 else { return CGSize(width: size.width * 0.3, height: size.width * 0.3) }
        if fill {
            let scale = min(size.width / pixels.width, size.height / pixels.height)
            return CGSize(width: (pixels.width * scale).rounded(.down), height: (pixels.height * scale).rounded(.down))
        }
        let scale = min(size.width / pixels.width * 2, size.height * 0.85 / pixels.height * 2, 2) / 2
        return CGSize(width: (pixels.width * scale).rounded(), height: (pixels.height * scale).rounded())
    }

    private func placeholder(_ attachment: BookAttachment, paragraph: NSParagraphStyle, base: CGFloat) -> NSAttributedString {
        var callbacks = CTRunDelegateCallbacks(
            version: kCTRunDelegateVersion1,
            dealloc: { Unmanaged<BookAttachment>.fromOpaque($0).release() },
            getAscent: { Unmanaged<BookAttachment>.fromOpaque($0).takeUnretainedValue().size.height },
            getDescent: { _ in 0 },
            getWidth: { Unmanaged<BookAttachment>.fromOpaque($0).takeUnretainedValue().size.width }
        )
        let delegate = CTRunDelegateCreate(&callbacks, Unmanaged.passRetained(attachment).toOpaque())!
        // A picture is as tall as it is: the line spacing for text would add space above it.
        let tight = paragraph.mutableCopy() as! NSMutableParagraphStyle
        tight.lineHeightMultiple = 1
        return NSAttributedString(string: "\u{FFFC}", attributes: [
            .font: style.font.font(size: base, bold: false, italic: false, mono: false),
            .paragraphStyle: tight,
            .bookAttachment: attachment,
            NSAttributedString.Key(kCTRunDelegateAttributeName as String): delegate,
        ])
    }

    private func paragraphStyle(for block: BookBlock, base: CGFloat) -> NSParagraphStyle {
        let paragraph = NSMutableParagraphStyle()
        let left = min(CGFloat(block.left) * base, size.width * 0.4)
        paragraph.headIndent = left
        paragraph.firstLineHeadIndent = max(0, min(left + CGFloat(block.firstLine) * base, size.width * 0.5))
        paragraph.paragraphSpacingBefore = CGFloat(block.spaceBefore) * base
        paragraph.lineHeightMultiple = block.heading > 0 ? min(CGFloat(style.lineSpacing), 1.25) : CGFloat(style.lineSpacing)
        paragraph.lineBreakMode = .byWordWrapping
        // The hyphen at the end of a broken word is drawn after the line: room is left for it, so it stays inside the column and the
        // right edge is as straight as the left.
        if style.hyphenate { paragraph.tailIndent = -base * 0.3 }
        paragraph.tabStops = [NSTextTab(textAlignment: .left, location: left)]
        paragraph.defaultTabInterval = base * 2
        switch block.align {
        case .center: paragraph.alignment = .center
        case .end: paragraph.alignment = .right
        case .justify, .start:
            paragraph.alignment = justifies(block) ? .justified : .natural
        }
        return paragraph
    }

    /// Justified when the reader asked for it, except headings, tiny blocks, and columns so narrow for the type that justifying
    /// would spread single words across the line letter by letter.
    private func justifies(_ block: BookBlock) -> Bool {
        style.justify && block.heading == 0 && !isRagged(block) && size.width / CGFloat(style.size) >= 14
    }

    /// A block the book set to the left on purpose and short enough that justifying it would only spread it.
    private func isRagged(_ block: BookBlock) -> Bool {
        if case .text(let runs) = block.kind { return runs.reduce(0) { $0 + $1.text.utf16.count } < 3 }
        return true
    }

    private func attributes(for run: BookRun, paragraph: NSParagraphStyle, base: CGFloat) -> [NSAttributedString.Key: Any] {
        let size = base * CGFloat(run.size) * (run.shift == 0 ? 1 : 0.85)
        var font = run.family.flatMap { fonts?.font(family: $0, size: size, bold: run.bold, italic: run.italic) }
            ?? style.font.font(size: size, bold: run.bold, italic: run.italic, mono: run.mono)
        var features: [[UIFontDescriptor.FeatureKey: Int]] = []
        if run.smallCaps { features.append([.type: kLowerCaseType, .selector: kLowerCaseSmallCapsSelector]) }
        if !features.isEmpty { font = UIFont(descriptor: font.fontDescriptor.addingAttributes([.featureSettings: features]), size: size) }
        var attributes: [NSAttributedString.Key: Any] = [.font: font, .paragraphStyle: paragraph]
        if run.shift > 0 { attributes[.bookRaised] = true }
        if run.shift != 0 {
            // A raised or lowered mark (a footnote number): its own baseline, moved up or down from the line's.
            let offset = -CGFloat(run.shift) * (run.shift > 0 ? 0.38 : 0.2) * base
            attributes[NSAttributedString.Key(kCTBaselineClassAttributeName as String)] = kCTBaselineClassRoman
            attributes[NSAttributedString.Key(kCTBaselineInfoAttributeName as String)] = [kCTBaselineClassRoman: offset]
            attributes[NSAttributedString.Key(kCTBaselineReferenceInfoAttributeName as String)] = [kCTBaselineClassRoman: 0]
        }
        if run.underline { attributes[NSAttributedString.Key(kCTUnderlineStyleAttributeName as String)] = NSNumber(value: CTUnderlineStyle.single.rawValue) }
        if run.strike { attributes[.bookStrike] = true }
        if let link = run.link { attributes[.bookLink] = link }
        return attributes
    }

    /// The places in a run's text (UTF-16 offsets) where a long word may be broken with a hyphen.
    private func hyphenPoints(in text: String, locale: CFLocale) -> [Int] {
        let characters = text as CFString
        let length = CFStringGetLength(characters)
        guard length >= 8 else { return [] }
        let tokenizer = CFStringTokenizerCreate(nil, characters, CFRange(location: 0, length: length), kCFStringTokenizerUnitWord, locale)
        var points: [Int] = []
        while CFStringTokenizerAdvanceToNextToken(tokenizer) != [] {
            let word = CFStringTokenizerGetCurrentTokenRange(tokenizer)
            guard word.length >= 8 else { continue }
            var found: [Int] = []
            var index = word.location + word.length
            while index > word.location + 1 {
                let at = CFStringGetHyphenationLocationBeforeIndex(characters, index, word, 0, locale, nil)
                if at == kCFNotFound || at <= word.location { break }
                found.append(at)
                index = at
            }
            points.append(contentsOf: found.reversed())
        }
        return points
    }
}
