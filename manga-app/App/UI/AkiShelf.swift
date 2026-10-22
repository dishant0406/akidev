import SwiftUI

/// A titled row of cards that scrolls sideways: one "shelf" of the home feed.
struct AkiShelf<Content: View>: View {
    let title: String
    var subtitle: String?
    @ViewBuilder var content: () -> Content

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            VStack(alignment: .leading, spacing: 2) {
                AkiText(title, .heading, lines: 1)
                if let subtitle { AkiText(subtitle, .caption, tone: .muted, lines: 1) }
            }
            ScrollView(.horizontal, showsIndicators: false) {
                LazyHStack(alignment: .top, spacing: 14) { content() }
                    .scrollTargetLayout()
            }
            .scrollTargetBehavior(.viewAligned)
            .scrollClipDisabled()
        }
    }
}

/// A cover standing up in a shelf, its title below. With a `rank` it is a "Top 10" card: a big numeral beside it.
struct AkiPoster: View {
    let title: String
    let cover: String?
    var caption: String?
    var rank: Int?
    var width: CGFloat = 128

    var body: some View {
        HStack(alignment: .bottom, spacing: 0) {
            if let rank {
                AkiText("\(rank)", .numeral, tone: .muted)
                    .frame(width: rank >= 10 ? 70 : 46, alignment: .trailing)
                    .padding(.trailing, 4)
                    .accessibilityHidden(true)
            }
            VStack(alignment: .leading, spacing: 8) {
                AkiImage(url: cover, placeholder: 1.5, fill: true)
                    .frame(width: width)
                    .clipShape(RoundedRectangle(cornerRadius: Aki.thumbRadius, style: .continuous))
                    .contentShape(RoundedRectangle(cornerRadius: Aki.thumbRadius, style: .continuous))
                AkiText(title, .captionStrong, lines: 2).frame(width: width, alignment: .leading)
                if let caption { AkiText(caption, .caption, tone: .muted, lines: 1).frame(width: width, alignment: .leading) }
            }
        }
    }
}

/// The one title the feed leads with: a tall picture, a glass tag, the title and what tapping does.
struct AkiBillboard: View {
    let title: String
    let cover: String?
    let tag: String
    var action = "Read now"

    var body: some View {
        ZStack(alignment: .bottomLeading) {
            AkiImage(url: cover, placeholder: 1.3, fill: true, free: true)
            LinearGradient(colors: [.clear, Aki.scrimBottom], startPoint: .center, endPoint: .bottom)
            VStack(alignment: .leading, spacing: 12) {
                AkiBadge(tag, icon: "sparkles", style: .glass)
                AkiText(title, .display, tone: .onMedia, lines: 2)
                AkiBadge(action, icon: "play-solid", style: .primary)
            }
            .padding(20)
        }
        .frame(height: 440)
        .clipShape(RoundedRectangle(cornerRadius: Aki.cardRadius, style: .continuous))
        .contentShape(RoundedRectangle(cornerRadius: Aki.cardRadius, style: .continuous))
    }
}
