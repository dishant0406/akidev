import AkiManga
import SwiftUI

/// One search sent to every installed source at once; each source's answer appears when it arrives, and one that
/// is slow or broken shows its own state without holding the others back.
@MainActor @Observable
final class GlobalSearchModel {
    enum State { case loading, done([Card]), failed(String) }
    struct Group: Identifiable {
        let source: SourceInfo
        var state: State
        var id: String { source.id }
    }

    var groups: [Group] = []
    private var term = ""

    func search(_ engine: Engine, sources: [SourceInfo], term: String, report: @escaping (String, Error?) -> Void) async {
        self.term = term
        groups = sources.filter(\.canSearch).map { Group(source: $0, state: .loading) }
        await withTaskGroup(of: (String, State, Error?).self) { group in
            for source in groups.map(\.source) {
                group.addTask {
                    do { return (source.id, .done(try await engine.search(source: source.id, term: term, page: 1, choices: []).items), nil) } catch { return (source.id, .failed(describe(error).message), error) }
                }
            }
            for await (id, state, error) in group where self.term == term {
                report(id, error)
                if let at = groups.firstIndex(where: { $0.id == id }) { groups[at].state = state }
            }
        }
    }
}

/// The results of a search across sources, a row of covers per source.
struct GlobalSearchResults: View {
    let model: GlobalSearchModel

    var body: some View {
        VStack(alignment: .leading, spacing: 22) {
            ForEach(model.groups) { group in
                VStack(alignment: .leading, spacing: 10) {
                    AkiText(group.source.name, .heading)
                    switch group.state {
                    case .loading:
                        AkiSpinner().frame(maxWidth: .infinity, minHeight: 60)
                    case .failed(let message):
                        AkiCard(padding: 14) { AkiText(message, .caption, tone: .dim, lines: 3) }
                    case .done(let cards) where cards.isEmpty:
                        AkiText("No results here", .caption, tone: .dim)
                    case .done(let cards):
                        ScrollView(.horizontal, showsIndicators: false) {
                            HStack(alignment: .top, spacing: 14) {
                                ForEach(cards, id: \.id) { card in
                                    AkiLink(value: SeriesRef(source: group.source.id, id: card.id, title: card.title, cover: card.cover)) {
                                        AkiCoverCard(title: card.title, cover: card.cover).frame(width: 130)
                                    }
                                }
                            }
                        }
                        .scrollClipDisabled()
                    }
                }
            }
        }
    }
}
