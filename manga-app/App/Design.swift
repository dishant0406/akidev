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

/// The look of Aki Manga: a bright, rounded reader with one yellow, soft grey cards, and Liquid Glass for
/// everything that floats or is touched (the tab bar, buttons, pills, chips over pictures). See `DESIGN.md`.
enum Aki {
    private static func adaptive(_ light: UInt32, _ dark: UInt32) -> Color {
        Color(uiColor: UIColor { $0.userInterfaceStyle == .dark ? UIColor(hex: dark) : UIColor(hex: light) })
    }

    /// The one accent: active pills, the active tab, the main action.
    static let yellow = Color(hex: 0xFFD21F)
    /// Text on yellow.
    static let onYellow = Color(hex: 0x141414)
    static let canvas = adaptive(0xFFFFFF, 0x0D0D0E)
    /// Cards: an episode, an update.
    static let card = adaptive(0xF4F4F5, 0x1B1B1D)
    /// Small flat chips on a card.
    static let chip = adaptive(0xE8E8EB, 0x2C2C2F)
    static let ink = adaptive(0x111111, 0xF5F5F5)
    static let dim = adaptive(0x6E6E73, 0xA1A1A8)
    static let line = adaptive(0xE6E6E9, 0x2A2A2D)
    static let danger = Color(hex: 0xE5484D)

    /// Corner radii: thumbnails, cards, the sheet that rises over a hero picture.
    static let thumb: CGFloat = 14
    static let cardRadius: CGFloat = 22
    static let sheet: CGFloat = 32

    /// Titles are rounded and bold.
    static func title(_ size: CGFloat) -> Font { .system(size: size, weight: .bold, design: .rounded) }

    /// The distance from the top of the screen to below the status bar.
    static var safeTop: CGFloat {
        let scene = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first
        return scene?.keyWindow?.safeAreaInsets.top ?? 59
    }
}

/// The page behind every screen: white with a soft yellow glow, so the glass above has something to refract.
struct AmbientBackground: View {
    var body: some View {
        ZStack {
            Aki.canvas
            Circle().fill(Aki.yellow.opacity(0.30)).frame(width: 420).blur(radius: 100).offset(x: 170, y: -300)
            Circle().fill(Aki.yellow.opacity(0.16)).frame(width: 360).blur(radius: 100).offset(x: -190, y: 320)
        }
        .ignoresSafeArea()
    }
}

/// A round glass button: back, bookmark, more.
struct GlassIconButton: View {
    let symbol: String
    var tint: Color?
    /// Over a picture the icon is white; elsewhere it is ink.
    var onImage = false
    var size: CGFloat = 44
    let action: () -> Void

    var body: some View {
        Button(action: action) { icon }
            .buttonStyle(.plain)
            .glassEffect(glass, in: .circle)
    }

    var icon: some View {
        Image(systemName: symbol)
            .font(.system(size: size * 0.38, weight: .semibold))
            .foregroundStyle(tint != nil ? Aki.onYellow : (onImage ? Color.white : Aki.ink))
            .frame(width: size, height: size)
            .contentShape(Circle())
    }

    private var glass: Glass {
        let base: Glass = tint.map { Glass.regular.tint($0) } ?? .regular
        return base.interactive()
    }
}

/// The main action: yellow glass, dark text.
struct YellowGlassButton: View {
    let title: String
    var symbol: String?
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: 8) {
                if let symbol { Image(systemName: symbol).font(.system(size: 15, weight: .bold)) }
                Text(title).font(.system(size: 16, weight: .bold, design: .rounded)).lineLimit(1)
            }
            .foregroundStyle(Aki.onYellow)
            .padding(.horizontal, 22).frame(height: 52)
            .contentShape(Capsule())
        }
        .buttonStyle(.plain)
        .glassEffect(.regular.tint(Aki.yellow).interactive(), in: .capsule)
    }
}

/// A quiet glass button: show more, try again.
struct GlassTextButton: View {
    let title: String
    let action: () -> Void
    var body: some View {
        Button(action: action) {
            Text(title).font(.system(size: 15, weight: .semibold, design: .rounded)).foregroundStyle(Aki.ink)
                .padding(.horizontal, 20).frame(height: 44).contentShape(Capsule())
        }
        .buttonStyle(.plain)
        .glassEffect(.regular.interactive(), in: .capsule)
    }
}

