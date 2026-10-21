import AkiManga
import SwiftUI

/// Moves a series to the same series on another source: the library entry, its shelves and the chapters read
/// (matched by chapter number) go with it.
struct MigrateSheet: View {
    let ref: SeriesRef
    let moved: () -> Void
    @Environment(AppModel.self) private var model
    @Environment(\.dismiss) private var dismiss
    @State private var target = ""
    @State private var term = ""
    @State private var results: [Card] = []
    @State private var searched = false
    @State private var busy = false
    @State private var didMove = false
    @State private var chosen: Card?
    @State private var chosenTarget = ""
    @State private var message: (title: String, message: String)?

    private var others: [SourceInfo] { model.visibleSources.filter { $0.id != ref.source && $0.canSearch } }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                AkiText("Move to another source", .title)
                if others.isEmpty {
                    AkiEmptyState(title: "No other source", message: "Install another source that can search, then come back.")
                } else {
                    AkiChips(items: others.map { ($0.id, $0.name) }, selection: $target)
                    AkiInput(placeholder: "Find the series there", text: $term, icon: "search", onSubmit: { Task { await search() } })
                    if busy { AkiSpinner().frame(maxWidth: .infinity, minHeight: 80) }
                    else if searched && results.isEmpty { AkiText("Nothing matched on \(model.name(of: target)).", .caption, tone: .muted) }
                    ForEach(results, id: \.id) { card in
                        AkiTap(action: { chosenTarget = target; chosen = card }) { AkiListCard(thumbnail: card.cover, label: model.name(of: target), title: card.title) }
                    }
                }
            }
            .padding(24)
        }
        .onDisappear { if didMove { moved() } }
        .onAppear { term = ref.title; target = others.first?.id ?? "" }
        .onChange(of: target) { results = []; searched = false }
        .akiConfirm("Move to “\(chosen?.title ?? "")”?", message: "It leaves \(model.name(of: ref.source)); your shelves and read chapters carry over.", confirm: "Move", isPresented: Binding(get: { chosen != nil }, set: { if !$0 { chosen = nil } })) {
            if let chosen { Task { await move(to: chosen) } }
        }
        .akiAlert(message?.title ?? "", message: message?.message, isPresented: Binding(get: { message != nil }, set: { if !$0 { message = nil } }))
    }

    private func search() async {
        let asked = target
        busy = true
        defer { busy = false }
        do {
            let found = try await model.engine.search(source: asked, term: term, page: 1, choices: []).items
            guard asked == target else { return }
            results = found
            searched = true
        } catch { message = describe(error) }
    }

    private func move(to card: Card) async {
        guard !busy else { return }
        let target = chosenTarget
        busy = true
        defer { busy = false }
        do {
            // Opening the other series once lets the engine match chapters by number.
            _ = try await model.engine.details(source: target, id: card.id)
            _ = try await model.engine.chapters(source: target, id: card.id)
            _ = try model.engine.migrate(source: ref.source, series: ref.id, toSource: target, toSeries: card.id)
            model.refresh()
            didMove = true
            dismiss()
        } catch { message = describe(error) }
    }
}
