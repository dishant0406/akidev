import AkiManga
import SwiftUI

/// What the home feed shows besides the popular list: each source's newest titles, and with every source chosen, a
/// shelf of popular and a shelf of new for each.
@MainActor @Observable
final class FeedModel {
    struct Shelf: Identifiable {
        let id: String
        let title: String
        let subtitle: String?
        let source: String
        var items: [Card]
    }

    /// One source's new releases (the popular list comes from the browse model, which pages it).
    var latest: [Card] = []
    /// Every source's shelves, for the "All" view.
    var shelves: [Shelf] = []
    var loading = false
    private var key = ""

    func loadLatest(_ engine: Engine, report: (String, Error?) -> Void, source: SourceInfo) async {
        key = "latest|\(source.id)"
        guard source.canLatest else { return latest = [] }
        if let kept = engine.rememberedListing(source: source.id, latest: true) { latest = Array(kept.items.prefix(14)) }
        let mine = key
        do {
            let fresh = try await engine.latest(source: source.id, page: 1)
            guard key == mine else { return }
            latest = Array(fresh.items.prefix(14))
        } catch {
            guard key == mine else { return }
            report(source.id, error)
        }
    }

    /// Asks every source at once, showing what was remembered first and each answer as it arrives.
    func loadAll(_ engine: Engine, report: @escaping (String, Error?) -> Void, sources: [SourceInfo]) async {
        key = "all|" + sources.map(\.id).joined(separator: ",")
        let mine = key
        loading = true
        defer { if key == mine { loading = false } }
        shelves = sources.flatMap { source -> [Shelf] in
            var found: [Shelf] = []
            if let kept = engine.rememberedListing(source: source.id, latest: false) { found.append(Shelf(id: "\(source.id)|popular", title: source.name, subtitle: "Popular", source: source.id, items: Array(kept.items.prefix(14)))) }
            if source.canLatest, let kept = engine.rememberedListing(source: source.id, latest: true) { found.append(Shelf(id: "\(source.id)|latest", title: source.name, subtitle: "New releases", source: source.id, items: Array(kept.items.prefix(14)))) }
            return found
        }
        await withTaskGroup(of: (SourceInfo, Bool, Result<[Card], Error>).self) { group in
            for source in sources {
                group.addTask { (source, false, await Result { try await engine.popular(source: source.id, page: 1).items }) }
                if source.canLatest { group.addTask { (source, true, await Result { try await engine.latest(source: source.id, page: 1).items }) } }
            }
            for await (source, isLatest, result) in group where key == mine {
                let id = "\(source.id)|\(isLatest ? "latest" : "popular")"
                switch result {
                case .success(let cards):
                    report(source.id, nil)
                    let shelf = Shelf(id: id, title: source.name, subtitle: isLatest ? "New releases" : "Popular", source: source.id, items: Array(cards.prefix(14)))
                    if let at = shelves.firstIndex(where: { $0.id == id }) { shelves[at] = shelf } else { shelves.append(shelf) }
                case .failure(let error):
                    report(source.id, error)
                }
            }
        }
        let order = sources.map(\.id)
        // Sources in the order of the chips; each one's popular shelf before its new releases.
        func rank(_ shelf: Shelf) -> Int { (order.firstIndex(of: shelf.source) ?? 0) * 2 + (shelf.id.hasSuffix("latest") ? 1 : 0) }
        shelves.sort { rank($0) < rank($1) }
    }
}

private extension Result where Failure == Error {
    init(_ work: () async throws -> Success) async {
        do { self = .success(try await work()) } catch { self = .failure(error) }
    }
}
