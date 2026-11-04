import Foundation
import Network

/// A direct link between two iPhones with no Wi-Fi in common, made with Apple's own peer-to-peer Wi-Fi. The engine speaks HTTPS over
/// plain sockets and cannot reach that link, so these two classes carry the bytes: the receiver hands every connection that comes in
/// to the engine's server on this phone, and the sender opens a local door the engine connects to and carries each connection over
/// the link. The HTTPS stays on top, so the link is only a pipe. Apple slows the Wi-Fi while peer-to-peer is on: both stop as soon
/// as they can.
private let linkType = "_akidev-p2p._tcp"
private let queue = DispatchQueue(label: "live.akidev.peer-link")

private func peerParameters() -> NWParameters {
    let parameters = NWParameters.tcp
    parameters.includePeerToPeer = true
    return parameters
}

/// Two connections joined: what comes in on one goes out on the other, and the pipe closes when both sides have ended.
private final class Pipe: @unchecked Sendable {
    private let one: NWConnection
    private let other: NWConnection
    private var open = 2

    init(_ one: NWConnection, _ other: NWConnection) {
        self.one = one
        self.other = other
        one.start(queue: queue)
        other.start(queue: queue)
        carry(from: one, to: other)
        carry(from: other, to: one)
    }

    /// Reads a piece, sends it, and only then reads the next, so nothing piles up in memory.
    private func carry(from source: NWConnection, to target: NWConnection) {
        source.receive(minimumIncompleteLength: 1, maximumLength: 64 * 1024) { [self] data, _, ended, error in
            if error != nil { return close() }
            let next = {
                if ended { self.finish(target) } else { self.carry(from: source, to: target) }
            }
            if let data, !data.isEmpty {
                target.send(content: data, completion: .contentProcessed { failure in
                    if failure != nil { self.close() } else { next() }
                })
            } else {
                next()
            }
        }
    }

    /// One side has nothing more to say: tell the other, and close when both are done.
    private func finish(_ target: NWConnection) {
        target.send(content: nil, contentContext: .finalMessage, isComplete: true, completion: .contentProcessed { _ in })
        open -= 1
        if open == 0 { close() }
    }

    func close() {
        one.cancel()
        other.cancel()
    }
}

/// The receiving phone: announces a name and hands every connection to the engine's server on this phone.
final class PeerHost: @unchecked Sendable {
    /// What the other phone looks for: it is the `b` in the QR code.
    let name = "akidev-\(UUID().uuidString.prefix(8).lowercased())"
    private var listener: NWListener?

    func start(enginePort: UInt16) throws {
        guard let port = NWEndpoint.Port(rawValue: enginePort) else { return }
        let listener = try NWListener(using: peerParameters())
        listener.service = NWListener.Service(name: name, type: linkType)
        listener.newConnectionHandler = { incoming in
            _ = Pipe(incoming, NWConnection(host: "127.0.0.1", port: port, using: .tcp))
        }
        listener.start(queue: queue)
        self.listener = listener
    }

    func stop() {
        listener?.cancel()
        listener = nil
    }
}

struct PeerLinkError: Error {}

/// The sending phone: finds the receiver by its name, and opens a door on this phone that leads to it.
final class PeerGate: @unchecked Sendable {
    private var browser: NWBrowser?
    private var door: NWListener?

    /// Finds the phone called `name` and returns the port of the local door. Throws when it is not found in time.
    func open(name: String, within seconds: Double = 10) async throws -> UInt16 {
        let endpoint = try await find(name, within: seconds)
        return try await listen(to: endpoint)
    }

    private func find(_ name: String, within seconds: Double) async throws -> NWEndpoint {
        try await withCheckedThrowingContinuation { continuation in
            let once = Once()
            let browser = NWBrowser(for: .bonjour(type: linkType, domain: nil), using: peerParameters())
            browser.browseResultsChangedHandler = { [weak self] results, _ in
                for result in results {
                    if case .service(let found, _, _, _) = result.endpoint, found == name, once.first() {
                        // Apple says to stop browsing before connecting, or the radio stays busy.
                        self?.browser?.cancel()
                        self?.browser = nil
                        continuation.resume(returning: result.endpoint)
                    }
                }
            }
            browser.start(queue: queue)
            self.browser = browser
            queue.asyncAfter(deadline: .now() + seconds) { [weak self] in
                guard once.first() else { return }
                self?.browser?.cancel()
                self?.browser = nil
                continuation.resume(throwing: PeerLinkError())
            }
        }
    }

    /// A door on this phone only: the engine connects to it, and each connection is carried to the other phone.
    private func listen(to endpoint: NWEndpoint) async throws -> UInt16 {
        let parameters = NWParameters.tcp
        parameters.requiredLocalEndpoint = .hostPort(host: "127.0.0.1", port: .any)
        let door = try NWListener(using: parameters)
        door.newConnectionHandler = { local in
            _ = Pipe(local, NWConnection(to: endpoint, using: peerParameters()))
        }
        self.door = door
        return try await withCheckedThrowingContinuation { continuation in
            let once = Once()
            door.stateUpdateHandler = { state in
                switch state {
                case .ready:
                    if once.first(), let port = door.port?.rawValue { continuation.resume(returning: port) }
                case .failed, .cancelled:
                    if once.first() { continuation.resume(throwing: PeerLinkError()) }
                default: break
                }
            }
            door.start(queue: queue)
        }
    }

    func stop() {
        browser?.cancel()
        browser = nil
        door?.cancel()
        door = nil
    }
}

/// `first()` is true once: for the one answer a continuation can take.
private final class Once: @unchecked Sendable {
    private let lock = NSLock()
    private var spent = false

    func first() -> Bool {
        lock.lock()
        defer { lock.unlock() }
        if spent { return false }
        spent = true
        return true
    }
}
