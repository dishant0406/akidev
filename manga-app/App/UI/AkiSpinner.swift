import SwiftUI

struct AkiSpinner: View {
    var body: some View { ProgressView().tint(Aki.dim) }
}

/// A bar that fills: a download.
struct AkiProgressBar: View {
    let fraction: Double

    var body: some View {
        GeometryReader { geo in
            ZStack(alignment: .leading) {
                Capsule().fill(Aki.chip)
                Capsule().fill(Aki.yellow).frame(width: geo.size.width * min(max(fraction, 0), 1))
            }
        }
        .frame(height: 6)
    }
}
