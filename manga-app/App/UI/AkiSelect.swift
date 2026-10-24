import SwiftUI

/// A choice among a few options, shown as a menu.
struct AkiSelect<Value: Hashable>: View {
    let title: String
    @Binding var selection: Value
    let options: [(value: Value, label: String)]

    var body: some View {
        HStack {
            AkiText(title, .bodyStrong)
            Spacer()
            Menu {
                Picker(title, selection: $selection) {
                    ForEach(options, id: \.value) { option in Text(option.label).tag(option.value) }
                }
            } label: {
                AkiText(options.first { $0.value == selection }?.label ?? "", .bodyStrong, tone: .text, lines: 1)
                // A chip, so it reads as something to tap.
                .padding(.horizontal, 14).padding(.vertical, 8)
                .background(Aki.secondary, in: Capsule())
            }
        }
    }
}
