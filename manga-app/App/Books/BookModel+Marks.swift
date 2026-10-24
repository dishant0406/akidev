import AkiManga
import SwiftUI

/// A footnote shown in a card, with where it is in the book.
struct Footnote: Identifiable {
    let id = UUID()
    let text: String
    let link: String
}

/// A highlight being given a note.
struct NoteDraft: Identifiable {
    let id: Int64
    let quote: String
    var text: String
}

extension BookModel {
    var overlays: BookOverlays {
        BookOverlays(
            marks: marks.map { BookMarkSpan(id: $0.id, span: BookSpan(chapter: Int($0.chapter), start: Int($0.start), end: Int($0.end)), colour: Int($0.colour), hasNote: $0.note != nil) },
            selection: selection,
            found: found,
            speaking: speaking
        )
    }

    func overlaysChanged() { NotificationCenter.default.post(name: .bookOverlaysChanged, object: nil) }

    func select(_ span: BookSpan?) {
        guard span != selection else { return }
        selection = span
    }

    /// Does what the menu over a selection or a highlight asked for.
    func perform(_ action: BookAction, on span: BookSpan, mark: Int64?, words: String) {
        selection = nil
        switch action {
        case .highlight(let colour):
            if let mark, let old = marks.first(where: { $0.id == mark }) {
                try? engine.setBookMark(id: mark, colour: UInt8(colour), note: old.note)
            } else {
                _ = try? engine.addBookMark(book: info.id, chapter: UInt32(span.chapter), start: UInt32(span.start), end: UInt32(span.end), colour: UInt8(colour), quote: words)
            }
        case .note:
            var id = mark
            if id == nil { id = try? engine.addBookMark(book: info.id, chapter: UInt32(span.chapter), start: UInt32(span.start), end: UInt32(span.end), colour: 0, quote: words) }
            reloadMarks()
            if let id, let made = marks.first(where: { $0.id == id }) { noteDraft = NoteDraft(id: id, quote: made.quote, text: made.note ?? "") }
            return
        case .copy:
            UIPasteboard.general.string = words
            return
        case .remove:
            if let mark { try? engine.removeBookMark(id: mark) }
        }
        reloadMarks()
    }

    func saveNote(_ draft: NoteDraft) {
        if let mark = marks.first(where: { $0.id == draft.id }) { try? engine.setBookMark(id: draft.id, colour: mark.colour, note: draft.text) }
        reloadMarks()
    }

    func remove(_ mark: BookHighlight) {
        try? engine.removeBookMark(id: mark.id)
        reloadMarks()
    }

    func reloadMarks() { marks = (try? engine.bookMarks(book: info.id)) ?? marks }

    /// Goes to a highlight.
    func open(_ mark: BookHighlight) {
        trail = []
        go(to: Spot(chapter: Int(mark.chapter), offset: Int(mark.start), highlight: Int(mark.end - mark.start)))
    }

    /// Looks for words in the whole book: the finds in reading order, or if the words are nowhere, the nearest words.
    func search(_ query: String) async -> [BookHit] {
        let engine = engine, id = info.id
        return await Task.detached(priority: .userInitiated) { (try? engine.searchBook(id: id, query: query)) ?? [] }.value
    }

    /// Goes to a place a search found.
    func open(_ hit: BookHit) {
        trail = []
        go(to: Spot(chapter: Int(hit.chapter), offset: Int(hit.offset), highlight: Int(hit.length)))
    }

    /// The pages around the book itself (copyright, dedication, notes, colophon), in reading order, with a title each: the contents' own
    /// title for the page when it has one, the first line of the page when not.
    var aboutPages: [Int] {
        sections.indices.filter { [.front, .back].contains(sections[$0]) }
    }

    /// A chapter's text as plain paragraphs, for a page that is only to be read (a copyright page, notes).
    func plainText(ofChapter chapter: Int) async -> String {
        let reader = reader
        let content = await Task.detached(priority: .userInitiated) { try? reader.chapter(index: UInt32(chapter)) }.value
        return (content?.blocks ?? []).compactMap { block in
            guard case .text(let runs) = block.kind else { return nil }
            let text = runs.map(\.text).joined().replacingOccurrences(of: "\u{2028}", with: "\n").replacingOccurrences(of: "\t", with: " ")
            return text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? nil : text
        }.joined(separator: "\n\n")
    }

    /// The title of a page: its entry in the contents, or the first line of its text.
    func pageTitle(_ chapter: Int, text: String) -> String {
        contents.first { Int($0.chapter) == chapter }?.title ?? String(text.split(separator: "\n").first?.prefix(60) ?? "Page \(chapter + 1)")
    }

    /// The title of the part of the book a chapter falls in.
    func title(ofChapter chapter: Int) -> String {
        (contents.last { Int($0.chapter) <= chapter }?.title) ?? info.title
    }

    /// The highlights and notes as a Markdown file in the temporary folder, for the share sheet; nil when there are none.
    func exportMarkdown() -> URL? {
        guard !marks.isEmpty else { return nil }
        let name = info.title.replacingOccurrences(of: "/", with: "-")
        let file = FileManager.default.temporaryDirectory.appendingPathComponent("\(name) highlights.md")
        return (try? markdown().write(to: file, atomically: true, encoding: .utf8)) != nil ? file : nil
    }

    private func markdown() -> String {
        var lines = ["# \(info.title)"]
        if !info.authors.isEmpty { lines.append("*\(info.authors.joined(separator: ", "))*") }
        var part: String?
        for mark in marks {
            let heading = title(ofChapter: Int(mark.chapter))
            if heading != part {
                lines += ["", "## \(heading)"]
                part = heading
            }
            lines += ["", "> " + mark.quote.replacingOccurrences(of: "\n", with: "\n> ")]
            if let note = mark.note { lines += ["", note] }
        }
        return lines.joined(separator: "\n") + "\n"
    }
}
