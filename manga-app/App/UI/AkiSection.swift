import SwiftUI

/// A heading and what belongs under it.
struct AkiSection<Content: View>: View {
    let title: String
    var footer: String?
    @ViewBuilder var content: () -> Content

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            AkiText(title, .heading)
            content()
            if let footer { AkiText(footer, .caption, tone: .dim) }
        }
    }
}
