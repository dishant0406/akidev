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
                    AkiText("Add a source", .title).accessibilityAddTraits(.isHeader)
                }
                AkiText("Paste the text of a source. It is checked before anything is kept.", .caption, tone: .muted)
                AkiPasteButton { pasted in text = pasted }

                AkiTextArea(text: $text, label: "Source text")
                    .onChange(of: text) { preview = nil; error = nil }

                if let error {
                    AkiEmptyState(title: error.title, message: error.message, action: ("Try again", { Task { await check() } }))
                } else if let preview {
                    AkiCard(padding: 16) {
                        VStack(alignment: .leading, spacing: 6) {
                            AkiText(preview.name, .heading)
                            AkiText("\(preview.languages.joined(separator: ", ").uppercased()) · v\(preview.version)\(preview.nsfw ? " · 18+" : "")", .caption, tone: .muted, digits: true)
                        }
                    }
                    if let origin = model.engine.installedOrigin(id: preview.id) {
                        AkiText("This replaces the \(preview.name) installed from \(origin). A pasted source does not update itself, and the repository no longer updates it.", .caption, tone: .muted)
                    }
                    AkiButton(installing ? "Installing…" : "Install source", variant: .primary, size: .large, fullWidth: true) { Task { await install() } }
                        .disabled(installing)
                } else {
                    AkiButton(checking ? "Checking…" : "Check source", variant: .glass, size: .large, fullWidth: true) { Task { await check() } }
                        .disabled(checking || json.isEmpty)
                }
                AkiText("Akidev does not include, offer or endorse any source. Only add one you trust: it decides which sites the app contacts.", .caption, tone: .muted)
                AkiButton("Cancel", variant: .ghost, fullWidth: true) { dismiss() }
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
                self.error = ("Could not add this source", "Sources marked 18+ are not available in this app.")
            } else {
                preview = checked
            }
        } catch { self.error = describe(error, "Could not read this source") }
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
        } catch { self.error = describe(error, "Could not add this source") }
    }
}
