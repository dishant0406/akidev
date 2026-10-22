import SwiftUI

/// One small pill that shows the current choice and opens a menu of the rest: for lists too long to lay out as a row
/// of pills (every source, every genre). `icon` leads the label; `clearable` adds a way back to the first option.
struct AkiMenuPill<Value: Hashable>: View {
    var icon: String?
    /// What the pill says while the first option is chosen ("Genre" rather than "All").
    var placeholder: String?
    @Binding var selection: Value
    let options: [(value: Value, label: String)]

    private var current: String {
        if let placeholder, selection == options.first?.value { return placeholder }
        return options.first { $0.value == selection }?.label ?? placeholder ?? ""
    }

    private var isChosen: Bool { placeholder != nil && selection != options.first?.value }

    var body: some View {
        Menu {
            Picker(current, selection: $selection) {
                ForEach(options, id: \.value) { option in Text(option.label).tag(option.value) }
            }
        } label: {
            HStack(spacing: 8) {
                if let icon { AkiIcon(icon, size: 15, tone: isChosen ? .onPrimary : .text) }
                AkiText(current, .bodyStrong, tone: isChosen ? .onPrimary : .text, lines: 1)
                AkiIcon("down", size: 11, tone: isChosen ? .onPrimary : .muted)
            }
            .padding(.horizontal, 16)
            .frame(height: 44)
            .contentShape(Capsule())
        }
        .akiGlass(.capsule, tint: isChosen ? Aki.primary : nil)
        .accessibilityLabel(current)
        .akiHaptic(trigger: selection)
    }
}
