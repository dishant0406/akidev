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
                HStack(spacing: 6) {
                    AkiText(options.first { $0.value == selection }?.label ?? "", .bodyStrong, tone: .text, lines: 1)
                    AkiIcon("down", size: 12, tone: .muted)
                }
                // A chip, so it reads as something to tap and the arrow belongs to it.
                .padding(.horizontal, 14).padding(.vertical, 8)
                .background(Aki.secondary, in: Capsule())
            }
        }
    }
}
