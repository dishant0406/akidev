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
    /// Where this phone is, one thing at a time: choosing who to send to (or showing its own code), scanning a friend's code, following
    /// that code (joining their temporary Wi-Fi, then finding their phone), waiting for them to send (this phone is the one receiving),
    /// or sending.
    enum Step {
        case choosing
        case scanning
        case joining
        case connecting
        case connected(String)
        case sending(SharePeer, Outcome)

        /// A scanned code is being followed.
        var isFollowingCode: Bool {
            switch self {
            case .joining, .connecting: true
            default: false
            }
        }
    }
    /// What another phone is sending to this one. The engine takes one send at a time.
    enum Incoming {
        case none
        /// The person is being asked.
        case asked(Asked)
        /// The person said yes: which send it is, who sends, and how far it has got.
        case receiving(session: String, from: String, fraction: Double)
    }
    struct NewSource { let path: String; let card: ShareCard }

    var item: ShareItem?
    var peers: [SharePeer] = []
    var denied = false
    var failure: String?
    /// The text the receive screen turns into a QR code; nil when this phone is not on a Wi-Fi.
    var code: String?
    /// Eight seconds went by and nobody was listed: the sheet says what to check, and the Wi-Fi was looked through once.
    var looked = false
    var step = Step.choosing
    var codeNote: String?
    var incoming = Incoming.none
    /// The person is being asked whether to accept a send.
    var asked: Asked? {
        if case .asked(let asked) = incoming { asked } else { nil }
    }
    /// What arrived and was added, one line each. Once there is one, the sheet says so.
    var added: [String] = []
    var newSource: NewSource?
    /// What went wrong, for the person to read. When it is closed and a source could not be added for want of the internet,
    /// the question about that source comes back, so the person can connect and tap Add again.
    var problem: String? {
        didSet { if problem == nil, let again = retry { retry = nil; newSource = again } }
    }
    private var retry: NewSource?

    private var app: AppModel?
    private var share: Share?
    private var job: ShareSend?
    private let nearby = Nearby()
    private var lookAgain: Task<Void, Never>?
    private var waiting: [NewSource] = []
    private var host: PeerHost?
    private var gate: PeerGate?
    /// The temporary Wi-Fi this phone joined, to leave when the send is over.
    private var joined: String?
    private var networkTimer: Task<Void, Never>?

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
            // Only a phone that is waiting to receive offers the direct link: Apple slows the Wi-Fi while it is on.
            if item == nil {
                let host = PeerHost()
                if (try? host.start(enginePort: port)) != nil { self.host = host }
            }
            code = share.makeCode(ip: nil, ssid: nil, password: nil, peer: host?.name)
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
        endNetwork()
        host?.stop()
        host = nil
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

    /// A friend's code was scanned: join their temporary Wi-Fi if it has one, find their phone, and send to it at once (or, when this
    /// phone is the one receiving, wait for them to send).
    func connect(code: String) {
        guard let share, !step.isFollowingCode else { return }
        step = .connecting
        Task {
            // Where the screen goes if this fails: the scanner stays open, unless a temporary Wi-Fi was joined.
            var back = Step.scanning
            guard let found = try? share.readCode(code: code) else {
                codeNote = "That is not an Akidev code"
                step = back
                return
            }
            if let ssid = found.ssid, let password = found.password {
                back = .choosing
                step = .joining
                do { try await WifiJoin.join(ssid: ssid, password: password) } catch {
                    problem = "Could not join their Wi-Fi."
                    step = back
                    return
                }
                joined = ssid
                keepNetworkFor5Minutes()
                step = .connecting
            }
            guard let peer = await reach(found, code: code, share: share) else {
                if joined != nil {
                    endNetwork()
                    problem = "Could not reach that phone."
                } else {
                    codeNote = found.peer == nil ? "Could not reach that phone" : "Could not reach that phone. Switch Wi-Fi on for both phones; it does not need to be connected."
                }
                step = back
                return
            }
            codeNote = nil
            if item != nil {
                step = .choosing
                send(to: peer)
            } else {
                step = .connected(peer.alias)
            }
        }
    }

    /// By its address (ten tries on a temporary Wi-Fi, about 15 seconds: a new Wi-Fi can take that long to give this phone an address),
    /// or else by the direct link.
    private func reach(_ found: ShareCode, code: String, share: Share) async -> SharePeer? {
        if found.ip != nil {
            for attempt in 0..<(joined == nil ? 1 : 10) {
                if attempt > 0 { try? await Task.sleep(for: .milliseconds(1500)) }
                if let peer = try? await share.probeCode(code: code) { return peer }
            }
        }
        guard let name = found.peer else { return nil }
        let gate = PeerGate()
        guard let port = try? await gate.open(name: name), let peer = try? await share.probe(ip: "127.0.0.1", port: port, protocol: "https") else {
            gate.stop()
            return nil
        }
        self.gate?.stop()
        self.gate = gate
        keepNetworkFor5Minutes()
        return peer
    }

    /// The temporary Wi-Fi or the direct link ends with the send, and at the latest after five minutes.
    private func keepNetworkFor5Minutes() {
        networkTimer?.cancel()
        networkTimer = Task { [weak self] in
            try? await Task.sleep(for: .seconds(300))
            guard !Task.isCancelled else { return }
            self?.endNetwork()
            self?.stopWaiting()
        }
    }

    /// Ends the waiting for the other phone to send, if this phone was waiting.
    private func stopWaiting() {
        if case .connected = step { step = .choosing }
    }

    /// Leaves the temporary Wi-Fi in a few seconds: the "got it" reply to the sender is still on its way, and leaving now would cut it.
    /// Joining another Wi-Fi in that time replaces this timer.
    private func endNetworkSoon() {
        networkTimer?.cancel()
        networkTimer = Task { [weak self] in
            try? await Task.sleep(for: .seconds(3))
            guard !Task.isCancelled else { return }
            self?.endNetwork()
        }
    }

    func endNetwork() {
        networkTimer?.cancel()
        networkTimer = nil
        if let joined { WifiJoin.leave(ssid: joined) }
        joined = nil
        gate?.stop()
        gate = nil
    }

    /// The person gives up waiting on the other phone.
    func disconnect() {
        endNetwork()
        stopWaiting()
    }

    // MARK: sending

    func send(to peer: SharePeer) {
        guard let share, let item, let engine = app?.engine else { return }
        step = .sending(peer, .waiting)
        Task {
            do {
                let file = try await Task.detached { try item.file(engine) }.value
                let job = share.sender(to: peer, files: [file])
                self.job = job
                let poll = Task {
                    while !Task.isCancelled {
                        let progress = job.progress()
                        if progress.total > 0, !Task.isCancelled { step = .sending(peer, .sending(Double(progress.done) / Double(progress.total))) }
                        try? await Task.sleep(for: .milliseconds(200))
                    }
                }
                let outcome = try await job.run()
                poll.cancel()
                switch outcome {
                case .sent:
                    step = .sending(peer, .done)
                    endNetwork()
                case .declined: step = .sending(peer, .declined)
                case .busy: step = .sending(peer, .busy)
                case .stopped: step = .choosing
                }
            } catch { step = .sending(peer, .failed(describe(error).message)) }
            job = nil
            engine.shareClearOutgoing()
        }
    }

    func stopSending() { job?.stop() }

    func sendAgain() {
        guard case .sending(let peer, _) = step else { return }
        send(to: peer)
    }

    func backToList() { step = .choosing }

    // MARK: receiving

    func asked(_ session: String, _ from: String, _ files: [ShareOffered]) { incoming = .asked(Asked(session: session, from: from, files: files)) }

    func answer(_ accept: Bool) {
        guard let asked else { return }
        share?.answer(session: asked.session, accept: accept)
        incoming = accept ? .receiving(session: asked.session, from: asked.from, fraction: 0) : .none
    }

    func stopReceiving() { if case .receiving(let session, _, _) = incoming { share?.stopReceiving(session: session) } }

    func progress(_ fraction: Double) {
        if case .receiving(let session, let from, _) = incoming { incoming = .receiving(session: session, from: from, fraction: fraction) }
    }

    func ended(_ session: String, _ reason: String) {
        let wasReceiving: Bool
        switch incoming {
        case .none: wasReceiving = false
        case .asked(let asked): wasReceiving = asked.session == session
        case .receiving: wasReceiving = true
        }
        incoming = .none
        if wasReceiving, reason != "declined", reason != "stopped on this phone" { problem = "The send stopped: \(reason)." }
    }

    /// Puts each arrived file where it belongs. A series from a source that is new here waits for a yes first.
    func finished(_ files: [ShareReceived]) {
        incoming = .none
        stopWaiting()
        endNetworkSoon()
        host?.stop()
        host = nil
        guard let app, let share else { return }
        let engine = app.engine
        Task {
            for file in files {
                do { if let line = try await take(file, engine) { added.append(line) } } catch { problem = describe(error).message }
            }
            app.refresh()
            app.booksRevision += 1
            askNextSource()
            // A card that waits for "Add the source?" is still a file: keep it until the person answers.
            if newSource == nil { share.clean() }
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
                } catch {
                    let reason = describe(error).message
                    if reason.contains("could not reach") {
                        // Nothing was added, and the card is kept: a source is downloaded, so this phone needs the internet.
                        problem = "Could not add the source: this phone needs the internet to download it. Connect to Wi-Fi or mobile data, then tap Add again."
                        retry = pending
                    } else {
                        problem = reason
                    }
                }
            }
            askNextSource()
            if newSource == nil, retry == nil { share?.clean() }
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
