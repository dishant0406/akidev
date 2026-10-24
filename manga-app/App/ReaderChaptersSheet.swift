import AkiManga
import SwiftUI

/// The series' chapters, from inside the reader: tap one to read it, without going back to the series page. The chapter being
/// read is yellow and the list opens on it; read chapters are dimmed, and the ones kept on the device say so.
struct ReaderChaptersSheet: View {
    let model: ReaderModel
    let open: (ChapterEntry) -> Void
    /// Read state as it is now: the reader's own list was made when it opened.
    @State private var chapters: [ChapterEntry] = []
    @State private var kept: Set<String> = []

    var body: some View {
        ScrollViewReader { proxy in
            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    VStack(alignment: .leading, spacing: 4) {
                        AkiText("Chapters", .title)
                        AkiText("\(model.ref.title) · \(chapters.count)", .caption, tone: .muted, lines: 1)
                    }
                    LazyVStack(spacing: 10) {
                        ForEach(chapters, id: \.id) { chapter in
                            row(chapter).id(chapter.id)
                        }
                    }
                }
                .padding(.horizontal, 20).padding(.top, 24).padding(.bottom, 32)
            }
            .onAppear {
                load()
                // Opened on the chapter being read, in the middle of the list.
                DispatchQueue.main.async { proxy.scrollTo(model.current, anchor: .center) }
            }
        }
    }

    private func row(_ chapter: ChapterEntry) -> some View {
        let reading = chapter.id == model.current
        return AkiTap(action: { open(chapter) }) {
            HStack(spacing: 14) {
                status(chapter, reading: reading)
                VStack(alignment: .leading, spacing: 3) {
                    AkiText(chapterName(number: chapter.number, title: chapter.title), .bodyStrong, tone: reading ? .onPrimary : chapter.finished ? .muted : .text, lines: 1)
                    if let date = shortDate(chapter.dateMs) {
                        AkiText(date, .caption, tone: reading ? .onPrimary : .muted, lines: 1).opacity(reading ? 0.7 : 1)
                    }
                }
                Spacer(minLength: 8)
                if kept.contains(chapter.id) { AkiIcon("download", size: 18, tone: reading ? .onPrimary : .muted) }
            }
            .padding(.horizontal, 12).padding(.vertical, 10)
            .background(reading ? Aki.primary : Aki.surface, in: RoundedRectangle(cornerRadius: 22, style: .continuous))
            .contentShape(RoundedRectangle(cornerRadius: 22, style: .continuous))
        }
    }

    /// Where the person stands with a chapter, as one round icon: playing now, read, begun, or not yet opened. A fixed size,
    /// so a long number or title can never push it about.
    private func status(_ chapter: ChapterEntry, reading: Bool) -> some View {
        ZStack {
            Circle().fill(reading ? Aki.onPrimary : Aki.secondary)
            if reading {
                AkiIcon("play-solid", size: 16, tone: .onMedia)
            } else if chapter.finished {
                AkiIcon("check-solid", size: 18, tone: .muted)
            } else if chapter.page != nil {
                AkiIcon("play", size: 16, tone: .text)
            } else {
                Circle().fill(Aki.text.opacity(0.85)).frame(width: 8, height: 8)
            }
        }
        .frame(width: 44, height: 44)
        .accessibilityLabel(reading ? "Reading now" : chapter.finished ? "Read" : chapter.page != nil ? "Started" : "Not read")
    }

    private func load() {
        chapters = (try? model.engine.cachedChapters(source: model.ref.source, series: model.ref.id)).flatMap { $0.isEmpty ? nil : $0 } ?? model.chapters
        kept = Set(((try? model.engine.downloads()) ?? []).filter { $0.source == model.ref.source && $0.series == model.ref.id && $0.state == "done" }.map(\.chapter))
    }
}
