import Foundation
import Network

/// Other Akidev phones on the Wi-Fi, found and announced with Bonjour. iPhone blocks multicast without an entitlement Apple hands out
/// on request, and Bonjour needs none. Each phone is announced as `_akidev._tcp` on the port the engine's server listens on.
@MainActor
final class Nearby {
    private static let type = "_akidev._tcp"
    private var service: NetService?
    private var browser: NWBrowser?
    private var asking: [String: (service: NetService, answer: Resolved)] = [:]
    /// What each Bonjour name turned out to be, so a phone that goes away can be taken off the list.
    private var known: [String: (ip: String, port: UInt16)] = [:]
    /// A name nobody else has, so this phone does not list itself and two phones called "iPhone" are not mixed up.
    private let ownName = "akidev-\(UUID().uuidString.prefix(8))"

    var found: (_ ip: String, _ port: UInt16) -> Void = { _, _ in }
    var lost: (_ ip: String, _ port: UInt16) -> Void = { _, _ in }
    /// The person said no to the Local Network question (or the phone cannot ask it).
    var denied: (Bool) -> Void = { _ in }

    func start(port: UInt16) {
        let service = NetService(domain: "", type: Self.type + ".", name: ownName, port: Int32(port))
        service.publish()
        self.service = service

        let browser = NWBrowser(for: .bonjour(type: Self.type, domain: nil), using: .tcp)
        browser.stateUpdateHandler = { [weak self] state in
            Task { @MainActor in
                switch state {
                case .ready: self?.denied(false)
                case .waiting(let error), .failed(let error):
                    // -65570: the system refused, because Local Network is off for Akidev.
                    if case .dns(let code) = error, code == -65570 || code == -65555 { self?.denied(true) }
                default: break
                }
            }
        }
        browser.browseResultsChangedHandler = { [weak self] _, changes in
            Task { @MainActor in
                for change in changes {
                    switch change {
                    case .added(let result): self?.resolve(result)
                    case .removed(let result): self?.forget(result)
                    default: break
                    }
                }
            }
        }
        browser.start(queue: .main)
        self.browser = browser
    }

    func stop() {
        service?.stop()
        service = nil
        browser?.cancel()
        browser = nil
        asking.values.forEach { $0.service.stop() }
        asking = [:]
        known = [:]
    }

    private func name(of result: NWBrowser.Result) -> String? {
        if case .service(let name, _, _, _) = result.endpoint, name != ownName { return name }
        return nil
    }

    /// Bonjour gives a name, not an address: resolving it asks the other phone for its addresses, and the IPv4 one is kept.
    private func resolve(_ result: NWBrowser.Result) {
        guard let name = name(of: result), case .service(_, let type, let domain, _) = result.endpoint else { return }
        let service = NetService(domain: domain, type: type + ".", name: name)
        let answer = Resolved { [weak self] ip, port in
            self?.known[name] = (ip, port)
            self?.found(ip, port)
            self?.asking[name] = nil
        }
        service.delegate = answer
        asking[name] = (service, answer)
        service.resolve(withTimeout: 5)
    }

    private func forget(_ result: NWBrowser.Result) {
        guard let name = name(of: result) else { return }
        asking[name]?.service.stop()
        asking[name] = nil
        if let gone = known.removeValue(forKey: name) { lost(gone.ip, gone.port) }
    }
}

/// Hears a service's addresses and hands over its first IPv4 one.
private final class Resolved: NSObject, NetServiceDelegate, @unchecked Sendable {
    let done: @MainActor @Sendable (String, UInt16) -> Void

    init(done: @escaping @MainActor @Sendable (String, UInt16) -> Void) { self.done = done }

    func netServiceDidResolveAddress(_ sender: NetService) {
        for address in sender.addresses ?? [] {
            let ip = address.withUnsafeBytes { raw -> String? in
                let socket = raw.baseAddress!.assumingMemoryBound(to: sockaddr.self)
                guard socket.pointee.sa_family == UInt8(AF_INET) else { return nil }
                var host = [CChar](repeating: 0, count: Int(NI_MAXHOST))
                getnameinfo(socket, socklen_t(address.count), &host, socklen_t(host.count), nil, 0, NI_NUMERICHOST)
                return String(cString: host)
            }
            if let ip {
                let port = UInt16(sender.port)
                Task { @MainActor [done] in done(ip, port) }
                return
            }
        }
    }
}
