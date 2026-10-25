import SwiftUI

/// The first screen of a fresh install: what does the person read? Both stay in the app; the answer only decides what comes first.
struct OnboardingView: View {
    @Environment(AppModel.self) private var model
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var chosen: ReaderKind?
    @State private var shown = false

    var body: some View {
        ZStack {
            AkiAurora()
            VStack(spacing: 28) {
                Spacer(minLength: 20)
                VStack(spacing: 18) {
                    AkiLogo(size: 76)
                        .padding(10)
                        .akiGlass(.rounded(34), interactive: false)
                        .shadow(color: Aki.shadow, radius: 18, y: 8)
                    VStack(spacing: 10) {
                        AkiText("What do you read?", .display, align: .center)
                        AkiText("Akidev reads manga and books, both always here. Pick what comes first.", .body, tone: .muted, align: .center)
                    }
                }
                .reveal(shown, delay: 0)
                AkiGlassGroup(spacing: 14) {
                    HStack(alignment: .top, spacing: 14) {
                        card(.manga, detail: "Follow series from your sources, with new chapter alerts.") { AkiMangaArt() }
                            .reveal(shown, delay: 0.12)
                        card(.books, detail: "EPUB books from your files or a catalogue, with your place kept.") { AkiBooksArt() }
                            .reveal(shown, delay: 0.22)
                    }
                }
                AkiText("You can change this any time in Settings.", .caption, tone: .muted, align: .center)
                    .reveal(shown, delay: 0.32)
                Spacer(minLength: 20)
            }
            .padding(.horizontal, 20)
            .frame(maxWidth: 560)
        }
        .onAppear { shown = true }
        .akiHaptic(.success, trigger: chosen)
    }

    private func card<Art: View>(_ kind: ReaderKind, detail: String, @ViewBuilder art: @escaping () -> Art) -> some View {
        AkiChoiceCard(title: kind.title, detail: detail, state: chosen == nil ? .rest : chosen == kind ? .chosen : .passed, action: { choose(kind) }, art: art)
            .disabled(chosen != nil)
    }

    /// The card takes the yellow and the other steps back, and then the app opens on what was chosen.
    private func choose(_ kind: ReaderKind) {
        withAnimation(AkiMotion.settle(reduceMotion)) { chosen = kind }
        Task {
            try? await Task.sleep(for: .milliseconds(450))
            model.prefs.readerKind = kind
            model.tab = model.tabs.first ?? .home
        }
    }
}

private extension View {
    /// Comes up from below and fades in, a moment after the one before.
    func reveal(_ shown: Bool, delay: Double) -> some View {
        opacity(shown ? 1 : 0)
            .offset(y: shown ? 0 : 24)
            .animation(.spring(duration: 0.7, bounce: 0.12).delay(delay), value: shown)
    }
}
