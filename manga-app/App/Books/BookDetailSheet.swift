import AkiManga
import SwiftUI

/// What the person can do with one book: where they are in it, its highlights and what to change about it.
struct BookDetailSheet: View {
    let book: BookInfo
    let marks: [BookHighlight]
    let read: () -> Void
    let finish: () -> Void
    let reset: () -> Void
    let categories: () -> Void
    let chooseCover: () -> Void
    let ownCover: () -> Void
    let remove: () -> Void

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                HStack(alignment: .top, spacing: 16) {
                    AkiImage(url: book.cover, placeholder: 1.5, fill: true)
                        .frame(width: 104)
                        .clipShape(RoundedRectangle(cornerRadius: Aki.thumbRadius, style: .continuous))
                    VStack(alignment: .leading, spacing: 6) {
                        AkiText(book.title, .heading, lines: 4)
                        if !book.authors.isEmpty { AkiText(book.authors.joined(separator: ", "), .caption, tone: .muted, lines: 3) }
                        AkiText(facts, .caption, tone: .muted, lines: 3)
                        if book.openedMs != nil {
                            AkiProgressBar(fraction: book.finished ? 1 : book.progress).padding(.top, 4)
                            AkiText(book.finished ? "Finished" : "\(Int((book.progress * 100).rounded()))% read", .caption, tone: .muted)
                        }
                    }
                    Spacer(minLength: 0)
                }
                AkiButton(book.openedMs == nil ? "Start reading" : book.finished ? "Read again" : "Continue", icon: "play-solid", variant: .primary, size: .large, fullWidth: true, action: read)
                VStack(spacing: 10) {
                    if !book.finished { AkiButton("Mark as finished", icon: "check", fullWidth: true, action: finish) }
                    if book.openedMs != nil { AkiButton("Mark as not started", icon: "restore", fullWidth: true, action: reset) }
                    AkiButton("Categories", icon: "folder", fullWidth: true, action: categories)
                    AkiButton("Choose a cover picture", icon: "image", fullWidth: true, action: chooseCover)
                    if book.customCover { AkiButton("Use the book's own cover", icon: "restore", fullWidth: true, action: ownCover) }
                }
                if !marks.isEmpty {
                    AkiSection(title: "Highlights \(marks.count)") {
                        ForEach(marks, id: \.id) { mark in
                            AkiCard(padding: 14) {
                                VStack(alignment: .leading, spacing: 6) {
                                    AkiText("“\(mark.quote)”", .body, lines: 4)
                                    if let note = mark.note, !note.isEmpty { AkiText(note, .caption, tone: .muted, lines: 4) }
                                }
                            }
                        }
                    }
                }
                AkiButton("Remove from this phone", icon: "trash", variant: .destructive, fullWidth: true, action: remove)
            }
            .padding(24)
        }
    }

    private var facts: String {
        var parts: [String] = []
        if let language = book.language { parts.append(language.uppercased()) }
        parts.append(ByteCountFormatter.string(fromByteCount: Int64(book.bytes), countStyle: .file))
        parts.append("Added \(shortDate(book.addedMs) ?? "")")
        return parts.joined(separator: " · ")
    }
}
