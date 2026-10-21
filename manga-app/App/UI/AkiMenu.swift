import SwiftUI

struct AkiMenuItem: Identifiable {
    let id = UUID()
    let title: String
    let icon: String
    var destructive = false
    var isSelected = false
    let action: () -> Void
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
                Button(role: item.destructive ? .destructive : nil, action: item.action) { Label(item.title, systemImage: item.isSelected ? "checkmark" : item.icon) }
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
                Button(role: item.destructive ? .destructive : nil, action: item.action) { Label(item.title, systemImage: item.isSelected ? "checkmark" : item.icon) }
            }
        }
    }
}
