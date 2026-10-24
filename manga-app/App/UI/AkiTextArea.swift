import SwiftUI

/// A glass box for text that runs over many lines: a pasted definition (monospaced), or words of your own (`prose`).
struct AkiTextArea: View {
    @Binding var text: String
    var height: CGFloat = 160
    var prose = false

    var body: some View {
        TextEditor(text: $text)
            .font(prose ? .system(.body, design: .rounded) : .system(.footnote, design: .monospaced))
            .foregroundStyle(Aki.text)
            .scrollContentBackground(.hidden)
            .autocorrectionDisabled(!prose)
            .textInputAutocapitalization(prose ? .sentences : .never)
            // A fixed height: a long paste scrolls inside the box instead of pushing the buttons below it off the screen.
            .frame(height: height)
            .padding(10)
            .akiGlass(.rounded(18), interactive: false)
    }
}
