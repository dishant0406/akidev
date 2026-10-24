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
                AkiText(number(chapter), .heading, tone: reading ? .onPrimary : chapter.finished ? .muted : .text, lines: 1)
                    .monospacedDigit()
                    .minimumScaleFactor(0.6)
                    .frame(width: 52, height: 52)
                    .background(reading ? Aki.onPrimary.opacity(0.10) : Aki.secondary, in: RoundedRectangle(cornerRadius: 16, style: .continuous))
                VStack(alignment: .leading, spacing: 3) {
                    AkiText(chapterName(number: chapter.number, title: chapter.title), .bodyStrong, tone: reading ? .onPrimary : chapter.finished ? .muted : .text, lines: 1)
                    if let line = detail(chapter, reading: reading) {
                        AkiText(line, .caption, tone: reading ? .onPrimary : .muted, lines: 1).opacity(reading ? 0.7 : 1)
                    }
                }
                Spacer(minLength: 8)
                trailing(chapter, reading: reading)
            }
            .padding(.horizontal, 12).padding(.vertical, 10)
            .background(reading ? Aki.primary : Aki.surface, in: RoundedRectangle(cornerRadius: 22, style: .continuous))
            .contentShape(RoundedRectangle(cornerRadius: 22, style: .continuous))
        }
    }

    @ViewBuilder private func trailing(_ chapter: ChapterEntry, reading: Bool) -> some View {
        if reading {
            AkiText("Reading", .captionStrong, tone: .onPrimary)
        } else if chapter.finished {
            AkiIcon("check-solid", size: 18, tone: .muted)
        } else if kept.contains(chapter.id) {
            AkiIcon("download", size: 18, tone: .muted)
        }
    }

    /// The date and where the person is in it, as one quiet line.
    private func detail(_ chapter: ChapterEntry, reading: Bool) -> String? {
        var parts: [String] = []
        if reading { parts.append("Page \(model.page + 1)") } else if let page = chapter.page { parts.append("Page \(page + 1)") }
        if let date = shortDate(chapter.dateMs) { parts.append(date) }
        if kept.contains(chapter.id), chapter.finished { parts.append("Downloaded") }
        return parts.isEmpty ? nil : parts.joined(separator: " · ")
    }

    /// "46", "12.5": the chapter's number, or a dash when the source gave none.
    private func number(_ chapter: ChapterEntry) -> String {
        chapter.number.map { $0.formatted(.number.precision(.fractionLength(0...1))) } ?? "–"
    }

    private func load() {
        chapters = (try? model.engine.cachedChapters(source: model.ref.source, series: model.ref.id)).flatMap { $0.isEmpty ? nil : $0 } ?? model.chapters
        kept = Set(((try? model.engine.downloads()) ?? []).filter { $0.source == model.ref.source && $0.series == model.ref.id && $0.state == "done" }.map(\.chapter))
    }
}
