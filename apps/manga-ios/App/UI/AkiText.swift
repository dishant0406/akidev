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
    /// Equal-width digits, so a number that changes (a counter, a size, a percentage) does not jiggle.
    var digits = false
    @Environment(\.dynamicTypeSize) private var typeSize

    init(_ text: String, _ style: AkiTextStyle = .body, tone: AkiTone = .text, lines: Int? = nil, reserves: Bool = false, align: TextAlignment = .leading, digits: Bool = false) {
        self.text = text
        self.style = style
        self.tone = tone
        self.lines = lines
        self.reserves = reserves
        self.align = align
        self.digits = digits
    }

    var body: some View {
        // At the accessibility text sizes nothing is cut short: every line limit is lifted.
        let limit = typeSize.isAccessibilitySize ? nil : lines
        Text(style == .eyebrow ? text.uppercased() : text)
            .font(digits ? font.monospacedDigit() : font)
            .tracking(tracking)
            .lineSpacing(lineSpacing)
            .foregroundStyle(tone.color)
            .lineLimit(limit ?? Int.max, reservesSpace: reserves && limit != nil)
            .multilineTextAlignment(align)
            // Screen titles are headings for VoiceOver's heading rotor.
            .accessibilityAddTraits(style == .display || style == .title ? .isHeader : [])
    }

    /// Text that wraps needs about 1.4 times its size between lines.
    private var lineSpacing: CGFloat {
        switch style {
        case .display: -2
        case .body, .bodyStrong: 3
        case .caption, .captionStrong: 2
        case .title, .heading, .eyebrow, .numeral: 0
        }
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
