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

/// The design tokens. Screens and components use these names only, never a raw colour. See `DESIGN.md`.
///
/// Surfaces: `background` (the screen), `surface` (cards), `secondary` (chips, fields, quiet fills).
/// Text: `text`, `textMuted`. Action: `primary` is the one accent (the chosen item, the main action) and `onPrimary`
/// is what is written on it. Over pictures: `onMedia`, with `scrim*` darkening the picture under it.
enum Aki {
    private static func adaptive(_ light: UInt32, _ dark: UInt32) -> Color {
        Color(uiColor: UIColor { $0.userInterfaceStyle == .dark ? UIColor(hex: dark) : UIColor(hex: light) })
    }

    // Surfaces
    static let background = adaptive(0xFFFFFF, 0x0D0D0E)
    static let surface = adaptive(0xF4F4F5, 0x1B1B1D)
    static let secondary = adaptive(0xE8E8EB, 0x2C2C2F)

    // Text
    static let text = adaptive(0x111111, 0xF5F5F5)
    static let textMuted = adaptive(0x6E6E73, 0xA1A1A8)

    // Action
    static let primary = Color(hex: 0xFFD21F)
    static let onPrimary = Color(hex: 0x141414)
    static let destructive = Color(hex: 0xE5484D)

    /// The soft glows behind every screen, drawn from `primary`.
    static let accent = primary.opacity(0.30)
    static let accentSoft = primary.opacity(0.16)

    // Over pictures and the reader
    static let onMedia = Color(hex: 0xFFFFFF)
    static let scrim = Color(hex: 0x000000).opacity(0.35)
    static let scrimTop = Color(hex: 0x000000).opacity(0.35)
    static let scrimBottom = Color(hex: 0x000000).opacity(0.78)
    static let scrimStrong = Color(hex: 0x000000).opacity(0.7)
    static let shadow = Color(hex: 0x000000).opacity(0.12)
    /// The reader's two backdrops.
    static let stage = Color(hex: 0x000000)
    static let stageAlt = Color(hex: 0x1C1C1C)

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
