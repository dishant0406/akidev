import SwiftUI
import UIKit

extension UIColor {
    convenience init(hex: UInt32) {
        self.init(red: CGFloat((hex >> 16) & 0xFF) / 255, green: CGFloat((hex >> 8) & 0xFF) / 255, blue: CGFloat(hex & 0xFF) / 255, alpha: 1)
    }
}

extension Color {
    init(hex: UInt32) { self.init(uiColor: UIColor(hex: hex)) }
}

/// The design tokens: colours that follow light and dark, and the corner radii. See `DESIGN.md`.
enum Aki {
    private static func adaptive(_ light: UInt32, _ dark: UInt32) -> Color {
        Color(uiColor: UIColor { $0.userInterfaceStyle == .dark ? UIColor(hex: dark) : UIColor(hex: light) })
    }

    /// The one accent: the current choice and the main action.
    static let yellow = Color(hex: 0xFFD21F)
    static let onYellow = Color(hex: 0x141414)
    static let canvas = adaptive(0xFFFFFF, 0x0D0D0E)
    static let card = adaptive(0xF4F4F5, 0x1B1B1D)
    static let chip = adaptive(0xE8E8EB, 0x2C2C2F)
    static let ink = adaptive(0x111111, 0xF5F5F5)
    static let dim = adaptive(0x6E6E73, 0xA1A1A8)
    static let danger = Color(hex: 0xE5484D)

    /// Text and icons over a picture or the black reader.
    static let onImage = Color.white
    /// Darkening laid over a picture so `onImage` stays readable: a flat veil under glass, and the fades under titles.
    static let veil = Color.black.opacity(0.35)
    static let fadeTop = Color.black.opacity(0.35)
    static let fadeBottom = Color.black.opacity(0.78)
    static let fadeFeatured = Color.black.opacity(0.7)
    /// Under floating chrome (the tab bar).
    static let shadow = Color.black.opacity(0.12)
    /// The two soft yellow glows behind every screen.
    static let glowStrong = yellow.opacity(0.30)
    static let glowSoft = yellow.opacity(0.16)
    /// The reader's two backdrops.
    static let readerBlack = Color.black
    static let readerGrey = Color(white: 0.11)

    /// Inside a card with 10pt of padding the thumbnail's corner is the card's minus that padding, so the two curves are concentric.
    static let cardRadius: CGFloat = 28
    static let thumbRadius: CGFloat = 18
    static let sheetRadius: CGFloat = 32

    private static var scene: UIWindowScene? {
        UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first
    }

    /// The distance from the top of the screen to below the status bar.
    static var safeTop: CGFloat { scene?.keyWindow?.safeAreaInsets.top ?? 59 }

    /// The screen the app is on, in points; a phone's size until a window exists.
    static var screenSize: CGSize { scene?.screen.bounds.size ?? CGSize(width: 393, height: 852) }
    static var screenScale: CGFloat { scene?.screen.scale ?? 3 }
}
