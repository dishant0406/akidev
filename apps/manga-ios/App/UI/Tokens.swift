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

/// The design tokens. Screens and components use these names only, never a raw colour. See `design/manga.md`.
///
/// Surfaces: `background` (the screen), `surface` (cards), `secondary` (chips, fields, quiet fills).
/// Text: `text`, `textMuted`. Action: `primary` is the one accent (the chosen item, the main action) and `onPrimary`
/// is what is written on it. Over pictures: `onMedia`, with `scrim*` darkening the picture under it.
enum Aki {
    /// The generated values are in `Generated/TokenValues.swift`. This turns a light and a dark value into one color.
    static func adaptive(_ light: UInt32, _ dark: UInt32) -> Color {
        Color(uiColor: UIColor { $0.userInterfaceStyle == .dark ? UIColor(hex: dark) : UIColor(hex: light) })
    }

    private static var scene: UIWindowScene? {
        UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first
    }

    /// The distance from the top of the screen to below the status bar.
    static var safeTop: CGFloat { scene?.keyWindow?.safeAreaInsets.top ?? 59 }

    /// The screen the app is on, in points; a phone's size until a window exists.
    static var screenSize: CGSize { scene?.screen.bounds.size ?? CGSize(width: 393, height: 852) }
    static var screenScale: CGFloat { scene?.screen.scale ?? 3 }
}
