import SwiftUI

/// A line in a card: a yellow icon tile, a title, a line under it, something on the right.
struct AkiRow<Trailing: View>: View {
    let title: String
    var subtitle: String?
    var icon: String?
    var showsChevron = false
    @ViewBuilder var trailing: () -> Trailing

    var body: some View {
        AkiCard {
            HStack(spacing: 14) {
                if let icon {
                    AkiIcon(icon, size: 18, tone: .onYellow)
                        .frame(width: 46, height: 46)
                        .background(Aki.yellow, in: RoundedRectangle(cornerRadius: Aki.thumbRadius, style: .continuous))
                }
                VStack(alignment: .leading, spacing: 3) {
                    AkiText(title, .heading, lines: 1)
                    if let subtitle { AkiText(subtitle, .caption, tone: .dim, lines: 1) }
                }
                Spacer(minLength: 8)
                trailing()
                if showsChevron { AkiIcon("chevron.right", size: 13, tone: .dim) }
            }
        }
    }
}

extension AkiRow where Trailing == EmptyView {
    init(title: String, subtitle: String? = nil, icon: String? = nil, showsChevron: Bool = false) {
        self.init(title: title, subtitle: subtitle, icon: icon, showsChevron: showsChevron) { EmptyView() }
    }
}
