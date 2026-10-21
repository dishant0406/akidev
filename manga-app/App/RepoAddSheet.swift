import AkiManga
import SwiftUI

/// "Add this repository?": shows what an address offers before anything is kept. A repository is code that
/// decides which sites the app contacts, so the person sees its name, whether it is signed and what it lists.
struct RepoAddSheet: View {
    let pending: PendingRepo
    @Environment(AppModel.self) private var model
    @Environment(\.dismiss) private var dismiss
    @State private var view: RepoView?
    @State private var error: (title: String, message: String)?
    @State private var adding = false

    var body: some View {
        ScrollView {
        VStack(alignment: .leading, spacing: 16) {
            AkiText("Add a repository", .title)
            if let error {
                AkiEmptyState(title: error.title, message: error.message, action: ("Try again", { Task { await load() } }))
            } else if let view {
                AkiText(view.info.name, .heading)
                AkiText(view.info.signed ? "Signed, so the app notices if it ever changes hands." : "Not signed: you are trusting this address.", .caption, tone: .muted)
                AkiCard(padding: 4) {
                    VStack(spacing: 0) {
                        ForEach(view.sources, id: \.id) { source in
                            AkiRow(title: source.name, subtitle: "\(source.languages.joined(separator: ", ").uppercased())\(source.nsfw ? " · 18+" : "")")
                        }
                    }
                }
                AkiText("Aki Manga does not include, offer or endorse any repository. Only add one you trust.", .caption, tone: .muted)
                AkiButton("Add repository", variant: .primary, size: .large, fullWidth: true) { Task { await add(view) } }
            } else {
                AkiSpinner().frame(maxWidth: .infinity, minHeight: 160)
            }
            AkiButton("Cancel", variant: .ghost, fullWidth: true) { dismiss() }
        }
        .padding(24)
        }
        .task { await load() }
    }

    private func load() async {
        error = nil
        do { view = try await model.engine.previewRepo(link: pending.address) } catch { self.error = describe(error) }
    }

    private func add(_ view: RepoView) async {
        guard !adding else { return }
        adding = true
        defer { adding = false }
        do {
            _ = try await model.engine.addRepo(link: pending.address)
            model.refresh()
            model.tab = .sources
            dismiss()
        } catch { self.error = describe(error) }
    }
}
