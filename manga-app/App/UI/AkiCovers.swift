import SwiftUI

/// A cover with its title: the grid card on Home and in the Library.
struct AkiCoverCard: View {
    let title: String
    let cover: String?
    var subtitle: String?
    var badge: Int?

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            AkiImage(url: cover, placeholder: 1.4, fill: true)
                .clipShape(RoundedRectangle(cornerRadius: Aki.cardRadius, style: .continuous))
                .overlay(alignment: .topTrailing) {
                    if let badge, badge > 0 { AkiCount(value: badge).padding(8) }
                }
            AkiText(title, .heading, lines: 2)
            if let subtitle { AkiText(subtitle, .caption, tone: .muted, lines: 1) }
        }
    }
}

/// A row in a list of cards: a thumbnail, a small line, a bold title, badges. Updates, History and chapters.
struct AkiListCard<Trailing: View>: View {
    let thumbnail: String?
    let label: String
    let title: String
    var badges: [String] = []
    var dimmed = false
    @ViewBuilder var trailing: () -> Trailing

    var body: some View {
        AkiCard(padding: 10, dimmed: dimmed) {
            HStack(spacing: 14) {
                AkiImage(url: thumbnail, placeholder: 1.25, fill: true)
                    .frame(width: 66)
                    .clipShape(RoundedRectangle(cornerRadius: Aki.thumbRadius, style: .continuous))
                VStack(alignment: .leading, spacing: 6) {
                    AkiText(label, .caption, tone: .muted, lines: 1)
                    AkiText(title, .heading, lines: 2)
                    if !badges.isEmpty {
                        HStack(spacing: 6) { ForEach(badges, id: \.self) { AkiBadge($0) } }
                    }
                }
                Spacer(minLength: 0)
                trailing()
            }
        }
    }
}

extension AkiListCard where Trailing == EmptyView {
    init(thumbnail: String?, label: String, title: String, badges: [String] = [], dimmed: Bool = false) {
        self.init(thumbnail: thumbnail, label: label, title: title, badges: badges, dimmed: dimmed) { EmptyView() }
    }
}
