import AkiManga
import SwiftUI

/// A series being opened: the source it comes from and its id.
struct SeriesRef: Hashable {
    let source: String
    let id: String
    let title: String
    let cover: String?
}

/// A cover with its title: the grid card everywhere (Home, Library).
struct CoverCard: View {
    let title: String
    let cover: String?
    var subtitle: String?
    var badge: Int?

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            RemoteImage(url: cover, placeholder: 1.4, fill: true)
                .clipShape(RoundedRectangle(cornerRadius: Aki.cardRadius, style: .continuous))
                .overlay(alignment: .topTrailing) {
                    if let badge, badge > 0 {
                        Text("\(badge)").font(.system(size: 12, weight: .bold, design: .rounded)).monospacedDigit()
                            .foregroundStyle(Aki.onYellow).padding(.horizontal, 9).frame(minWidth: 26, minHeight: 26)
                            .glassEffect(.regular.tint(Aki.yellow), in: .capsule).padding(8)
                    }
                }
            Text(title).font(.system(size: 15, weight: .bold, design: .rounded)).foregroundStyle(Aki.ink).lineLimit(2).multilineTextAlignment(.leading)
            if let subtitle { Text(subtitle).font(.system(size: 12, weight: .medium, design: .rounded)).foregroundStyle(Aki.dim).lineLimit(1) }
        }
    }
}

/// A row in a list of cards: a thumbnail, a small line, a bold title, chips. The Updates and History rows.
struct InboxRow: View {
    let thumbnail: String?
    let label: String
    let title: String
    var chips: [String] = []
    var dimmed = false

    var body: some View {
        HStack(spacing: 14) {
            RemoteImage(url: thumbnail, placeholder: 1.25, fill: true)
                .frame(width: 66)
                .clipShape(RoundedRectangle(cornerRadius: Aki.thumb, style: .continuous))
            VStack(alignment: .leading, spacing: 6) {
                Text(label).font(.system(size: 12, weight: .medium, design: .rounded)).foregroundStyle(Aki.dim).lineLimit(1)
                Text(title).font(.system(size: 17, weight: .bold, design: .rounded)).foregroundStyle(Aki.ink).lineLimit(2).multilineTextAlignment(.leading)
                if !chips.isEmpty {
                    HStack(spacing: 6) { ForEach(chips, id: \.self) { Chip(text: $0) } }
                }
            }
            Spacer(minLength: 0)
        }
        .padding(10)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Aki.card, in: RoundedRectangle(cornerRadius: Aki.cardRadius, style: .continuous))
        .opacity(dimmed ? 0.62 : 1)
    }
}

func shortDate(_ ms: Int64?) -> String? {
    guard let ms else { return nil }
    return Date(timeIntervalSince1970: Double(ms) / 1000).formatted(.dateTime.month(.abbreviated).day().year())
}

func chapterName(number: Double?, title: String?) -> String {
    if let title, !title.isEmpty { return title }
    if let number { return "Chapter \(number.formatted(.number.precision(.fractionLength(0...2))))" }
    return "Chapter"
}

/// A header: a big title, an optional line under it, and glass buttons on the right.
struct ScreenHeader<Trailing: View>: View {
    let title: String
    var subtitle: String?
    @ViewBuilder var trailing: () -> Trailing

    var body: some View {
        HStack(alignment: .center) {
            VStack(alignment: .leading, spacing: 2) {
                Text(title).font(Aki.title(32)).foregroundStyle(Aki.ink)
                if let subtitle { Text(subtitle).font(.system(size: 14, weight: .medium, design: .rounded)).foregroundStyle(Aki.dim) }
            }
            Spacer()
            trailing()
        }
    }
}
