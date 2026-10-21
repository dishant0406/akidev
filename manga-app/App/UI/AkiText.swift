import SwiftUI

enum AkiTextStyle {
    case display, title, heading, body, bodyStrong, caption, captionStrong, eyebrow
}

enum AkiTone {
    case primary, dim, onImage, onYellow, danger
    /// For the black reader.
    case onDark

    var color: Color {
        switch self {
        case .primary: Aki.ink
        case .dim: Aki.dim
        case .onImage, .onDark: Aki.onImage
        case .onYellow: Aki.onYellow
        case .danger: Aki.danger
        }
    }
}

/// All text in the app. Sizes follow Dynamic Type; the face is rounded.
struct AkiText: View {
    let text: String
    var style: AkiTextStyle = .body
    var tone: AkiTone = .primary
    var lines: Int?
    var align: TextAlignment = .leading

    init(_ text: String, _ style: AkiTextStyle = .body, tone: AkiTone = .primary, lines: Int? = nil, align: TextAlignment = .leading) {
        self.text = text
        self.style = style
        self.tone = tone
        self.lines = lines
        self.align = align
    }

    var body: some View {
        Text(style == .eyebrow ? text.uppercased() : text)
            .font(font)
            .tracking(tracking)
            .lineSpacing(style == .display ? -2 : 0)
            .foregroundStyle(tone.color.opacity(style == .eyebrow ? 0.7 : 1))
            .lineLimit(lines)
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
        }
    }
}
