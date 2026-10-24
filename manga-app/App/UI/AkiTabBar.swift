import SwiftUI

/// The floating tab bar: a glass capsule, with the current tab as a yellow glass circle that flows to the next.
struct AkiTabBar: View {
    var tabs: [AppTab] = AppTab.allCases
    @Binding var selection: AppTab
    var badge: [AppTab: Int] = [:]
    @Namespace private var space
    /// The bar has the corner radius of a card; the selected item sits inside it with the padding taken off, so the curves are concentric.
    private let padding: CGFloat = 6
    private var inner: CGFloat { Aki.cardRadius - padding }
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        AkiGlassGroup(spacing: 8) {
            HStack(spacing: 6) {
                ForEach(tabs, id: \.self) { tab in
                    let on = tab == selection
                    Button {
                        withAnimation(AkiMotion.settle(reduceMotion)) { selection = tab }
                    } label: {
                        AkiIcon(tab.symbol(selected: on), size: 24, tone: on ? .onPrimary : .muted)
                            .frame(width: 62, height: 58)
                            .contentShape(RoundedRectangle(cornerRadius: inner, style: .continuous))
                            .overlay(alignment: .topTrailing) {
                                if let count = badge[tab], count > 0 { AkiCount(value: count, tinted: false).offset(x: -2, y: 2) }
                            }
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel(tab.title)
                    .akiGlassIf(on, .rounded(inner), tint: Aki.primary)
                    .glassEffectID(tab, in: space)
                }
            }
            .padding(padding)
            .akiGlass(.card, interactive: false)
        }
        .shadow(color: Aki.shadow, radius: 20, y: 8)
        .akiHaptic(trigger: selection)
    }
}
