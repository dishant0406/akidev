import SwiftUI

struct AkiMenuItem: Identifiable {
    let id = UUID()
    let title: String
    let icon: String
    var destructive = false
    var isSelected = false
    let action: () -> Void
}

/// One row of a menu: its icon (a tick when it is the chosen one) and its title.
private struct AkiMenuButton: View {
    let item: AkiMenuItem

    var body: some View {
        Button(role: item.destructive ? .destructive : nil, action: item.action) {
            Label { Text(item.title) } icon: { Image("solar-\(item.isSelected ? "tick" : item.icon)") }
        }
    }
}

/// A glass round button that opens a menu.
struct AkiMenu: View {
    let symbol: String
    let label: String
    var onImage = false
    let items: [AkiMenuItem]

    var body: some View {
        Menu {
            ForEach(items) { item in
                AkiMenuButton(item: item)
            }
        } label: {
            AkiIconButton(symbol: symbol, label: label, onImage: onImage) {}.face
        }
        .akiGlass(.circle)
        .accessibilityLabel(label)
    }
}

extension View {
    /// Items that appear on a long press.
    func akiContextMenu(_ items: [AkiMenuItem]) -> some View {
        contextMenu {
            ForEach(items) { item in
                AkiMenuButton(item: item)
            }
        }
    }
}
