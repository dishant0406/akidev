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

    func loadLatest(_ engine: Engine, report: (String, Error?) -> Void, source: SourceInfo, force: Bool = false) async {
        key = "latest|\(source.id)"
        guard source.canLatest else { return latest = [] }
        // A listing kept in the last ten minutes is shown and the site is not asked again, unless the person pulled to refresh.
        if !force, let fresh = await Task.detached(operation: { engine.freshListing(source: source.id, latest: true) }).value {
            latest = Array(fresh.items.prefix(14))
            return
        }
        if let kept = await Task.detached(operation: { engine.rememberedListing(source: source.id, latest: true) }).value { latest = Array(kept.items.prefix(14)) }
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
    func loadAll(_ engine: Engine, report: @escaping (String, Error?) -> Void, sources: [SourceInfo], force: Bool = false) async {
        key = "all|" + sources.map(\.id).joined(separator: ",")
        let mine = key
        loading = true
        defer { if key == mine { loading = false } }
        // What was kept is shown at once. A listing kept in the last ten minutes is also not asked for again (unless pulled to refresh).
        let (kept, stale) = await Task.detached { () -> ([Shelf], Set<String>) in
            var found: [Shelf] = []
            var stale = Set<String>()
            for source in sources {
                for isLatest in source.canLatest ? [false, true] : [false] {
                    let id = "\(source.id)|\(isLatest ? "latest" : "popular")"
                    let fresh = force ? nil : engine.freshListing(source: source.id, latest: isLatest)
                    if fresh == nil { stale.insert(id) }
                    if let page = fresh ?? engine.rememberedListing(source: source.id, latest: isLatest) {
                        found.append(Shelf(id: id, title: source.name, subtitle: isLatest ? Copy.Home.newReleases : Copy.Home.popular, source: source.id, items: Array(page.items.prefix(14))))
                    }
                }
            }
            return (found, stale)
        }.value
        shelves = kept
        await withTaskGroup(of: (SourceInfo, Bool, Result<[Card], Error>).self) { group in
            // At most four sites are asked at a time, so the first sources answer first and covers can load beside them.
            var asking = 0
            var waiting: [(SourceInfo, Bool)] = []
            func start(_ source: SourceInfo, _ isLatest: Bool) {
                asking += 1
                group.addTask { (source, isLatest, await Result { isLatest ? try await engine.latest(source: source.id, page: 1).items : try await engine.popular(source: source.id, page: 1).items }) }
            }
            for source in sources {
                // A source without a popular or latest list is not asked for it: asking would only surface the
                // engine's "has no popular list" as a failure the person cannot fix.
                if source.canPopular && stale.contains("\(source.id)|popular") { waiting.append((source, false)) }
                if source.canLatest && stale.contains("\(source.id)|latest") { waiting.append((source, true)) }
            }
            waiting.reverse()
            while asking < 4, let next = waiting.popLast() { start(next.0, next.1) }
            for await (source, isLatest, result) in group {
                asking -= 1
                if let next = waiting.popLast() { start(next.0, next.1) }
                guard key == mine else { continue }
                let id = "\(source.id)|\(isLatest ? "latest" : "popular")"
                switch result {
                case .success(let cards):
                    report(source.id, nil)
                    let shelf = Shelf(id: id, title: source.name, subtitle: isLatest ? Copy.Home.newReleases : Copy.Home.popular, source: source.id, items: Array(cards.prefix(14)))
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
