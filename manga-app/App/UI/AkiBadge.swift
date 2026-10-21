import SwiftUI

enum AkiBadgeStyle {
    /// A flat grey chip on a card.
    case flat
    /// Glass, for a picture.
    case glass
}

/// A small label: a date, a scanlator, a status.
struct AkiBadge: View {
    let text: String
    var icon: String?
    var style: AkiBadgeStyle = .flat

    init(_ text: String, icon: String? = nil, style: AkiBadgeStyle = .flat) {
        self.text = text
        self.icon = icon
        self.style = style
    }

    var body: some View {
        let tone: AkiTone = style == .glass ? .onImage : .dim
        let label = HStack(spacing: 4) {
            if let icon { AkiIcon(icon, size: 10, tone: tone) }
            AkiText(text, .captionStrong, tone: tone, lines: 1)
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 5)
        switch style {
        case .flat: label.background(Aki.chip, in: Capsule())
        case .glass: label.akiGlass(.capsule, interactive: false)
        }
    }
}

/// A count over a cover or a tab: unread chapters.
struct AkiCount: View {
    let value: Int
    var tinted = true

    var body: some View {
        AkiText("\(min(value, 99))", .captionStrong, tone: tinted ? .onYellow : .onImage)
            .monospacedDigit()
            .padding(.horizontal, 9)
            .frame(minWidth: 26, minHeight: 26)
            .akiGlass(.capsule, tint: tinted ? Aki.yellow : Aki.danger, interactive: false)
    }
}
