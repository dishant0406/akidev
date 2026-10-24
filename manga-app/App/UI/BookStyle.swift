import SwiftUI
import UIKit

/// How a book looks to its reader: the choices in the reader's settings. Saved as one value, so a new choice is one new field.
struct BookStyle: Codable, Equatable {
    var font = BookFont.newYork
    /// Text size in points, 14 to 34.
    var size: Double = 20
    /// How tall a line is, as a multiple of the font's own: 1.1 to 1.9.
    var lineSpacing: Double = 1.35
    var margin = BookMargin.normal
    var justify = true
    var hyphenate = true
    var theme = BookTheme.auto

    init() {}

    /// What is missing is the default, so a setting added later does not reset the ones already chosen.
    init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        let defaults = BookStyle()
        font = try values.decodeIfPresent(BookFont.self, forKey: .font) ?? defaults.font
        size = try values.decodeIfPresent(Double.self, forKey: .size) ?? defaults.size
        lineSpacing = try values.decodeIfPresent(Double.self, forKey: .lineSpacing) ?? defaults.lineSpacing
        margin = try values.decodeIfPresent(BookMargin.self, forKey: .margin) ?? defaults.margin
        justify = try values.decodeIfPresent(Bool.self, forKey: .justify) ?? defaults.justify
        hyphenate = try values.decodeIfPresent(Bool.self, forKey: .hyphenate) ?? defaults.hyphenate
        theme = try values.decodeIfPresent(BookTheme.self, forKey: .theme) ?? defaults.theme
    }

    static let sizes: ClosedRange<Double> = 14...34
    static let spacings: ClosedRange<Double> = 1.1...1.9
}

enum BookFont: String, Codable, CaseIterable {
    case newYork, system, iowan, georgia, palatino, charter, athelas, avenir

    var title: String {
        switch self {
        case .newYork: "New York"
        case .system: "San Francisco"
        case .iowan: "Iowan"
        case .georgia: "Georgia"
        case .palatino: "Palatino"
        case .charter: "Charter"
        case .athelas: "Athelas"
        case .avenir: "Avenir Next"
        }
    }

    /// The font's name on the phone; the system faces are made, not looked up.
    fileprivate var name: String? {
        switch self {
        case .newYork, .system: nil
        case .iowan: "IowanOldStyle-Roman"
        case .georgia: "Georgia"
        case .palatino: "Palatino-Roman"
        case .charter: "Charter-Roman"
        case .athelas: "Athelas-Regular"
        case .avenir: "AvenirNext-Regular"
        }
    }

    func font(size: CGFloat, bold: Bool, italic: Bool, mono: Bool) -> UIFont {
        var traits: UIFontDescriptor.SymbolicTraits = []
        if bold { traits.insert(.traitBold) }
        if italic { traits.insert(.traitItalic) }
        let base: UIFont
        if mono {
            base = UIFont.monospacedSystemFont(ofSize: size * 0.9, weight: bold ? .bold : .regular)
            return italic ? UIFont(descriptor: base.fontDescriptor.withSymbolicTraits(.traitItalic) ?? base.fontDescriptor, size: base.pointSize) : base
        } else if let name, let named = UIFont(name: name, size: size) {
            base = named
        } else {
            let system = UIFont.systemFont(ofSize: size)
            base = self == .newYork ? UIFont(descriptor: system.fontDescriptor.withDesign(.serif) ?? system.fontDescriptor, size: size) : system
        }
        guard !traits.isEmpty, let descriptor = base.fontDescriptor.withSymbolicTraits(traits) else { return base }
        return UIFont(descriptor: descriptor, size: size)
    }
}

enum BookMargin: String, Codable, CaseIterable {
    case narrow, normal, wide

    var title: String { rawValue.capitalized }
    var points: CGFloat {
        switch self {
        case .narrow: 16
        case .normal: 28
        case .wide: 44
        }
    }
}

/// The reading surface: paper and ink. Not the app's tokens, which are for the app's own screens.
enum BookTheme: String, Codable, CaseIterable {
    /// White by day, night by night: whichever the phone is in.
    case auto, paper, sepia, night, black

    var title: String {
        switch self {
        case .auto: "Auto"
        case .paper: "Paper"
        case .sepia: "Sepia"
        case .night: "Night"
        case .black: "Black"
        }
    }

    /// The theme itself, with `auto` settled by the phone's appearance.
    func resolved(dark: Bool) -> BookTheme { self == .auto ? (dark ? .night : .paper) : self }

    var paper: UIColor {
        switch self {
        case .auto, .paper: UIColor(hex: 0xFAFAF8)
        case .sepia: UIColor(hex: 0xF3EAD7)
        case .night: UIColor(hex: 0x1C1C1E)
        case .black: UIColor(hex: 0x000000)
        }
    }

    var ink: UIColor {
        switch self {
        case .auto, .paper: UIColor(hex: 0x1A1A1A)
        case .sepia: UIColor(hex: 0x4B3B2A)
        case .night: UIColor(hex: 0xD6D6DA)
        case .black: UIColor(hex: 0xBDBDC2)
        }
    }

    /// Chapter name and page number under the text.
    var quiet: UIColor { ink.withAlphaComponent(0.5) }
    var isDark: Bool { self == .night || self == .black }
}

extension BookTheme {
    var paperColor: Color { Color(uiColor: paper) }
    var inkColor: Color { Color(uiColor: ink) }
}

/// The reading themes as small round swatches, the chosen one ringed in yellow.
struct BookThemePicker: View {
    @Binding var selection: BookTheme
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        HStack(spacing: 12) {
            ForEach(BookTheme.allCases, id: \.self) { theme in
                let shown = theme.resolved(dark: scheme == .dark)
                let chosen = theme == selection
                AkiTap(action: { selection = theme }) {
                    VStack(spacing: 6) {
                        ZStack {
                            Circle().fill(shown.paperColor)
                            Text("Aa").font(.system(size: 15, weight: .semibold, design: .serif)).foregroundStyle(shown.inkColor)
                        }
                        .frame(width: 54, height: 54)
                        .overlay(Circle().strokeBorder(chosen ? Aki.primary : Aki.secondary, lineWidth: chosen ? 3 : 1))
                        AkiText(theme.title, .caption, tone: chosen ? .text : .muted, lines: 1)
                    }
                }
                .accessibilityLabel(theme.title)
                .accessibilityAddTraits(chosen ? .isSelected : [])
            }
        }
        .frame(maxWidth: .infinity)
    }
}
