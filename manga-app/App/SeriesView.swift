import AkiManga
import SwiftUI

struct ReaderTarget: Identifiable, Hashable {
    let chapterID: String
    let page: Int
    /// How far down that page, 0 to 1.
    var offset: Double = 0
    var id: String { chapterID }
}

struct SeriesView: View {
    let ref: SeriesRef
    @Environment(AppModel.self) private var model
    @State private var detail: Detail?
    @State private var chapters: [ChapterEntry] = []
    @State private var saved = false
    @State private var loading = true
    @State private var error: (title: String, message: String)?
    @State private var target: ReaderTarget?
    @State private var expanded = false

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                header
                if let description = detail?.description {
                    Text(description).font(.subheadline).foregroundStyle(Aki.dim).lineLimit(expanded ? nil : 4)
                        .frame(maxWidth: .infinity, alignment: .leading).onTapGesture { withAnimation { expanded.toggle() } }
                }
                if let error, chapters.isEmpty {
                    StateCard(title: error.title, message: error.message, action: ("Try again", { Task { await load() } })).frame(minHeight: 260)
                } else {
                    actions
                    chapterList
                }
            }.padding(16)
        }
        .background(Aki.canvas)
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Button(saved ? "Saved" : "Save", systemImage: saved ? "bookmark.fill" : "bookmark") { Task { await toggleSaved() } }
            }
        }
        .task {
            await load()
            #if DEBUG
            if UserDefaults.standard.bool(forKey: "autoContinue"), target == nil { target = resumable?.target }
            #endif
        }
        .fullScreenCover(item: $target, onDismiss: { Task { await refreshProgress() } }) { target in
            ReaderView(model: ReaderModel(engine: model.engine, ref: ref, chapters: chapters, start: target))
        }
    }

    private var header: some View {
        HStack(alignment: .top, spacing: 16) {
            RemoteImage(url: detail?.cover ?? ref.cover, placeholder: 1.5, fill: true)
                .frame(width: 112).clipShape(RoundedRectangle(cornerRadius: Aki.radius[2]))
            VStack(alignment: .leading, spacing: 8) {
                Eyebrow([model.name(of: ref.source), statusText].compactMap { $0 }.joined(separator: " · "))
                Text(detail?.title ?? ref.title).font(Aki.title(26)).foregroundStyle(Aki.text)
                if let authors = detail?.authors, !authors.isEmpty {
                    Text(authors.joined(separator: ", ")).font(.subheadline).foregroundStyle(Aki.dim)
                }
                if let tags = detail?.tags, !tags.isEmpty {
                    Text(tags.prefix(4).joined(separator: " · ")).font(.caption).foregroundStyle(Aki.faint)
                }
            }
        }
    }

    private var statusText: String? {
        switch detail?.status {
        case .ongoing: "Ongoing"
        case .completed: "Completed"
        case .hiatus: "Hiatus"
        case .cancelled: "Cancelled"
        default: nil
        }
    }

    private var actions: some View {
        HStack {
            if let next = resumable {
                Button { target = next.target } label: {
                    Label(next.label, systemImage: "play.fill").frame(maxWidth: .infinity)
                }.buttonStyle(.glassProminent)
            } else if loading {
                ProgressView().frame(maxWidth: .infinity)
            }
        }
    }

    private var chapterList: some View {
        LazyVStack(alignment: .leading, spacing: 0) {
            Eyebrow("\(chapters.count) chapters")
            Divider().overlay(Aki.line).padding(.vertical, 8)
            ForEach(chapters, id: \.id) { chapter in
                Button { target = ReaderTarget(chapterID: chapter.id, page: Int(chapter.page ?? 0)) } label: {
                    HStack {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(label(chapter)).font(.body).foregroundStyle(chapter.finished ? Aki.faint : Aki.text)
                            Text([date(chapter.dateMs), chapter.scanlator].compactMap { $0 }.joined(separator: " · ")).font(.caption).foregroundStyle(Aki.faint)
                        }
                        Spacer()
                        if let page = chapter.page, !chapter.finished { Text("p. \(page + 1)").font(.caption.monospacedDigit()).foregroundStyle(Aki.accent) }
                        if chapter.finished { Image(systemName: "checkmark").font(.caption).foregroundStyle(Aki.faint) }
                    }.padding(.vertical, 10).contentShape(Rectangle())
                }.buttonStyle(.plain)
                Divider().overlay(Aki.line)
            }
        }
    }

    private func label(_ chapter: ChapterEntry) -> String {
        if let title = chapter.title { return title }
        if let number = chapter.number { return "Chapter \(number.formatted(.number.precision(.fractionLength(0...2))))" }
        return "Chapter"
    }

    private func date(_ ms: Int64?) -> String? {
        guard let ms else { return nil }
        return Date(timeIntervalSince1970: Double(ms) / 1000).formatted(date: .abbreviated, time: .omitted)
    }

    private struct Resumable {
        let target: ReaderTarget
        let label: String
    }

    private var resumable: Resumable? {
        guard !chapters.isEmpty else { return nil }
        let resume = try? model.engine.resume(source: ref.source, series: ref.id)
        let started = chapters.contains { $0.finished || $0.page != nil }
        guard let resume, let index = chapters.firstIndex(where: { $0.id == resume.chapter }) else { return nil }
        let name = label(chapters[index])
        return Resumable(target: ReaderTarget(chapterID: resume.chapter, page: Int(resume.page), offset: resume.offset), label: started ? "Continue · \(name)" : "Start · \(name)")
    }

    private func load() async {
        loading = true
        error = nil
        saved = (try? model.engine.inLibrary(source: ref.source, id: ref.id)) ?? false
        chapters = (try? model.engine.cachedChapters(source: ref.source, series: ref.id)) ?? []
        async let fetchedDetail = model.engine.details(source: ref.source, id: ref.id)
        async let fetchedChapters = model.engine.chapters(source: ref.source, id: ref.id)
        do { detail = try await fetchedDetail } catch { if detail == nil { self.error = describe(error) } }
        do { chapters = try await fetchedChapters.entries } catch { if chapters.isEmpty { self.error = describe(error) } }
        loading = false
    }

    private func refreshProgress() async {
        chapters = (try? model.engine.cachedChapters(source: ref.source, series: ref.id)) ?? chapters
        model.refresh()
    }

    private func toggleSaved() async {
        do {
            try await model.engine.setInLibrary(source: ref.source, id: ref.id, saved: !saved)
            saved.toggle()
            model.refresh()
        } catch { self.error = describe(error) }
    }
}
