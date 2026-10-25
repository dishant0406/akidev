import AkiManga
import SwiftUI

struct VoicesRoute: Hashable {}

/// The voice models for reading aloud: get one, choose which reads, remove it.
struct VoicesView: View {
    @Environment(AppModel.self) private var app
    @Environment(\.dismiss) private var dismiss

    private var model: ReadAloudModel { app.readAloud }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                AkiBackHeader(title: "Voice models", subtitle: "Reading aloud happens on this phone", back: { dismiss() })
                if model.models.isEmpty {
                    AkiEmptyState(title: "No models yet", message: "The voice repository has none to offer right now.").frame(minHeight: 260)
                }
                ForEach(model.models, id: \.id) { voice in
                    AkiRow(title: voice.name, subtitle: details(of: voice)) { control(for: voice) }
                }
                if let current = model.current, !current.voices.isEmpty {
                    AkiSection(title: "Voice") {
                        AkiLink(value: VoiceChoiceRoute()) {
                            AkiRow(title: "Voice of \(current.name)", subtitle: model.chosenVoice?.name ?? "Standard", icon: "headphones", showsChevron: true)
                        }
                    }
                }
                AkiText("A model is downloaded once, checked, and then reads books with no network. Nothing you read leaves your phone.", .caption, tone: .muted)
            }
            .akiPage(narrow: true)
        }
        .background(AkiBackdrop())
        .akiNoNavigationBar()
        .hidesTabBar()
        .akiAlert(model.failure?.title ?? "", message: model.failure?.message, isPresented: Binding(get: { model.failure != nil }, set: { if !$0 { model.failure = nil } }))
    }

    private func details(of voice: VoiceModel) -> String {
        var parts = [voice.languages.joined(separator: ", ").uppercased(), ByteCountFormatter.string(fromByteCount: Int64(voice.bytes), countStyle: .file), voice.licence]
        if !voice.commercial { parts.append("not for commercial use") }
        return parts.joined(separator: " · ")
    }

    @ViewBuilder private func control(for voice: VoiceModel) -> some View {
        if let fraction = model.progress[voice.id] {
            AkiProgressBar(fraction: fraction).frame(width: 64)
        } else if model.has(voice) {
            HStack(spacing: 8) {
                if model.hasUpdate(voice) {
                    AkiButton("Update", variant: .primary, size: .small) { Task { await model.install(voice) } }
                } else if model.current?.id == voice.id {
                    AkiText("In use", .captionStrong, tone: .muted)
                } else {
                    AkiButton("Use", size: .small) { model.use(voice) }
                }
                AkiIconButton(symbol: "trash", label: "Remove \(voice.name)", size: 44) { model.remove(voice) }
            }
        } else if voice.usable {
            AkiButton("Get", icon: "download", variant: .primary, size: .small) { Task { await model.install(voice) } }
        } else {
            AkiText("Needs a newer Akidev", .caption, tone: .muted)
        }
    }
}
