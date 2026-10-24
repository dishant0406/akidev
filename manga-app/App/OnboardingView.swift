import SwiftUI

/// The first screen of a fresh install: what does the person read? Both stay in the app; the answer only decides what comes first.
struct OnboardingView: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        VStack(spacing: 26) {
            Spacer(minLength: 24)
            AkiLogo(size: 96).shadow(color: Aki.shadow, radius: 20, y: 8)
            VStack(spacing: 10) {
                AkiText("What do you read?", .display, align: .center)
                AkiText("Akidev reads manga and books. You can use both: this only decides which comes first. Change it any time in Settings.", .body, tone: .muted, align: .center)
            }
            VStack(spacing: 14) {
                option(.manga, icon: "library", detail: "Add sources and follow series, with new chapter alerts.")
                option(.books, icon: "book", detail: "Read EPUB books from your files or a catalogue, with your place kept.")
            }
            Spacer(minLength: 24)
        }
        .padding(.horizontal, 24)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(AkiBackdrop())
    }

    private func option(_ kind: ReaderKind, icon: String, detail: String) -> some View {
        AkiTap(action: { choose(kind) }) {
            AkiCard(padding: 16) {
                HStack(spacing: 14) {
                    AkiIcon(icon, size: 22, tone: .onPrimary)
                        .frame(width: 52, height: 52)
                        .background(Aki.primary, in: RoundedRectangle(cornerRadius: Aki.thumbRadius, style: .continuous))
                    VStack(alignment: .leading, spacing: 4) {
                        AkiText(kind.title, .heading)
                        AkiText(detail, .caption, tone: .muted, lines: 3)
                    }
                    Spacer(minLength: 8)
                    AkiIcon("forward", size: 13, tone: .muted)
                }
            }
        }
        .accessibilityLabel("I read \(kind.title.lowercased())")
    }

    private func choose(_ kind: ReaderKind) {
        model.prefs.readerKind = kind
        model.tab = model.tabs.first ?? .home
    }
}
