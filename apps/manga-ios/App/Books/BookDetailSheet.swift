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
    let share: () -> Void
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
                        AkiText(book.title, .heading).accessibilityAddTraits(.isHeader)
                        if !book.authors.isEmpty { AkiText(book.authors.joined(separator: ", "), .caption, tone: .muted) }
                        AkiText(facts, .caption, tone: .muted, lines: 3)
                        if book.openedMs != nil {
                            AkiProgressBar(fraction: book.finished ? 1 : book.progress).padding(.top, 4).accessibilityHidden(true)
                            AkiText(book.finished ? Copy.Library.finished : Copy.Books.percentRead(bookPercent(book.progress)), .caption, tone: .muted, digits: true)
                        }
                    }
                    Spacer(minLength: 0)
                }
                AkiButton(book.openedMs == nil ? Copy.Books.startReading : book.finished ? Copy.Books.readAgain : Copy.Books.continueButton, icon: "play-solid", variant: .primary, size: .large, fullWidth: true, action: read)
                VStack(spacing: 10) {
                    if !book.finished { AkiButton(Copy.Books.markFinished, icon: "check", fullWidth: true, action: finish) }
                    if book.openedMs != nil { AkiButton(Copy.Books.markNotStarted, icon: "restore", fullWidth: true, action: reset) }
                    AkiButton(Copy.Books.shareWithFriend, icon: "share", fullWidth: true, action: share)
                    AkiButton(Copy.Library.changeCategories, icon: "folder", fullWidth: true, action: categories)
                    AkiButton(Copy.Books.chooseCoverPicture, icon: "image", fullWidth: true, action: chooseCover)
                    if book.customCover { AkiButton(Copy.Books.useOwnCover, icon: "restore", fullWidth: true, action: ownCover) }
                }
                if !marks.isEmpty {
                    AkiSection(title: Copy.Books.highlightsCount(marks.count)) {
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
                AkiButton(Copy.Books.deleteBook, icon: "trash", variant: .destructive, fullWidth: true, action: remove)
            }
            .padding(24)
        }
    }

    private var facts: String {
        var parts: [String] = []
        if let language = book.language { parts.append(language.uppercased()) }
        parts.append(bookSize(book.bytes))
        if let added = shortDate(book.addedMs) { parts.append(Copy.Books.addedDate(added)) }
        return parts.joined(separator: " · ")
    }
}
