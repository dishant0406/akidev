import SwiftUI

enum AppTab: Hashable, CaseIterable {
    case home, library, updates, sources

    var title: String {
        switch self {
        case .home: "Home"
        case .library: "Library"
        case .updates: "Updates"
        case .sources: "Sources"
        }
    }

    func symbol(selected: Bool) -> String {
        switch self {
        case .home: selected ? "house.fill" : "house"
        case .library: selected ? "books.vertical.fill" : "books.vertical"
        case .updates: selected ? "tray.fill" : "tray"
        case .sources: selected ? "puzzlepiece.extension.fill" : "puzzlepiece.extension"
        }
    }
}

/// The floating tab bar: a glass capsule, with the current tab as a yellow glass circle that flows to the next.
struct FloatingTabBar: View {
    @Binding var selection: AppTab
    var badge: [AppTab: Int] = [:]
    @Namespace private var space

    var body: some View {
        GlassEffectContainer(spacing: 8) {
            HStack(spacing: 6) {
                ForEach(AppTab.allCases, id: \.self) { tab in
                    let on = tab == selection
                    Button {
                        withAnimation(.smooth(duration: 0.32)) { selection = tab }
                    } label: {
                        Image(systemName: tab.symbol(selected: on))
                            .font(.system(size: 20, weight: .semibold))
                            .foregroundStyle(on ? Aki.onYellow : Aki.dim)
                            .frame(width: 70, height: 58)
                            .contentShape(Circle())
                            .overlay(alignment: .topTrailing) {
                                if let count = badge[tab], count > 0 {
                                    Text("\(min(count, 99))").font(.system(size: 10, weight: .bold, design: .rounded)).foregroundStyle(.white)
                                        .padding(.horizontal, 5).frame(minWidth: 17, minHeight: 17)
                                        .background(Aki.danger, in: Capsule()).offset(x: -4, y: 4)
                                }
                            }
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel(tab.title)
                    // Only the current tab is a glass circle of its own, inside the bar's glass: glass does not stack.
                    .glassEffect(on ? .regular.tint(Aki.yellow).interactive() : .identity, in: .circle)
                    .glassEffectID(tab, in: space)
                }
            }
            .padding(6)
            .glassEffect(.regular, in: .capsule)
        }
        .shadow(color: .black.opacity(0.12), radius: 20, y: 8)
    }
}
