import AkiManga
import SwiftUI

struct VoicesRoute: Hashable {}

/// The voice models for reading aloud: get one, choose which reads, remove it.
struct VoicesView: View {
    @Environment(AppModel.self) private var app
    @Environment(\.dismiss) private var dismiss
    @Environment(\.openURL) private var openURL

    private var model: ReadAloudModel { app.readAloud }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                AkiBackHeader(title: Copy.Listen.voiceModels, subtitle: Copy.Listen.readingAloudOnPhone, back: { dismiss() })
                if model.models.isEmpty {
                    AkiEmptyState(title: Copy.Listen.noVoiceModels, message: Copy.Listen.noVoiceModelsHint).frame(minHeight: 260)
                }
                ForEach(model.models, id: \.id) { voice in
                    AkiRow(title: voice.name, subtitle: details(of: voice)) { control(for: voice) }
                }
                if let current = model.current, !current.voices.isEmpty {
                    AkiSection(title: Copy.Listen.voice) {
                        AkiLink(value: VoiceChoiceRoute()) {
                            AkiRow(title: Copy.Listen.voiceOf(current.name), subtitle: model.chosenVoice?.name ?? Copy.Listen.standard, icon: "headphones", showsChevron: true)
                        }
                    }
                }
                // Underdog Woof's licence asks every app that offers it to credit it, with a link.
                if model.models.contains(where: { $0.id == "woof-en" }) {
                    Button { openURL(URL(string: "https://underdog.ai/woof")!) } label: {
                        AkiText(Copy.Listen.underdogCredit, .caption, tone: .muted)
                    }
                    .buttonStyle(.plain)
                    .accessibilityAddTraits(.isLink)
                }
                AkiText(Copy.Listen.modelDownloadedOnce, .caption, tone: .muted)
            }
            .akiPage(narrow: true)
        }
        .background(AkiBackdrop())
        .akiNoNavigationBar()
        .hidesTabBar()
        .akiAlert(model.failure?.title ?? "", message: model.failure?.message, isPresented: Binding(get: { model.failure != nil }, set: { if !$0 { model.failure = nil } }))
    }

    private func details(of voice: VoiceModel) -> String {
        var parts = [voice.languages.joined(separator: ", ").uppercased(), bookSize(voice.bytes), voice.licence]
        if !voice.commercial { parts.append(Copy.Listen.notForCommercialUse) }
        return parts.joined(separator: " · ")
    }

    @ViewBuilder private func control(for voice: VoiceModel) -> some View {
        if let fraction = model.progress[voice.id] {
            AkiProgressBar(fraction: fraction).frame(width: 64).accessibilityElement(children: .ignore).accessibilityLabel(Copy.Listen.downloadingVoice(voice.name)).accessibilityValue(bookPercent(fraction))
        } else if model.has(voice) {
            HStack(spacing: 8) {
                if model.hasUpdate(voice) {
                    AkiButton(Copy.Listen.update, variant: .primary, size: .small) { Task { await model.install(voice) } }.accessibilityLabel(Copy.Listen.updateVoice(voice.name))
                } else if model.current?.id == voice.id {
                    AkiText(Copy.Listen.inUse, .captionStrong, tone: .muted)
                } else {
                    AkiButton(Copy.Listen.use, size: .small) { model.use(voice) }.accessibilityLabel(Copy.Listen.useVoice(voice.name))
                }
                AkiIconButton(symbol: "trash", label: Copy.Listen.deleteVoice(voice.name), size: 44) { model.remove(voice) }
            }
        } else if voice.usable {
            AkiButton(Copy.Series.download, icon: "download", variant: .primary, size: .small) { Task { await model.install(voice) } }.accessibilityLabel(Copy.Listen.downloadVoice(voice.name))
        } else {
            AkiText(Copy.Listen.updateAkidevToUseVoice, .caption, tone: .muted)
        }
    }
}
