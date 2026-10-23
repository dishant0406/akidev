import AkiManga
import SwiftUI

/// Moves a series to the same series on another source: the library entry, its shelves and the chapters read
/// (matched by chapter number) go with it. It looks for the series by its title as soon as it opens and again whenever
/// another source is chosen, so the usual move is: open, pick the source, tap the match.
struct MigrateSheet: View {
    let ref: SeriesRef
    let moved: () -> Void
    @Environment(AppModel.self) private var model
    @Environment(\.dismiss) private var dismiss
    @State private var target = ""
    @State private var term = ""
    @State private var results: [Card] = []
    @State private var searching = false
    @State private var failure: (title: String, message: String)?
    @State private var busy = false
    @State private var didMove = false
    @State private var chosen: Card?
    @State private var problem: (title: String, message: String)?

    private var others: [SourceInfo] { model.visibleSources.filter { $0.id != ref.source && $0.canSearch } }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 18) {
                VStack(alignment: .leading, spacing: 6) {
                    AkiText("Move to another source", .title)
                    AkiText("Find “\(ref.title)” on another source. Its shelves and the chapters you read go with it.", .caption, tone: .muted)
                }
                if others.isEmpty {
                    AkiEmptyState(title: "No other source", message: "Install another source that can search, then come back.").frame(minHeight: 260)
                } else {
                    AkiMenuPill(icon: "sources", selection: $target, options: others.map { ($0.id, $0.name) })
                    AkiInput(placeholder: "Search \(model.name(of: target))", text: $term, icon: "search", onSubmit: { Task { await search() } })
                    results(for: target)
                }
            }
            .padding(24)
        }
        .onDisappear { if didMove { moved() } }
        .onAppear { term = ref.title; if target.isEmpty { target = others.first?.id ?? "" } }
        // Opening the sheet, and choosing another source, each look for the title straight away.
        .task(id: target) { if !target.isEmpty { await search() } }
        .akiConfirm("Move to “\(chosen?.title ?? "")”?", message: "It leaves \(model.name(of: ref.source)) and moves to \(model.name(of: target)); your shelves and read chapters carry over.", confirm: "Move", isPresented: Binding(get: { chosen != nil }, set: { if !$0 { chosen = nil } })) {
            if let chosen { Task { await move(to: chosen) } }
        }
        .akiAlert(problem?.title ?? "", message: problem?.message, isPresented: Binding(get: { problem != nil }, set: { if !$0 { problem = nil } }))
    }

    @ViewBuilder private func results(for source: String) -> some View {
        if searching {
            AkiSpinner().frame(maxWidth: .infinity, minHeight: 140)
        } else if let failure {
            AkiEmptyState(title: failure.title, message: failure.message, action: ("Try again", { Task { await search() } })).frame(minHeight: 240)
        } else if results.isEmpty {
            AkiEmptyState(title: "Nothing found", message: "\(model.name(of: source)) has nothing called “\(term)”. Try a shorter title, or another source.").frame(minHeight: 240)
        } else {
            AkiText("Tap the one that is the same series", .captionStrong, tone: .muted)
            LazyVStack(spacing: 12) {
                ForEach(results, id: \.id) { card in
                    AkiTap(action: { chosen = card }) {
                        AkiListCard(thumbnail: card.cover, label: model.name(of: source), title: card.title, badges: [chaptersLabel(card.chapters)].compactMap { $0 }) {
                            AkiIcon("forward", size: 13, tone: .muted)
                        }
                    }
                }
            }
        }
    }

    private func search() async {
        let asked = target
        let text = term.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !asked.isEmpty else { return }
        searching = true
        failure = nil
        results = []
        do {
            let found = try await model.engine.search(source: asked, term: text, page: 1, choices: []).items
            guard asked == target else { return }
            results = found
        } catch {
            guard asked == target else { return }
            failure = describe(error)
        }
        searching = false
    }

    private func move(to card: Card) async {
        guard !busy else { return }
        let to = target
        busy = true
        defer { busy = false }
        do {
            // Opening the other series once lets the engine match chapters by number.
            _ = try await model.engine.details(source: to, id: card.id)
            _ = try await model.engine.chapters(source: to, id: card.id)
            _ = try model.engine.migrate(source: ref.source, series: ref.id, toSource: to, toSeries: card.id)
            model.refresh()
            didMove = true
            dismiss()
        } catch { problem = describe(error) }
    }
}
