import SwiftUI

/// The first screen of a fresh install: the app ships with no source, so the way in is an address.
struct WelcomeView: View {
    @Environment(AppModel.self) private var model
    @State private var typing = false
    @State private var typed = ""

    var body: some View {
        VStack(spacing: 22) {
            Spacer(minLength: 40)
            AkiLogo(size: 120)
                .shadow(color: Aki.shadow, radius: 20, y: 8)
            VStack(spacing: 10) {
                AkiText("Welcome to Akidev", .display, align: .center)
                AkiText("Akidev is a reader. It comes with no manga or books. Add a repository, which is a folder of sources, and read what those sources offer.", .body, tone: .muted, align: .center)
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
        .akiPrompt("Add a repository", message: "Enter the address of a repository, for example a folder on GitHub.", placeholder: "github.com/owner/repository", confirm: "Continue", text: $typed, isPresented: $typing) {
            model.propose(repository: typed)
            typed = ""
        }
    }
}
