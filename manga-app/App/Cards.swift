import AkiManga
import SwiftUI

/// A cover with a title under it, as in every grid.
struct CoverTile: View {
    let title: String
    let cover: String?
    var badge: Int?
    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            RemoteImage(url: cover, placeholder: 1.5, fill: true)
                .clipShape(RoundedRectangle(cornerRadius: Aki.radius[2]))
                .overlay(alignment: .topTrailing) {
                    if let badge, badge > 0 {
                        Text("\(badge)").font(.caption2.weight(.bold)).monospacedDigit()
                            .padding(.horizontal, 7).padding(.vertical, 3)
                            .background(Aki.accent, in: RoundedRectangle(cornerRadius: Aki.radius[0]))
                            .foregroundStyle(Aki.canvas).padding(6)
                    }
                }
            Text(title).font(.footnote.weight(.medium)).foregroundStyle(Aki.text).lineLimit(2).multilineTextAlignment(.leading)
        }
    }
}

let gridColumns = [GridItem(.adaptive(minimum: 108, maximum: 170), spacing: 14, alignment: .top)]

/// A series being opened: the source it comes from and its id.
struct SeriesRef: Hashable {
    let source: String
    let id: String
    let title: String
    let cover: String?
}
