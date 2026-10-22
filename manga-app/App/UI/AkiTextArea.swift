import SwiftUI

/// A glass box for text that runs over many lines, in a monospaced face: a pasted definition.
struct AkiTextArea: View {
    @Binding var text: String
    var height: CGFloat = 160

    var body: some View {
        TextEditor(text: $text)
            .font(.system(.footnote, design: .monospaced))
            .foregroundStyle(Aki.text)
            .scrollContentBackground(.hidden)
            .autocorrectionDisabled()
            .textInputAutocapitalization(.never)
            // A fixed height: a long paste scrolls inside the box instead of pushing the buttons below it off the screen.
            .frame(height: height)
            .padding(10)
            .akiGlass(.rounded(18), interactive: false)
    }
}
