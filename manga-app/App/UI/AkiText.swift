import SwiftUI

enum AkiTextStyle {
    case display, title, heading, body, bodyStrong, caption, captionStrong, eyebrow
    /// A big number: the rank on a "Top 10" card.
    case numeral
}

enum AkiTone {
    case text, muted, onPrimary, onMedia, destructive

    var color: Color {
        switch self {
        case .text: Aki.text
        case .muted: Aki.textMuted
        case .onMedia: Aki.onMedia
        case .onPrimary: Aki.onPrimary
        case .destructive: Aki.destructive
        }
    }
}

/// All text in the app. Sizes follow Dynamic Type; the face is rounded.
struct AkiText: View {
    let text: String
    var style: AkiTextStyle = .body
    var tone: AkiTone = .text
    var lines: Int?
    /// Takes the room of all `lines` lines even when the text is shorter, so a row of these stays one height.
    var reserves = false
    var align: TextAlignment = .leading

    init(_ text: String, _ style: AkiTextStyle = .body, tone: AkiTone = .text, lines: Int? = nil, reserves: Bool = false, align: TextAlignment = .leading) {
        self.text = text
        self.style = style
        self.tone = tone
        self.lines = lines
        self.reserves = reserves
        self.align = align
    }

    var body: some View {
        Text(style == .eyebrow ? text.uppercased() : text)
            .font(font)
            .tracking(tracking)
            .lineSpacing(style == .display ? -2 : 0)
            .foregroundStyle(tone.color.opacity(style == .eyebrow ? 0.7 : 1))
            .lineLimit(lines ?? Int.max, reservesSpace: reserves && lines != nil)
            .multilineTextAlignment(align)
    }

    /// Big text sits tighter, small text a little looser: letters read too far apart as they grow.
    private var tracking: CGFloat {
        switch style {
        case .display: -0.8
        case .title: -0.4
        case .heading: -0.15
        case .body, .bodyStrong: 0
        case .caption, .captionStrong: 0.1
        case .eyebrow: 1.4
        case .numeral: -2
        }
    }

    private var font: Font {
        switch style {
        case .display: .system(.largeTitle, design: .rounded, weight: .bold)
        case .title: .system(.title2, design: .rounded, weight: .bold)
        case .heading: .system(.headline, design: .rounded, weight: .bold)
        case .body: .system(.body, design: .rounded)
        case .bodyStrong: .system(.body, design: .rounded, weight: .semibold)
        case .caption: .system(.footnote, design: .rounded, weight: .medium)
        case .captionStrong: .system(.caption, design: .rounded, weight: .semibold)
        case .eyebrow: .system(.caption2, design: .rounded, weight: .bold)
        case .numeral: .system(size: 72, weight: .black, design: .rounded)
        }
    }
}
