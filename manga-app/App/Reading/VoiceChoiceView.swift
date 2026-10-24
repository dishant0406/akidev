import AkiManga
import SwiftUI

struct VoiceChoiceRoute: Hashable {}

/// The voices of the model that reads: hear one, choose one. A voice is fetched (a few hundred kilobytes) when first heard or chosen.
struct VoiceChoiceView: View {
    @Environment(AppModel.self) private var app
    @Environment(\.dismiss) private var dismiss

    private var model: ReadAloudModel { app.readAloud }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 22) {
                AkiBackHeader(title: "Voice", subtitle: model.current.map { "Voices of \($0.name)" }, back: { dismiss() })
                AkiSection(title: "Standard") { card(id: "", name: "\(model.current?.name ?? "Model") voice", detail: nil, description: "The model's own voice.", voice: nil) }
                ForEach(groups, id: \.title) { group in
                    AkiSection(title: group.title) {
                        ForEach(group.voices, id: \.id) { voice in
                            card(id: voice.id, name: voice.name, detail: detail(of: voice), description: voice.description, voice: voice)
                        }
                    }
                }
                AkiText("These voices differ in pace and in how much they lift and drop. Hear a few before choosing: the model reads the words, the voice sets the character.", .caption, tone: .muted)
            }
            .padding(.horizontal, 20).padding(.top, 8)
        }
        .background(AkiBackdrop())
        .akiNoNavigationBar()
        .hidesTabBar()
        .akiAlert(model.failure?.title ?? "", message: model.failure?.message, isPresented: Binding(get: { model.failure != nil }, set: { if !$0 { model.failure = nil } }))
    }

    /// The voices grouped by where they come from, in the order the model lists them.
    private var groups: [(title: String, voices: [VoiceOption])] {
        let voices = model.current?.voices ?? []
        let titles = [("en-us", "American English"), ("en-gb", "British English"), ("", "Characters")]
        return titles.map { key, title in (title, voices.filter { ($0.accent ?? "") == key }) }.filter { !$0.voices.isEmpty }
    }

    private func detail(of voice: VoiceOption) -> String? {
        let parts = [voice.gender, voice.age].compactMap { $0 }.map { $0.prefix(1).uppercased() + $0.dropFirst() }
        return parts.isEmpty ? nil : parts.joined(separator: " · ")
    }

    private func card(id: String, name: String, detail: String?, description: String, voice: VoiceOption?) -> some View {
        let busy = model.fetching.contains(id) || model.hearing == id
        return AkiCard(padding: 16) {
            VStack(alignment: .leading, spacing: 10) {
                HStack(spacing: 8) {
                    AkiText(name, .heading, lines: 1)
                    if let detail { AkiText(detail, .caption, tone: .muted, lines: 1) }
                    Spacer(minLength: 0)
                }
                AkiText(description, .caption, tone: .muted, lines: 4)
                HStack(spacing: 10) {
                    if busy { AkiSpinner().frame(width: 44, height: 44) } else { AkiIconButton(symbol: "play", label: "Hear \(name)", size: 44) { Task { await model.hear(voice) } } }
                    if model.voiceInUse == id {
                        AkiText("In use", .captionStrong, tone: .muted)
                    } else {
                        AkiButton("Use", size: .small) { Task { await model.choose(voice) } }
                    }
                    Spacer(minLength: 0)
                }
            }
        }
    }
}
