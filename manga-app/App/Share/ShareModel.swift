import AkiManga
import Observation
import UIKit

/// What a screen can offer to send.
enum ShareItem {
    case book(BookInfo)
    case series(SeriesRef)

    var title: String {
        switch self {
        case .book(let book): book.title
        case .series(let series): series.title
        }
    }

    /// The file that goes: a book is sent as it is, a series from a source as a small card, a comic that came from a file as that file.
    func file(_ engine: Engine) throws -> ShareFile {
        switch self {
        case .book(let book): return try engine.shareBook(id: book.id)
        case .series(let series):
            if series.source == "device" { return try engine.shareComic(series: series.id, title: series.title) }
            return try engine.shareCard(source: series.source, series: series.id, title: series.title, cover: series.cover)
        }
    }
}

/// The engine's news comes from its own threads; each piece is handed to the main actor.
private final class Hears: ShareEvents, @unchecked Sendable {
    weak var model: ShareModel?

    func asked(session: String, from: String, files: [ShareOffered]) { Task { @MainActor in self.model?.asked(session, from, files) } }
    func progress(session: String, done: UInt64, total: UInt64) { Task { @MainActor in self.model?.progress(Double(done) / Double(max(total, 1))) } }
    func finished(session: String, files: [ShareReceived]) { Task { @MainActor in self.model?.finished(files) } }
    func ended(session: String, reason: String) { Task { @MainActor in self.model?.ended(session, reason) } }
    func seen(peer: SharePeer) { Task { @MainActor in self.model?.add(peer) } }
}

/// One opening of the share screen: this phone listens, announces itself and lists the others while the screen is open.
@MainActor @Observable
final class ShareModel {
    enum Outcome { case waiting, sending(Double), done, declined, busy, failed(String) }
    struct Asked { let session: String; let from: String; let files: [ShareOffered] }
    struct NewSource { let path: String; let card: ShareCard }

    var item: ShareItem?
    var peers: [SharePeer] = []
    var denied = false
    var failure: String?
    /// The text the receive screen turns into a QR code; nil when this phone is not on a Wi-Fi.
    var code: String?
    /// Eight seconds went by and nobody was listed: the sheet says what to check, and the Wi-Fi was looked through once.
    var looked = false
    /// The camera is open, looking for a friend's code.
    var scanningCode = false
    var codeNote: String?
    var sending: (peer: SharePeer, outcome: Outcome)?
    var asked: Asked?
    /// Who is sending to this phone, and how far it has got, once the person said yes.
    var incoming: (from: String, fraction: Double)?
    /// What arrived and was added, one line each.
    var added: [String] = []
    /// Set when the last send that came in is over, so the sheet can say so.
    var received = false
    var newSource: NewSource?
    var problem: String?

    private var app: AppModel?
    private var share: Share?
    private var job: ShareSend?
    private let nearby = Nearby()
    private var lookAgain: Task<Void, Never>?
    private var waiting: [NewSource] = []
    /// The send being received, once the person said yes.
    private var receiving: String?

    func begin(app: AppModel, item: ShareItem?) async {
        self.app = app
        self.item = item
        UIApplication.shared.isIdleTimerDisabled = true
        let hears = Hears()
        hears.model = self
        do {
            let share = try app.engine.share(alias: UIDevice.current.name, model: UIDevice.current.model, kind: "mobile", wifiAddress: Self.wifiAddress(), events: hears)
            self.share = share
            let port = try await share.start()
            nearby.found = { [weak self] ip, port in Task { await self?.probe(ip: ip, port: port) } }
            nearby.lost = { [weak self] ip, port in self?.peers.removeAll { $0.ip == ip && $0.port == port } }
            nearby.denied = { [weak self] in self?.denied = $0 }
            nearby.start(port: port)
            code = share.code()
            if item != nil {
                lookAgain = Task { [weak self] in
                    try? await Task.sleep(for: .seconds(8))
                    guard !Task.isCancelled, let self, peers.isEmpty else { return }
                    looked = true
                    await self.share?.scan()
                }
            }
        } catch { failure = describe(error).message }
    }

    func end() {
        lookAgain?.cancel()
        nearby.stop()
        share?.stop()
        share = nil
        job?.stop()
        UIApplication.shared.isIdleTimerDisabled = false
    }

    // MARK: finding

    func add(_ peer: SharePeer) {
        if let index = peers.firstIndex(where: { $0.ip == peer.ip && $0.port == peer.port }) { peers[index] = peer } else { peers.append(peer) }
    }

    private func probe(ip: String, port: UInt16) async {
        guard let share, let peer = try? await share.probe(ip: ip, port: port, protocol: "https") else { return }
        add(peer)
    }

    /// A friend's code was scanned: find their phone and send to it at once.
    func connect(code: String) {
        guard let share else { return }
        Task {
            do {
                let peer = try await share.probeCode(code: code)
                scanningCode = false
                codeNote = nil
                send(to: peer)
            } catch {
                codeNote = describe(error).message.contains("not a code") ? "That is not an Akidev code" : "Could not reach that phone"
            }
        }
    }

