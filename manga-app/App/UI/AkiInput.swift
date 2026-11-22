import SwiftUI

/// A glass text field with an icon and a clear button. `secure` hides what is typed (a password) and `email` asks for the
/// email keyboard; both are for a form, so Return goes to the next field instead of searching.
struct AkiInput: View {
    let placeholder: String
    @Binding var text: String
    var icon: String?
    var isURL = false
    var secure = false
    var email = false
    /// The name VoiceOver reads for the field; without it, the placeholder.
    var label: String?
    var onSubmit: () -> Void = {}
    var onClear: () -> Void = {}
    @FocusState private var focused: Bool

    var body: some View {
        HStack(spacing: 10) {
            if let icon { AkiIcon(icon, size: 16, tone: .muted) }
            field
                .font(.system(.body, design: .rounded))
                .foregroundStyle(Aki.text)
                .submitLabel(secure ? .go : email ? .done : isURL ? .go : .search)
                .keyboardType(email ? .emailAddress : isURL ? .URL : .default)
                .textContentType(contentType)
                .accessibilityLabel(label ?? placeholder)
                .autocorrectionDisabled()
                .textInputAutocapitalization(.never)
                .focused($focused)
                .onSubmit {
                    focused = false
                    onSubmit()
                }
            if !text.isEmpty && !secure {
                Button { text = ""; onClear() } label: { AkiIcon("close-solid", size: 16, tone: .muted) }
                    .buttonStyle(.plain)
                    // The hit area grows to 44 pt without moving anything.
                    .padding(14).contentShape(Rectangle()).padding(-14)
                    .accessibilityLabel("Clear text")
            }
            // While typing, a way out that does not need a search: closes the keyboard.
            if focused {
                Button { focused = false } label: { AkiText("Done", .bodyStrong, tone: .text) }
                    .buttonStyle(.plain)
                    .padding(10).contentShape(Rectangle()).padding(-10)
                    .transition(.opacity)
            }
        }
        .padding(.horizontal, 18)
        .frame(minHeight: 52)
        .akiGlass(.capsule, interactive: false)
        // The whole capsule opens the keyboard, not only the line of text in it.
        .contentShape(Capsule())
        .onTapGesture { focused = true }
        .animation(.easeOut(duration: 0.15), value: focused)
    }

    @ViewBuilder private var field: some View {
        if secure { SecureField(placeholder, text: $text) } else { TextField(placeholder, text: $text) }
    }

    /// `.oneTimeCode` keeps iOS from offering to save what is typed: Akidev keeps a session, not the password.
    private var contentType: UITextContentType? {
        if secure { return .oneTimeCode }
        return email ? .emailAddress : isURL ? .URL : nil
    }
}

/// A form field: its name above, the field below. The name stays when the field fills, which a placeholder does not.
struct AkiField: View {
    let label: String
    @Binding var text: String
    var placeholder = ""
    var secure = false
    var email = false
    var onSubmit: () -> Void = {}

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            // The field itself carries the label for VoiceOver, so the text above is not read twice.
            AkiText(label, .captionStrong, tone: .muted).padding(.leading, 18).accessibilityHidden(true)
            AkiInput(placeholder: placeholder, text: $text, secure: secure, email: email, label: label, onSubmit: onSubmit)
        }
    }
}
