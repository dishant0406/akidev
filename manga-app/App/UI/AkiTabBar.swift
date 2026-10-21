import SwiftUI

/// The floating tab bar: a glass capsule, with the current tab as a yellow glass circle that flows to the next.
struct AkiTabBar: View {
    @Binding var selection: AppTab
    var badge: [AppTab: Int] = [:]
    @Namespace private var space

    var body: some View {
        AkiGlassGroup(spacing: 8) {
            HStack(spacing: 6) {
                ForEach(AppTab.allCases, id: \.self) { tab in
                    let on = tab == selection
                    Button {
                        withAnimation(.smooth(duration: 0.32)) { selection = tab }
                    } label: {
                        AkiIcon(tab.symbol(selected: on), size: 20, tone: on ? .onYellow : .dim)
                            .frame(width: 70, height: 58)
                            .contentShape(Circle())
                            .overlay(alignment: .topTrailing) {
                                if let count = badge[tab], count > 0 { AkiCount(value: count, tinted: false).offset(x: -2, y: 2) }
                            }
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel(tab.title)
                    .akiGlassIf(on, .circle, tint: Aki.yellow)
                    .glassEffectID(tab, in: space)
                }
            }
            .padding(6)
            .akiGlass(.capsule, interactive: false)
        }
        .shadow(color: .black.opacity(0.12), radius: 20, y: 8)
    }
}
