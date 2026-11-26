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
                AkiText(Copy.Welcome.title, .display, align: .center)
                AkiText(Copy.Welcome.body, .body, tone: .muted, align: .center)
            }
            VStack(spacing: 12) {
                AkiPasteButton { model.propose(repository: $0) }
                AkiButton(Copy.Welcome.typeAddressButton, icon: "keyboard", fullWidth: true) { typing = true }
            }
            .padding(.top, 8)
            AkiText(Copy.Welcome.caption, .caption, tone: .muted, align: .center)
            Spacer(minLength: 40)
        }
        .padding(.horizontal, 28)
        .frame(maxWidth: .infinity, minHeight: 560)
        .akiPrompt(Copy.Welcome.AddRepository.title, message: Copy.Welcome.AddRepository.message, placeholder: Copy.Welcome.AddRepository.placeholder, confirm: Copy.Welcome.AddRepository.confirm, text: $typed, isPresented: $typing) {
            model.propose(repository: typed)
            typed = ""
        }
    }
}
