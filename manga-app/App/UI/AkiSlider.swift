import SwiftUI

/// A slider: a track with the part that is done in yellow and a round thumb. `onEnd` is called when the finger lifts.
struct AkiSlider: View {
    @Binding var value: Double
    var range: ClosedRange<Double> = 0...1
    var onEnd: () -> Void = {}
    /// The name VoiceOver reads for the slider.
    var label = ""
    @State private var dragging = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    private let thumb: CGFloat = 26

    var body: some View {
        GeometryReader { proxy in
            let width = max(proxy.size.width - thumb, 1)
            let fraction = (value - range.lowerBound) / (range.upperBound - range.lowerBound)
            ZStack(alignment: .leading) {
                Capsule().fill(Aki.secondary).frame(height: 6)
                Capsule().fill(Aki.primary).frame(width: thumb / 2 + width * fraction, height: 6)
                Circle().fill(Aki.onMedia)
                    .frame(width: thumb, height: thumb)
                    .shadow(color: Aki.shadow, radius: 4, y: 1)
                    .offset(x: width * fraction)
                    .scaleEffect(dragging && !reduceMotion ? 1.12 : 1)
            }
            .frame(maxHeight: .infinity)
            .contentShape(Rectangle())
            .gesture(DragGesture(minimumDistance: 0)
                .onChanged { drag in
                    dragging = true
                    let position = min(max((drag.location.x - thumb / 2) / width, 0), 1)
                    value = range.lowerBound + position * (range.upperBound - range.lowerBound)
                }
                .onEnded { _ in
                    dragging = false
                    onEnd()
                })
        }
        .frame(height: 44)
        .animation(.snappy(duration: 0.15), value: dragging)
        .accessibilityElement()
        .accessibilityLabel(label)
        .accessibilityValue(((value - range.lowerBound) / (range.upperBound - range.lowerBound)).formatted(.percent.precision(.fractionLength(0))))
        .accessibilityAdjustableAction { direction in
            let step = (range.upperBound - range.lowerBound) / 20
            value = min(max(value + (direction == .increment ? step : -step), range.lowerBound), range.upperBound)
            onEnd()
        }
    }
}
