import SwiftUI
import UIKit

/// When the warm tint is on.
enum BookWarmth: String, Codable, CaseIterable {
    case off, always, night

    var title: String { self == .night ? "At night" : rawValue.capitalized }

    /// Night is a clock window, so no location is asked for.
    func isOn(at date: Date) -> Bool {
        switch self {
        case .off: return false
        case .always: return true
        case .night:
            let hour = Calendar.current.component(.hour, from: date)
            return hour >= 20 || hour < 7
        }
    }
}

/// Brightness and warmth, laid over the page.
///
/// The slider has one scale. The upper part is the phone's own brightness; below it, the phone stays at its lowest and a dark layer
/// takes the page further down, because the lowest the phone goes is still too bright for reading in a dark bedroom. On an OLED
/// screen a darker picture is a dimmer screen, and a layer does not flicker the way dimming by pulses does.
@MainActor enum BookComfort {
    /// Where the slider turns from the phone's brightness to the dark layer.
    private static let turn = 0.3
    private static let deepest = 0.8

    /// The phone's brightness (0 to 1) and the strength of the dark layer (0 to 0.8) for a slider value.
    static func levels(for value: Double) -> (screen: CGFloat, dim: Double) {
        let value = min(max(value, 0), 1)
        if value >= turn { return (CGFloat((value - turn) / (1 - turn)), 0) }
        return (0, (1 - value / turn) * deepest)
    }

    private static var screen: UIScreen? {
        UIApplication.shared.connectedScenes.compactMap { ($0 as? UIWindowScene)?.screen }.first
    }

    private static var original: CGFloat?

    /// Sets the phone's brightness for reading (remembering what it was), or gives it back when `value` is nil.
    static func apply(_ value: Double?) {
        guard let screen else { return }
        guard let value else { return restore() }
        if original == nil { original = screen.brightness }
        screen.brightness = levels(for: value).screen
    }

    static func restore() {
        if let original { screen?.brightness = original }
        original = nil
    }
}

/// The warm tint and the dark layer over a page. They do not take touches.
struct BookComfortOverlay: View {
    let style: BookStyle

    var body: some View {
        TimelineView(.everyMinute) { context in
            ZStack {
                if style.warmth.isOn(at: context.date) {
                    Rectangle().fill(Color(red: 1, green: 0.76, blue: 0.48)).opacity(style.warmthAmount).blendMode(.multiply)
                }
                if let brightness = style.brightness, BookComfort.levels(for: brightness).dim > 0 {
                    Rectangle().fill(Color.black).opacity(BookComfort.levels(for: brightness).dim)
                }
            }
            .ignoresSafeArea()
            .allowsHitTesting(false)
        }
    }
}
