import SwiftUI

/// A glass text field with an icon and a clear button.
struct AkiInput: View {
    let placeholder: String
    @Binding var text: String
    var icon: String?
    var isURL = false
    var onSubmit: () -> Void = {}
    var onClear: () -> Void = {}
    @FocusState private var focused: Bool

    var body: some View {
        HStack(spacing: 10) {
            if let icon { AkiIcon(icon, size: 16, tone: .muted) }
            TextField(placeholder, text: $text)
                .font(.system(.body, design: .rounded))
                .foregroundStyle(Aki.text)
                .submitLabel(isURL ? .go : .search)
                .keyboardType(isURL ? .URL : .default)
                .autocorrectionDisabled()
                .textInputAutocapitalization(.never)
                .focused($focused)
                .onSubmit {
                    focused = false
                    onSubmit()
                }
            if !text.isEmpty {
                Button { text = ""; onClear() } label: { AkiIcon("close-solid", size: 16, tone: .muted) }
                    .buttonStyle(.plain)
                    .accessibilityLabel("Clear")
            }
            // While typing, a way out that does not need a search: closes the keyboard.
            if focused {
                Button { focused = false } label: { AkiText("Done", .bodyStrong, tone: .text) }
                    .buttonStyle(.plain)
                    .transition(.opacity)
            }
        }
        .padding(.horizontal, 18)
        .frame(height: 52)
        .akiGlass(.capsule, interactive: false)
        // The whole capsule opens the keyboard, not only the line of text in it.
        .contentShape(Capsule())
        .onTapGesture { focused = true }
        .animation(.easeOut(duration: 0.15), value: focused)
    }
}
