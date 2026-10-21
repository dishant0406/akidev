import SwiftUI

/// Pastes what is on the clipboard without the system's paste prompt: the one button iOS lets read it quietly.
struct AkiPasteButton: View {
    let onPaste: (String) -> Void

    var body: some View {
        PasteButton(payloadType: String.self) { strings in
            if let text = strings.first { onPaste(text) }
        }
        .labelStyle(.titleAndIcon)
        .buttonBorderShape(.capsule)
        .controlSize(.large)
        .tint(Aki.yellow)
    }
}