/// Pills that switch what is shown (Episodes, Details): the chosen one is yellow glass, and it slides.
struct PillTabs<ID: Hashable & Sendable>: View {
    let items: [(id: ID, title: String)]
    @Binding var selection: ID
    @Namespace private var space

    var body: some View {
        GlassEffectContainer(spacing: 10) {
            HStack(spacing: 10) {
                ForEach(items, id: \.id) { item in
                    let on = item.id == selection
                    Button {
                        withAnimation(.smooth(duration: 0.3)) { selection = item.id }
                    } label: {
                        Text(item.title)
                            .font(.system(size: 15, weight: on ? .bold : .medium, design: .rounded))
                            .foregroundStyle(on ? Aki.onYellow : Aki.dim)
                            .frame(maxWidth: .infinity).frame(height: 46)
                            .contentShape(Capsule())
                    }
                    .buttonStyle(.plain)
                    .glassEffect(on ? .regular.tint(Aki.yellow).interactive() : .regular.interactive(), in: .capsule)
                    .glassEffectID(item.id, in: space)
                }
            }
        }
    }
}

/// A small label: a date, a scanlator, a status.
struct Chip: View {
    let text: String
    var symbol: String?
    /// On a picture it is glass; on a card it is a flat grey chip.
    var onImage = false

    var body: some View {
        let label = HStack(spacing: 4) {
            if let symbol { Image(systemName: symbol).font(.system(size: 10, weight: .semibold)) }
            Text(text).font(.system(size: 12, weight: .semibold, design: .rounded)).lineLimit(1)
        }
        .foregroundStyle(onImage ? Color.white : Aki.dim)
        .padding(.horizontal, 10).padding(.vertical, 5)
        if onImage {
            label.glassEffect(.regular, in: .capsule)
        } else {
            label.background(Aki.chip, in: Capsule())
        }
    }
}

/// An empty or error screen: a bold line, a quiet line, and a next step that can help.
struct StateCard: View {
    let title: String
    let message: String
    var action: (label: String, run: () -> Void)?
    var body: some View {
        VStack(spacing: 10) {
            Text(title).font(Aki.title(22)).foregroundStyle(Aki.ink).multilineTextAlignment(.center)
            Text(message).font(.system(size: 15, design: .rounded)).foregroundStyle(Aki.dim).multilineTextAlignment(.center)
            if let action { GlassTextButton(title: action.label, action: action.run).padding(.top, 8) }
        }
        .padding(28)
        .frame(maxWidth: 420)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

/// Lays chips out in rows.
struct FlowLayout: Layout {
    var spacing: CGFloat = 8

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let width = proposal.width ?? .infinity
        var x: CGFloat = 0, y: CGFloat = 0, row: CGFloat = 0, widest: CGFloat = 0
        for view in subviews {
            let size = view.sizeThatFits(.unspecified)
            if x + size.width > width, x > 0 { x = 0; y += row + spacing; row = 0 }
            x += size.width + spacing; row = max(row, size.height); widest = max(widest, x - spacing)
        }
        return CGSize(width: widest, height: y + row)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        var x = bounds.minX, y = bounds.minY, row: CGFloat = 0
        for view in subviews {
            let size = view.sizeThatFits(.unspecified)
            if x + size.width > bounds.maxX, x > bounds.minX { x = bounds.minX; y += row + spacing; row = 0 }
            view.place(at: CGPoint(x: x, y: y), proposal: ProposedViewSize(size))
            x += size.width + spacing; row = max(row, size.height)
        }
    }
}

/// With a custom header the navigation bar is hidden; this keeps the swipe-from-the-edge back gesture.
extension UINavigationController: @retroactive UIGestureRecognizerDelegate {
    override open func viewDidLoad() {
        super.viewDidLoad()
        interactivePopGestureRecognizer?.delegate = self
    }

    public func gestureRecognizerShouldBegin(_ gestureRecognizer: UIGestureRecognizer) -> Bool {
        viewControllers.count > 1
    }
}
