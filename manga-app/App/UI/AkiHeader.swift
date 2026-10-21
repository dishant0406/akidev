import SwiftUI

/// The top of a screen: a big title, a line under it, and glass buttons on the right.
struct AkiHeader<Trailing: View>: View {
    let title: String
    var subtitle: String?
    @ViewBuilder var trailing: () -> Trailing

    var body: some View {
        HStack(alignment: .center) {
            VStack(alignment: .leading, spacing: 2) {
                AkiText(title, .display)
                if let subtitle { AkiText(subtitle, .caption, tone: .muted) }
            }
            Spacer()
            trailing()
        }
    }
}

extension AkiHeader where Trailing == EmptyView {
    init(title: String, subtitle: String? = nil) {
        self.init(title: title, subtitle: subtitle) { EmptyView() }
    }
}

/// The top of a pushed screen: a back button, then the header.
struct AkiBackHeader<Trailing: View>: View {
    let title: String
    var subtitle: String?
    let back: () -> Void
    @ViewBuilder var trailing: () -> Trailing

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            AkiIconButton(symbol: "back", label: "Back", action: back)
            AkiHeader(title: title, subtitle: subtitle, trailing: trailing)
        }
    }
}

extension AkiBackHeader where Trailing == EmptyView {
    init(title: String, subtitle: String? = nil, back: @escaping () -> Void) {
        self.init(title: title, subtitle: subtitle, back: back) { EmptyView() }
    }
}
