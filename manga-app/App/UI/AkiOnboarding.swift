import SwiftUI

/// Slow soft light behind the first screen: yellow glows that drift, so the glass above always has something moving to refract.
/// Still under Reduce Motion.
struct AkiAurora: View {
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        TimelineView(.animation(minimumInterval: 1 / 30, paused: reduceMotion)) { timeline in
            let t = reduceMotion ? 0 : timeline.date.timeIntervalSinceReferenceDate
            ZStack {
                Aki.background
                glow(Aki.primary.opacity(0.55), size: 380, x: 150 * sin(t / 7), y: -280 + 60 * cos(t / 5))
                glow(Aki.primary.opacity(0.30), size: 320, x: -160 + 50 * cos(t / 6), y: 300 + 70 * sin(t / 8))
                glow(Aki.primary.opacity(0.20), size: 260, x: 120 * cos(t / 9), y: 20 * sin(t / 4))
            }
        }
        .drawingGroup()
        .ignoresSafeArea()
    }

    private func glow(_ colour: Color, size: CGFloat, x: Double, y: Double) -> some View {
        Circle().fill(colour).frame(width: size).blur(radius: 90).offset(x: x, y: y)
    }
}

/// One of two big choices, as a tall glass card with a drawing on top. The chosen one fills with yellow and the other steps back.
struct AkiChoiceCard<Art: View>: View {
    let title: String
    let detail: String
    var state = State.rest
    let action: () -> Void
    @ViewBuilder var art: () -> Art

    enum State { case rest, chosen, passed }

    var body: some View {
        Button(action: action) {
            VStack(alignment: .leading, spacing: 14) {
                art()
                    .frame(maxWidth: .infinity)
                    .frame(height: 150)
                VStack(alignment: .leading, spacing: 4) {
                    AkiText(title, .heading, tone: state == .chosen ? .onPrimary : .text, lines: 2, reserves: true)
                    AkiText(detail, .caption, tone: state == .chosen ? .onPrimary : .muted, lines: 3, reserves: true)
                }
            }
            .padding(16)
            .frame(maxWidth: .infinity, alignment: .leading)
            .contentShape(RoundedRectangle(cornerRadius: Aki.cardRadius, style: .continuous))
        }
        .buttonStyle(AkiPressStyle())
        .akiGlass(.card, tint: state == .chosen ? Aki.primary : nil)
        .scaleEffect(state == .chosen ? 1.03 : state == .passed ? 0.95 : 1)
        .opacity(state == .passed ? 0.4 : 1)
        .accessibilityLabel(title)
        .accessibilityHint(detail)
    }
}

/// Three comic pages fanned out, with a speech bubble on the front one.
struct AkiMangaArt: View {
    var body: some View {
        ZStack {
            page(Aki.secondary, angle: -10, x: -26)
            page(Aki.text.opacity(0.85), angle: 9, x: 26)
            page(Aki.primary, angle: -1, x: 0)
                .overlay {
                    VStack(spacing: 6) {
                        RoundedRectangle(cornerRadius: 6, style: .continuous).fill(Aki.onPrimary.opacity(0.18)).frame(width: 62, height: 44)
                        HStack(spacing: 6) {
                            RoundedRectangle(cornerRadius: 6, style: .continuous).fill(Aki.onPrimary.opacity(0.18)).frame(width: 28, height: 30)
                            Ellipse().fill(Aki.background).frame(width: 30, height: 22)
                        }
                    }
                }
        }
        .accessibilityHidden(true)
    }

    private func page(_ fill: Color, angle: Double, x: CGFloat) -> some View {
        RoundedRectangle(cornerRadius: 12, style: .continuous)
            .fill(fill)
            .frame(width: 76, height: 114)
            .shadow(color: Aki.shadow, radius: 8, y: 4)
            .rotationEffect(.degrees(angle))
            .offset(x: x)
    }
}

/// Three books standing on a shelf, one of them with a ribbon.
struct AkiBooksArt: View {
    var body: some View {
        HStack(alignment: .bottom, spacing: 6) {
            spine(Aki.text.opacity(0.85), height: 112, width: 30)
            spine(Aki.primary, height: 132, width: 38).overlay(alignment: .top) {
                Rectangle().fill(Aki.destructive).frame(width: 8, height: 34).padding(.top, -4)
            }
            spine(Aki.secondary, height: 100, width: 28)
            spine(Aki.primary.opacity(0.55), height: 120, width: 32)
        }
        .padding(.bottom, 4)
        .overlay(alignment: .bottom) { Capsule().fill(Aki.text.opacity(0.25)).frame(width: 150, height: 4).offset(y: 4) }
        .accessibilityHidden(true)
    }

    private func spine(_ fill: Color, height: CGFloat, width: CGFloat) -> some View {
        RoundedRectangle(cornerRadius: 6, style: .continuous)
            .fill(fill)
            .frame(width: width, height: height)
            .overlay(alignment: .top) { Capsule().fill(Aki.onMedia.opacity(0.5)).frame(width: width - 12, height: 3).padding(.top, 14) }
            .shadow(color: Aki.shadow, radius: 6, y: 3)
    }
}
