import SwiftUI
import UIKit

/// A stretch of a chapter's text, in the book's own positions.
struct BookSpan: Equatable {
    var chapter: Int
    var start: Int
    var end: Int
}

/// A highlight as the page draws it.
struct BookMarkSpan: Equatable {
    let id: Int64
    let span: BookSpan
    let colour: Int
    let hasNote: Bool
}

/// What is drawn over the text of a page besides the text: highlights, the selection and the place a search found.
struct BookOverlays: Equatable {
    var marks: [BookMarkSpan] = []
    var selection: BookSpan?
    var found: BookSpan?
}

/// What the reader can do with a selection or a highlight, from the menu that opens over it.
enum BookAction {
    case highlight(colour: Int)
    case note
    case copy
    case remove
}

/// The four highlight colours.
enum HighlightColour {
    static let names = ["Yellow", "Green", "Blue", "Pink"]
    static let colours: [UIColor] = [UIColor(hex: 0xFFD21F), UIColor(hex: 0x6CCB5F), UIColor(hex: 0x5AA9FF), UIColor(hex: 0xFF7BAC)]

    /// The colour of a selection and its handles.
    static let selection = UIColor(hex: 0x4C8DFF)

    static func fill(_ index: Int) -> UIColor { colours[min(max(index, 0), colours.count - 1)].withAlphaComponent(0.42) }

    /// A small round swatch for a menu.
    static func swatch(_ index: Int) -> UIImage? {
        UIImage(systemName: "circle.fill")?.withTintColor(colours[min(max(index, 0), colours.count - 1)], renderingMode: .alwaysOriginal)
    }
}

/// A handle at the end of a selection: a dot on a stem, dragged to change where the selection stops.
final class BookHandle: UIView {
    /// The handle at the start hangs its dot above the text, the one at the end below it.
    let atStart: Bool

    init(atStart: Bool) {
        self.atStart = atStart
        super.init(frame: CGRect(x: 0, y: 0, width: 44, height: 44))
        backgroundColor = .clear
        isOpaque = false
    }

    required init?(coder: NSCoder) { fatalError() }

    override func draw(_ rect: CGRect) {
        HighlightColour.selection.setFill()
        let dot = CGRect(x: bounds.midX - 6, y: atStart ? 6 : bounds.height - 18, width: 12, height: 12)
        UIBezierPath(ovalIn: dot).fill()
        UIBezierPath(rect: CGRect(x: bounds.midX - 1, y: atStart ? 14 : 0, width: 2, height: bounds.height - 14)).fill()
    }
}

/// A small disc in a highlight's colour.
struct HighlightDot: View {
    let colour: Int

    var body: some View {
        Circle().fill(Color(uiColor: HighlightColour.colours[min(max(colour, 0), HighlightColour.colours.count - 1)])).frame(width: 14, height: 14)
    }
}
