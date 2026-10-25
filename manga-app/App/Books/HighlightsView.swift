import AkiManga
import SwiftUI

struct HighlightsRoute: Hashable {}

/// Everything the person highlighted or wrote a note on, in every book, newest first. Tapping one opens the book there.
struct HighlightsView: View {
    let open: (BookHighlightIn) -> Void
    @Environment(AppModel.self) private var model
    @Environment(\.dismiss) private var dismiss
    @State private var found: [BookHighlightIn] = []

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                AkiBackHeader(title: "Highlights", subtitle: found.count == 1 ? "1 highlight" : "\(found.count) highlights", back: { dismiss() })
                if found.isEmpty {
                    AkiEmptyState(title: "No highlights yet", message: "Press and hold a word in a book, drag to the end of what you want, and choose Highlight or Note.").frame(minHeight: 300)
                }
                ForEach(found, id: \.highlight.id) { item in
                    AkiTap(action: { dismiss(); open(item) }) {
                        AkiCard(padding: 14) {
                            VStack(alignment: .leading, spacing: 6) {
                                AkiText("“\(item.highlight.quote)”", .body, lines: 5)
                                if let note = item.highlight.note, !note.isEmpty { AkiText(note, .caption, tone: .muted, lines: 4) }
                                AkiText(item.title, .captionStrong, tone: .muted, lines: 1)
                            }
                            .frame(maxWidth: .infinity, alignment: .leading)
                        }
                    }
                }
            }
            .akiPage(narrow: true)
        }
        .background(AkiBackdrop())
        .akiNoNavigationBar()
        .hidesTabBar()
        .onAppear {
            let engine = model.engine
            Task { found = await Task.detached { (try? engine.recentHighlights(limit: 500)) ?? [] }.value }
        }
    }
}
