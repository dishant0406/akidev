import AkiManga
import SwiftUI

/// What opens the share sheet: to send something, or only to receive.
struct SharePresented: Identifiable {
    let id = UUID()
    let item: ShareItem?
}

/// Send a book or a series to a friend's phone on the same Wi-Fi, and receive from them. No server: the phones talk to each other,
/// so while this sheet is open the phone listens and shows up on the other phone's list. One thing is on screen at a time.
struct ShareView: View {
    let item: ShareItem?
    @Environment(AppModel.self) private var app
    @Environment(\.dismiss) private var dismiss
    @State private var model = ShareModel()

    var body: some View {
        VStack(alignment: .leading, spacing: 20) {
            AkiHeader(title: item == nil ? "Receive" : "Send", subtitle: item?.title)
            content
        }
        .padding(24)
        .frame(maxHeight: .infinity, alignment: .top)
        .task { await model.begin(app: app, item: item) }
        .onDisappear { model.end() }
        .akiQuestion(askTitle, message: "Only accept from someone you know.", yes: "Accept", no: "Decline", isPresented: Binding(get: { model.asked != nil }, set: { _ in }), onYes: { model.answer(true) }, onNo: { model.answer(false) })
        .akiQuestion("Add the source \(model.newSource?.card.sourceName ?? "")?", message: sourceMessage, yes: "Add", no: "Not now", isPresented: Binding(get: { model.newSource != nil }, set: { _ in }), onYes: { model.answerSource(true) }, onNo: { model.answerSource(false) })
        .akiAlert("Share", message: model.problem, isPresented: Binding(get: { model.problem != nil }, set: { if !$0 { model.problem = nil } }))
    }

    @ViewBuilder private var content: some View {
        if let failure = model.failure {
            centered { AkiText(failure, .body, tone: .muted, align: .center) }
        } else if model.denied {
            centered {
                AkiText("Turn on Local Network for Akidev to see other phones.", .body, tone: .muted, align: .center)
                AkiButton("Open Settings", variant: .primary) { openSettings() }
            }
        } else if model.joining {
            centered {
                AkiSpinner()
                AkiText("Joining their Wi-Fi…", .heading, align: .center)
            }
        } else if let sending = model.sending {
            sendingState(sending.peer, sending.outcome)
        } else if let incoming = model.incoming {
            centered {
                AkiText("Receiving from \(incoming.from)", .heading, align: .center)
                AkiProgressBar(fraction: incoming.fraction)
                AkiButton("Cancel", variant: .ghost, size: .small) { model.stopReceiving() }
            }
        } else if model.received, let line = model.added.last {
            done(line)
        } else if let name = model.connected {
            centered {
                AkiSpinner()
                AkiText("Connected. Waiting for \(name) to send…", .heading, lines: 3, align: .center)
                AkiButton("Cancel", variant: .ghost, size: .small) { model.disconnect() }
            }
        } else if model.scanningCode {
            scanner
        } else if item != nil {
            people
        } else {
            code
        }
    }

    // MARK: send

    private var people: some View {
        VStack(spacing: 16) {
            if model.peers.isEmpty {
                centered {
                    AkiSpinner()
                    AkiText("Looking nearby…", .body, tone: .muted)
                    if model.looked { AkiText("Can't see them? Both phones need the same Wi-Fi.", .caption, tone: .muted, align: .center) }
                }
                .frame(maxHeight: .infinity)
            } else {
                ScrollView {
                    VStack(spacing: 10) {
                        ForEach(model.peers, id: \.id) { peer in
                            AkiRow(title: peer.alias, icon: "share", showsChevron: true)
                                .contentShape(Rectangle())
                                .onTapGesture { model.send(to: peer) }
                                .accessibilityElement(children: .combine)
                                .accessibilityAddTraits(.isButton)
                        }
                    }
                }
            }
            AkiButton("Scan their code", icon: "qr", fullWidth: true) { model.scanningCode = true }
        }
        .frame(maxHeight: .infinity)
    }

