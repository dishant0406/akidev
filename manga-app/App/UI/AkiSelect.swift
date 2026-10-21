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
                    AkiText(options.first { $0.value == selection }?.label ?? "", .bodyStrong, tone: .muted, lines: 1)
                    AkiIcon("sort", size: 11, tone: .muted)
                }
            }
        }
    }
}
