import SwiftUI

/// Pills that switch what is shown: the chosen one is yellow glass and slides to the next.
struct AkiTabs<ID: Hashable & Sendable>: View {
    let items: [(id: ID, title: String)]
    @Binding var selection: ID
    @Namespace private var space
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        AkiGlassGroup(spacing: 10) {
            HStack(spacing: 10) {
                ForEach(items, id: \.id) { item in
                    let on = item.id == selection
                    Button {
                        withAnimation(AkiMotion.settle(reduceMotion)) { selection = item.id }
                    } label: {
                        AkiText(item.title, on ? .bodyStrong : .caption, tone: on ? .onYellow : .dim, lines: 1)
                            .frame(maxWidth: .infinity)
                            .frame(height: 46)
                            .contentShape(Capsule())
                    }
                    .buttonStyle(.plain)
                    .akiGlass(.capsule, tint: on ? Aki.yellow : nil)
                    .glassEffectID(item.id, in: space)
                }
            }
        }
        .akiHaptic(trigger: selection)
    }
}

/// A row of choices that scrolls sideways: the sources.
struct AkiChips<ID: Hashable & Sendable>: View {
    let items: [(id: ID, title: String)]
    @Binding var selection: ID
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            AkiGlassGroup(spacing: 8) {
                HStack(spacing: 8) {
                    ForEach(items, id: \.id) { item in
                        let on = item.id == selection
                        Button {
                            withAnimation(AkiMotion.settle(reduceMotion)) { selection = item.id }
                        } label: {
                            AkiText(item.title, on ? .bodyStrong : .caption, tone: on ? .onYellow : .primary, lines: 1)
                                .padding(.horizontal, 16)
                                .frame(height: 44)
                                .contentShape(Capsule())
                        }
                        .buttonStyle(.plain)
                        .akiGlass(.capsule, tint: on ? Aki.yellow : nil)
                    }
                }
            }
        }
        .scrollClipDisabled()
        .akiHaptic(trigger: selection)
    }
}