    private var scanner: some View {
        VStack(spacing: 14) {
            AkiScanner { model.connect(code: $0) }
            if let note = model.codeNote { AkiText(note, .caption, tone: .destructive, align: .center) }
            AkiButton("Cancel", variant: .ghost, size: .small) { model.scanningCode = false; model.codeNote = nil }
        }
        .frame(maxWidth: .infinity)
    }

    @ViewBuilder private func sendingState(_ peer: SharePeer, _ outcome: ShareModel.Outcome) -> some View {
        switch outcome {
        case .waiting:
            centered {
                AkiSpinner()
                AkiText("Waiting for \(peer.alias)…", .heading, align: .center)
                AkiButton("Cancel", variant: .ghost, size: .small) { model.stopSending() }
            }
        case .sending(let fraction):
            centered {
                AkiText("Sending to \(peer.alias)", .heading, align: .center)
                AkiProgressBar(fraction: fraction)
                AkiText("\(Int(fraction * 100))%", .caption, tone: .muted)
                AkiButton("Cancel", variant: .ghost, size: .small) { model.stopSending() }
                AkiText("Keep Akidev open", .caption, tone: .muted)
            }
        case .done:
            done("Sent to \(peer.alias)")
        case .declined: problem("\(peer.alias) said no.")
        case .busy: problem("\(peer.alias) is busy right now.")
        case .failed(let message): problem(message)
        }
    }

    private func problem(_ line: String) -> some View {
        centered {
            AkiText(line, .heading, align: .center)
            AkiButton("Try again", variant: .primary) { model.sendAgain() }
            AkiButton("Close", variant: .ghost, size: .small) { dismiss() }
        }
    }

    private func done(_ line: String) -> some View {
        centered {
            AkiIcon("check", size: 44)
            AkiText(line, .heading, lines: 3, align: .center)
            AkiButton("Done", variant: .primary) { dismiss() }
        }
    }

    // MARK: receive

    private var code: some View {
        centered {
            if let text = model.code {
                AkiQRCode(text: text)
                AkiText("Ask them to scan this, or open Share on their phone.", .body, tone: .muted, align: .center)
                AkiText("Visible as \(UIDevice.current.name)", .caption, tone: .muted)
            } else {
                AkiText("Connect to Wi-Fi to receive.", .body, tone: .muted, align: .center)
            }
            AkiButton("Scan a code", icon: "qr", variant: .ghost) { model.scanningCode = true }
        }
    }

    // MARK: pieces

    private func centered<Body: View>(@ViewBuilder _ body: () -> Body) -> some View {
        VStack(spacing: 14) { body() }.frame(maxWidth: .infinity)
    }

    private var askTitle: String {
        guard let asked = model.asked else { return "" }
        return "\(asked.from) wants to send you \(asked.files.map(what).joined(separator: ", "))"
    }

    private func what(_ file: ShareOffered) -> String {
        let size = ByteCountFormatter.string(fromByteCount: Int64(file.size), countStyle: .file)
        switch file.kind {
        case "book": return "a book (\(file.name), \(size))"
        case "comic": return "a comic (\(file.name), \(size))"
        case "card": return "a series (\(file.name.replacingOccurrences(of: ".akidev.json", with: "")))"
        default: return "a file (\(file.name), \(size))"
        }
    }

    private var sourceMessage: String {
        guard let card = model.newSource?.card else { return "" }
        let languages = card.languages.isEmpty ? "" : " (\(card.languages.joined(separator: ", ")))"
        let adult = card.nsfw ? " It is marked 18+." : ""
        let from = card.repo.map { " It comes from the repository \($0)." } ?? " Your friend added it by hand."
        return "\"\(card.title)\" is read from \(card.sourceName)\(languages), which this phone does not have.\(adult)\(from) A source is code that runs on this phone: add it only if you trust your friend."
    }

    private func openSettings() {
        if let url = URL(string: UIApplication.openSettingsURLString) { UIApplication.shared.open(url) }
    }
}

extension SharePeer: @retroactive Identifiable {
    public var id: String { "\(ip):\(port)" }
}
