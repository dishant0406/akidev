import AkiManga
import SwiftUI

/// What can be changed about how the book looks: theme, face, size, spacing, margins. Every change shows on the page at once.
struct BookSettingsSheet: View {
    let model: BookModel

    var body: some View {
        @Bindable var model = model
        ScrollView {
            VStack(alignment: .leading, spacing: 22) {
                AkiText("Type and theme", .title)
                BookThemePicker(selection: $model.style.theme)
                AkiCard(padding: 16) {
                    VStack(spacing: 18) {
                        AkiSelect(title: "Font", selection: $model.style.font, options: BookFont.allCases.map { ($0, $0.title) })
                        stepper("Text size", value: $model.style.size, range: BookStyle.sizes, small: "A", large: "A")
                        stepper("Line spacing", value: $model.style.lineSpacing, range: BookStyle.spacings, small: "−", large: "+")
                    }
                }
                VStack(alignment: .leading, spacing: 10) {
                    AkiText("Margins", .captionStrong, tone: .muted)
                    AkiTabs(items: BookMargin.allCases.map { ($0, $0.title) }, selection: $model.style.margin)
                }
                AkiCard(padding: 16) {
                    VStack(spacing: 16) {
                        AkiSwitch(title: "Justify text", isOn: $model.style.justify)
                        AkiSwitch(title: "Hyphenate long words", subtitle: "Fewer gaps in justified lines.", isOn: $model.style.hyphenate)
                    }
                }
            }
            .padding(24)
        }
    }

    private func stepper(_ title: String, value: Binding<Double>, range: ClosedRange<Double>, small: String, large: String) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            AkiText(title, .bodyStrong)
            HStack(spacing: 12) {
                AkiText(small, .caption, tone: .muted)
                AkiSlider(value: value, range: range)
                AkiText(large, .heading, tone: .muted)
            }
        }
    }
}

/// The book's table of contents: tap an entry to go there. The part being read is yellow.
struct BookContentsSheet: View {
    let model: BookModel
    let close: () -> Void

    var body: some View {
        ScrollViewReader { proxy in
            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    VStack(alignment: .leading, spacing: 4) {
                        AkiText("Contents", .title)
                        AkiText(model.info.title, .caption, tone: .muted, lines: 1)
                    }
                    LazyVStack(spacing: 10) {
                        ForEach(Array(model.contents.enumerated()), id: \.offset) { index, entry in
                            row(entry, current: index == currentIndex).id(index)
                        }
                    }
                }
                .padding(.horizontal, 20).padding(.top, 24).padding(.bottom, 32)
            }
            .onAppear { DispatchQueue.main.async { proxy.scrollTo(currentIndex, anchor: .center) } }
        }
    }

    /// The last entry that begins at or before the chapter being read.
    private var currentIndex: Int {
        let chapter = model.place?.chapter ?? 0
        return model.contents.lastIndex { Int($0.chapter) <= chapter } ?? 0
    }

    private func row(_ entry: BookEntry, current: Bool) -> some View {
        AkiTap(action: { model.open(entry); close() }) {
            HStack {
                AkiText(entry.title, entry.depth == 0 ? .bodyStrong : .body, tone: current ? .onPrimary : .text, lines: 2)
                Spacer(minLength: 8)
            }
            .padding(.leading, 18 + CGFloat(min(entry.depth, 3)) * 16).padding(.trailing, 18).padding(.vertical, 14)
            .background(current ? Aki.primary : Aki.surface, in: RoundedRectangle(cornerRadius: 22, style: .continuous))
            .contentShape(RoundedRectangle(cornerRadius: 22, style: .continuous))
        }
    }
}
