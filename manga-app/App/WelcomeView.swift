import SwiftUI

/// The first screen of a fresh install: the app ships with no source, so the way in is an address.
struct WelcomeView: View {
    @Environment(AppModel.self) private var model
    @State private var typing = false
    @State private var typed = ""

    var body: some View {
        VStack(spacing: 22) {
            Spacer(minLength: 40)
            AkiIcon("library-solid", size: 54, tone: .text)
                .frame(width: 112, height: 112)
                .akiGlass(.circle, tint: Aki.primary, interactive: false)
            VStack(spacing: 10) {
                AkiText("Welcome to Akidev", .display, align: .center)
                AkiText("Akidev is a reader. It comes with no manga: you add a repository of sources, a folder usually kept on GitHub, and read what those sources describe.", .body, tone: .muted, align: .center)
            }
            VStack(spacing: 12) {
                AkiPasteButton { model.propose(repository: $0) }
                AkiButton("Type an address", icon: "keyboard", fullWidth: true) { typing = true }
            }
            .padding(.top, 8)
            AkiText("Only add repositories you trust.", .caption, tone: .muted, align: .center)
            Spacer(minLength: 40)
        }
        .padding(.horizontal, 28)
        .frame(maxWidth: .infinity, minHeight: 560)
        .akiPrompt("Add a repository", message: "The address of a repository's folder on GitHub, or any https folder with an index.json.", placeholder: "github.com/owner/repository", confirm: "Continue", text: $typed, isPresented: $typing) {
            model.propose(repository: typed)
            typed = ""
        }
    }
}