    // MARK: sending

    func send(to peer: SharePeer) {
        guard let share, let item, let engine = app?.engine else { return }
        sending = (peer, .waiting)
        Task {
            do {
                let file = try await Task.detached { try item.file(engine) }.value
                let job = share.sender(to: peer, files: [file])
                self.job = job
                let poll = Task {
                    while !Task.isCancelled {
                        let progress = job.progress()
                        if progress.total > 0, !Task.isCancelled { sending = (peer, .sending(Double(progress.done) / Double(progress.total))) }
                        try? await Task.sleep(for: .milliseconds(200))
                    }
                }
                let outcome = try await job.run()
                poll.cancel()
                switch outcome {
                case .sent: sending = (peer, .done)
                case .declined: sending = (peer, .declined)
                case .busy: sending = (peer, .busy)
                case .stopped: sending = nil
                }
            } catch { sending = (peer, .failed(describe(error).message)) }
            job = nil
            engine.shareClearOutgoing()
        }
    }

    func stopSending() { job?.stop() }

    func sendAgain() {
        guard let peer = sending?.peer else { return }
        send(to: peer)
    }

    func backToList() { sending = nil }

    // MARK: receiving

    func asked(_ session: String, _ from: String, _ files: [ShareOffered]) { asked = Asked(session: session, from: from, files: files) }

    func answer(_ accept: Bool) {
        guard let asked else { return }
        share?.answer(session: asked.session, accept: accept)
        self.asked = nil
        if accept {
            incoming = (asked.from, 0)
            receiving = asked.session
        }
    }

    func stopReceiving() { if let receiving { share?.stopReceiving(session: receiving) } }

    func progress(_ fraction: Double) { if let from = incoming?.from { incoming = (from, fraction) } }

    func ended(_ session: String, _ reason: String) {
        let wasAsked = asked?.session == session
        let wasReceiving = incoming != nil || wasAsked
        asked = nil
        incoming = nil
        receiving = nil
        if wasReceiving, reason != "declined", reason != "stopped on this phone" { problem = "The send stopped: \(reason)." }
    }

    /// Puts each arrived file where it belongs. A series from a source that is new here waits for a yes first.
    func finished(_ files: [ShareReceived]) {
        incoming = nil
        receiving = nil
        guard let app, let share else { return }
        let engine = app.engine
        Task {
            for file in files {
                do { if let line = try await take(file, engine) { added.append(line) } } catch { problem = describe(error).message }
            }
            app.refresh()
            app.booksRevision += 1
            received = !added.isEmpty
            askNextSource()
            if waiting.isEmpty { share.clean() }
        }
    }

    private func take(_ file: ShareReceived, _ engine: Engine) async throws -> String? {
        switch file.kind {
        case "book":
            let title = try await Task.detached { try engine.importBook(path: file.path).title }.value
            return "\"\(title)\" is on your shelf."
        case "comic":
            _ = try await Task.detached { try engine.importFile(path: file.path) }.value
            return "\"\(file.name)\" is in your library."
        case "card":
            let card = try engine.shareReadCard(path: file.path)
            guard card.installed else {
                waiting.append(NewSource(path: file.path, card: card))
                return nil
            }
            try await engine.shareAddCard(path: file.path)
            return "\"\(card.title)\" is in your library."
        default:
            return nil
        }
    }

    private func askNextSource() {
        if newSource == nil, !waiting.isEmpty { newSource = waiting.removeFirst() }
    }

    /// The person's answer to "Add the source?".
    func answerSource(_ accept: Bool) {
        guard let pending = newSource, let engine = app?.engine else { return }
        newSource = nil
        Task {
            if accept {
                do {
                    try await engine.shareAddCard(path: pending.path)
                    added.append("\"\(pending.card.title)\" is in your library.")
                    app?.refresh()
                } catch { problem = describe(error).message }
            }
            askNextSource()
            if newSource == nil { share?.clean() }
        }
    }

    /// This phone's address on the Wi-Fi (the interface called en0): the other phone reaches it there, and the QR code carries it.
    private static func wifiAddress() -> String? {
        var list: UnsafeMutablePointer<ifaddrs>?
        guard getifaddrs(&list) == 0, let first = list else { return nil }
        defer { freeifaddrs(list) }
        for entry in sequence(first: first, next: { $0.pointee.ifa_next }) {
            let found = entry.pointee
            guard found.ifa_addr.pointee.sa_family == UInt8(AF_INET), String(cString: found.ifa_name) == "en0" else { continue }
            var host = [CChar](repeating: 0, count: Int(NI_MAXHOST))
            getnameinfo(found.ifa_addr, socklen_t(found.ifa_addr.pointee.sa_len), &host, socklen_t(host.count), nil, 0, NI_NUMERICHOST)
            return String(cString: host)
        }
        return nil
    }
}
