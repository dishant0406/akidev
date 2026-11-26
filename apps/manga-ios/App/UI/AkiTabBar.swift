import SwiftUI

/// The floating tab bar: a glass capsule, with the current tab as a yellow glass circle that flows to the next.
struct AkiTabBar: View {
    let tabs: [AppTab]
    var kind: ReaderKind?
    @Binding var selection: AppTab
    var badge: [AppTab: Int] = [:]
    /// A tap on the tab that is already open.
    var onReselect: (AppTab) -> Void = { _ in }
    @Namespace private var space
    /// The bar has the corner radius of a card; the selected item sits inside it with the padding taken off, so the curves are concentric.
    private let padding: CGFloat = 6
    private var inner: CGFloat { Aki.cardRadius - padding }
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private func unread(_ tab: AppTab) -> Int { badge[tab] ?? 0 }

    var body: some View {
        AkiGlassGroup(spacing: 8) {
            HStack(spacing: 6) {
                ForEach(tabs, id: \.self) { tab in
                    let on = tab == selection
                    Button {
                        if on { onReselect(tab) } else { withAnimation(AkiMotion.settle(reduceMotion)) { selection = tab } }
                    } label: {
                        AkiIcon(tab.symbol(selected: on), size: 24, tone: on ? .onPrimary : .muted)
                            .frame(width: 62, height: 58)
                            .contentShape(RoundedRectangle(cornerRadius: inner, style: .continuous))
                            .overlay(alignment: .topTrailing) {
                                if unread(tab) > 0 { AkiCount(value: unread(tab), tinted: false).offset(x: -2, y: 2) }
                            }
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel(tab.title(for: kind))
                    .accessibilityValue(unread(tab) > 0 ? "\(unread(tab)) unread" : "")
                    .accessibilityAddTraits(on ? .isSelected : [])
                    .akiGlassIf(on, .rounded(inner), tint: Aki.primary)
                    .glassEffectID(tab, in: space)
                }
            }
            .padding(padding)
            .akiGlass(.card, interactive: false)
            .accessibilityElement(children: .contain)
            .accessibilityAddTraits(.isTabBar)
        }
        .shadow(color: Aki.shadow, radius: 20, y: 8)
        .akiHaptic(trigger: selection)
    }
}
