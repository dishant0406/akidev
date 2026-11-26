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
            AkiHeader(title: item == nil ? Copy.Share.receive : Copy.Share.send, subtitle: item?.title)
            content
        }
        .padding(24)
        .frame(maxHeight: .infinity, alignment: .top)
        .task { await model.begin(app: app, item: item) }
        .onDisappear { model.end() }
        .onChange(of: model.codeNote) { if let note = model.codeNote { akiAnnounce(note) } }
        .akiQuestion(askTitle, message: Copy.Share.onlyAcceptFromKnown, yes: Copy.Share.accept, no: Copy.Share.decline, isPresented: Binding(get: { model.asked != nil }, set: { _ in }), onYes: { model.answer(true) }, onNo: { model.answer(false) })
        .akiQuestion(Copy.Share.addSourceQuestion(model.newSource?.card.sourceName ?? ""), message: sourceMessage, yes: Copy.Share.addSourceButton, no: Copy.Sources.cancel, isPresented: Binding(get: { model.newSource != nil }, set: { _ in }), onYes: { model.answerSource(true) }, onNo: { model.answerSource(false) })
        .akiAlert(Copy.Share.couldNotShare, message: model.problem, isPresented: Binding(get: { model.problem != nil }, set: { if !$0 { model.problem = nil } }))
    }

    @ViewBuilder private var content: some View {
        if let failure = model.failure {
            centered { AkiText(failure, .body, tone: .muted, align: .center) }.onAppear { akiAnnounce(failure) }
        } else if model.denied {
            centered {
                AkiText(Copy.Share.turnOnLocalNetwork, .body, tone: .muted, align: .center)
                AkiButton(Copy.Share.openSettings, variant: .primary) { openSettings() }
            }
        } else if case .joining = model.step {
            centered {
                AkiSpinner()
                AkiText(Copy.Share.joiningWifi, .heading, align: .center)
            }
        } else if case .sending(let peer, let outcome) = model.step {
            sendingState(peer, outcome)
        } else if case .receiving(_, let from, let fraction) = model.incoming {
            centered {
                AkiText(Copy.Share.receivingFrom(from), .heading, align: .center)
                progressBar(fraction)
                AkiButton(Copy.Sources.cancel, variant: .ghost, size: .small) { model.stopReceiving() }
            }
        } else if let line = model.added.last {
            done(line)
        } else if case .connected(let name) = model.step {
            centered {
                AkiSpinner()
                AkiText(Copy.Share.connectedWaiting(name), .heading, lines: 3, align: .center)
                AkiButton(Copy.Sources.cancel, variant: .ghost, size: .small) { model.disconnect() }
            }
        } else if case .connecting = model.step {
            centered {
                AkiSpinner()
                AkiText(Copy.Share.findingPhone, .heading, align: .center)
            }
        } else if case .scanning = model.step {
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
                    AkiText(Copy.Share.lookingNearby, .body, tone: .muted)
                    if model.looked { AkiText(Copy.Share.cannotSeePhone, .caption, tone: .muted, align: .center) }
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
                                .accessibilityLabel(Copy.Share.sendToPeer(peer.alias))
                                .accessibilityAddTraits(.isButton)
                        }
                    }
                }
            }
            AkiButton(Copy.Share.scanTheirCode, icon: "qr", fullWidth: true) { model.step = .scanning }
        }
        .frame(maxHeight: .infinity)
    }

    private var scanner: some View {
        VStack(spacing: 14) {
            AkiScanner { model.connect(code: $0) }
            if let note = model.codeNote { AkiText(note, .caption, tone: .destructive, align: .center) }
            AkiButton(Copy.Sources.cancel, variant: .ghost, size: .small) { model.step = .choosing; model.codeNote = nil }
        }
        .frame(maxWidth: .infinity)
    }

    @ViewBuilder private func sendingState(_ peer: SharePeer, _ outcome: ShareModel.Outcome) -> some View {
        switch outcome {
        case .waiting:
            centered {
                AkiSpinner()
                AkiText(Copy.Share.waitingFor(peer.alias), .heading, align: .center)
                AkiButton(Copy.Sources.cancel, variant: .ghost, size: .small) { model.stopSending() }
            }
        case .sending(let fraction):
            centered {
                AkiText(Copy.Share.sendingTo(peer.alias), .heading, align: .center)
                progressBar(fraction)
                AkiText(fraction.formatted(.percent.precision(.fractionLength(0))), .caption, tone: .muted, digits: true)
                AkiButton(Copy.Sources.cancel, variant: .ghost, size: .small) { model.stopSending() }
                AkiText(Copy.Share.keepAkidevOpen, .caption, tone: .muted)
            }
        case .done:
            done(Copy.Share.sentTo(peer.alias))
        case .declined: problem(Copy.Share.peerDeclined(peer.alias))
        case .busy: problem(Copy.Share.peerBusy(peer.alias))
        case .failed(let message): problem(message)
        }
    }

    private func problem(_ line: String) -> some View {
        centered {
            AkiText(line, .heading, align: .center)
            AkiButton(Copy.Home.tryAgain, variant: .primary) { model.sendAgain() }
            AkiButton(Copy.Sources.cancel, variant: .ghost, size: .small) { dismiss() }
        }
        .onAppear { akiAnnounce(line) }
    }

    private func done(_ line: String) -> some View {
        centered {
            AkiIcon("check", size: 44)
            AkiText(line, .heading, lines: 3, align: .center)
            AkiButton(Copy.Share.done, variant: .primary) { dismiss() }
        }
        .onAppear { akiAnnounce(line) }
    }

    // MARK: receive

    private var code: some View {
        centered {
            if let text = model.code {
                AkiQRCode(text: text)
                AkiText(Copy.Share.askThemToScan, .body, tone: .muted, align: .center)
                AkiText(Copy.Share.visibleAs(UIDevice.current.name), .caption, tone: .muted)
            } else {
                AkiText(Copy.Share.connectToWifiToReceive, .body, tone: .muted, align: .center)
            }
            AkiButton(Copy.Share.scanACode, icon: "qr", variant: .ghost) { model.step = .scanning }
        }
    }

    // MARK: pieces

    private func progressBar(_ fraction: Double) -> some View {
        AkiProgressBar(fraction: fraction)
            .accessibilityElement()
            .accessibilityLabel(Copy.Share.progress)
            .accessibilityValue(fraction.formatted(.percent.precision(.fractionLength(0))))
    }

    private func centered<Body: View>(@ViewBuilder _ body: () -> Body) -> some View {
        VStack(spacing: 14) { body() }.frame(maxWidth: .infinity)
    }

    private var askTitle: String {
        guard let asked = model.asked else { return "" }
        return Copy.Share.wantsToSendYou(asked.from, asked.files.map(what).joined(separator: ", "))
    }

    private func what(_ file: ShareOffered) -> String {
        let size = bookSize(file.size)
        switch file.kind {
        case "book": return Copy.Share.whatBook(file.name, size)
        case "comic": return Copy.Share.whatComic(file.name, size)
        case "card": return Copy.Share.whatSeries(file.name.replacingOccurrences(of: ".akidev.json", with: ""))
        default: return Copy.Share.whatFile(file.name, size)
        }
    }

    private var sourceMessage: String {
        guard let card = model.newSource?.card else { return "" }
        let source = card.languages.isEmpty ? card.sourceName : Copy.Share.sourceWithLanguages(card.sourceName, card.languages.joined(separator: ", "))
        let adult = card.nsfw ? Copy.Share.markedAdult : nil
        let from = card.repo.map { Copy.Share.fromRepository($0) } ?? Copy.Share.addedByHand
        let details = [adult, from].compactMap { $0 }.joined(separator: " ")
        return Copy.Share.sourceMessage(card.title, source, details)
    }

    private func openSettings() {
        if let url = URL(string: UIApplication.openSettingsURLString) { UIApplication.shared.open(url) }
    }
}

extension SharePeer: @retroactive Identifiable {
    public var id: String { "\(ip):\(port)" }
}
