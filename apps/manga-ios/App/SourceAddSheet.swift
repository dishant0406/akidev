import AkiManga
import SwiftUI

/// "Add a source?": a definition pasted in by hand, shown before anything is kept. Most sources come from a
/// repository; this is for one written on its own, which usually extends a theme.
struct SourceAddSheet: View {
    @Environment(AppModel.self) private var model
    @Environment(\.dismiss) private var dismiss
    @State private var text = ""
    @State private var preview: SourceInfo?
    @State private var error: (title: String, message: String)?
    @State private var checking = false
    @State private var installing = false

    private var json: String { text.trimmingCharacters(in: .whitespacesAndNewlines) }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                HStack(spacing: 12) {
                    AkiLogo(size: 36)
                    AkiText(Copy.Sources.addSource, .title).accessibilityAddTraits(.isHeader)
                }
                AkiText(Copy.Sources.pasteSourceHint, .caption, tone: .muted)
                AkiPasteButton { pasted in text = pasted }

                AkiTextArea(text: $text, label: Copy.Sources.sourceText)
                    .onChange(of: text) { preview = nil; error = nil }

                if let error {
                    AkiEmptyState(title: error.title, message: error.message, action: (Copy.Home.tryAgain, { Task { await check() } }))
                } else if let preview {
                    AkiCard(padding: 16) {
                        VStack(alignment: .leading, spacing: 6) {
                            AkiText(preview.name, .heading)
                            AkiText(preview.nsfw ? Copy.Sources.sourceLineAdult(preview.languages.joined(separator: ", ").uppercased(), "\(preview.version)") : Copy.Sources.sourceLine(preview.languages.joined(separator: ", ").uppercased(), "\(preview.version)"), .caption, tone: .muted, digits: true)
                        }
                    }
                    if let origin = model.engine.installedOrigin(id: preview.id) {
                        AkiText(Copy.Sources.replacesSource(preview.name, "\(origin)"), .caption, tone: .muted)
                    }
                    AkiButton(installing ? Copy.Sources.installing : Copy.Sources.installSource, variant: .primary, size: .large, fullWidth: true) { Task { await install() } }
                        .disabled(installing)
                } else {
                    AkiButton(checking ? Copy.Sources.checking : Copy.Sources.checkSource, variant: .glass, size: .large, fullWidth: true) { Task { await check() } }
                        .disabled(checking || json.isEmpty)
                }
                AkiText(Copy.Sources.sourceDisclaimer, .caption, tone: .muted)
                AkiButton(Copy.Sources.cancel, variant: .ghost, fullWidth: true) { dismiss() }
            }
            .padding(24)
        }
    }

    private func check() async {
        guard !checking, !json.isEmpty else { return }
        checking = true
        defer { checking = false }
        error = nil
        do {
            let checked = try await model.engine.previewSource(json: json)
            if checked.nsfw {
                self.error = (Copy.Sources.couldNotAddSource, Copy.Settings.contentFooterNoSwitch)
            } else {
                preview = checked
            }
        } catch { self.error = describe(error, Copy.Sources.couldNotReadSource) }
    }

    private func install() async {
        guard !installing, !json.isEmpty else { return }
        installing = true
        defer { installing = false }
        do {
            _ = try await model.engine.installSource(json: json)
            model.refresh()
            model.tab = .sources
            dismiss()
        } catch { self.error = describe(error, Copy.Sources.couldNotAddSource) }
    }
}
