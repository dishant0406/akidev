import AkiManga
import Foundation
import Network
import os

/// New chapters waiting to be kept on the device. The list is saved, so a download that iOS cut short, or that waited for
/// Wi-Fi, carries on at the next chance (the app opening, or the system giving the app time in the background).
@MainActor
final class AutoDownloader {
    private static let key = "autoDownloadQueue"
    /// Only the newest few of a series are fetched in one go, so a long-unchecked series does not pull hundreds.
    static let perSeries = 5
    private let engine: Engine
    private let wifiOnly: () -> Bool
    private var running = false

    init(engine: Engine, wifiOnly: @escaping () -> Bool) {
        self.engine = engine
        self.wifiOnly = wifiOnly
    }

    private struct Item: Codable, Equatable {
        let source: String
        let series: String
        let chapter: String
    }

    private var queue: [Item] {
        get { (UserDefaults.standard.data(forKey: Self.key).flatMap { try? JSONDecoder().decode([Item].self, from: $0) }) ?? [] }
        set { UserDefaults.standard.set(try? JSONEncoder().encode(newValue), forKey: Self.key) }
    }

    var hasWork: Bool { !queue.isEmpty }

    func enqueue(_ found: [NewChapters]) {
        var items = queue
        for series in found {
            for chapter in series.chapters.suffix(Self.perSeries) {
                let item = Item(source: series.source, series: series.series, chapter: chapter.id)
                if !items.contains(item) { items.append(item) }
            }
        }
        queue = items
    }

    /// Downloads what is queued, one chapter at a time, until it is done or the task is cancelled. A chapter that fails
    /// stays queued for next time; one already on the device is dropped from the list.
    func run() async {
        guard !running, hasWork else { return }
        if wifiOnly(), await Network.isMetered() { return }
        running = true
        defer { running = false }
        let kept = Set(((try? engine.downloads()) ?? []).filter { $0.state == "done" }.map { "\($0.source)/\($0.series)/\($0.chapter)" })
        for item in queue {
            if Task.isCancelled { return }
            if !kept.contains("\(item.source)/\(item.series)/\(item.chapter)") {
                do { try await engine.downloadChapter(source: item.source, series: item.series, chapter: item.chapter) } catch { continue }
            }
            queue.removeAll { $0 == item }
        }
    }
}

enum Network {
    /// Whether the connection right now is cellular or a hotspot, or the person turned on Low Data Mode.
    static func isMetered() async -> Bool {
        await withCheckedContinuation { continuation in
            let monitor = NWPathMonitor()
            let answered = OSAllocatedUnfairLock(initialState: false)
            monitor.pathUpdateHandler = { path in
                let first = answered.withLock { done -> Bool in
                    defer { done = true }
                    return !done
                }
                guard first else { return }
                monitor.cancel()
                continuation.resume(returning: path.isExpensive || path.isConstrained)
            }
            monitor.start(queue: DispatchQueue(label: "live.akidev.manga.network"))
        }
    }
}
