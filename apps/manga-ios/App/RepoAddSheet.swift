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
            HStack(spacing: 12) {
                AkiLogo(size: 36)
                AkiText(Copy.Sources.addRepository, .title).accessibilityAddTraits(.isHeader)
            }
            if let error {
                AkiEmptyState(title: error.title, message: error.message, action: (Copy.Home.tryAgain, { Task { await load() } }))
            } else if let view {
                AkiText(view.info.name, .heading).accessibilityAddTraits(.isHeader)
                AkiText(view.info.signed ? Copy.Sources.repositoryPreviewSigned : Copy.Sources.repositoryPreviewNotSigned, .caption, tone: .muted)
                AkiCard(padding: 4) {
                    VStack(spacing: 0) {
                        ForEach(view.sources, id: \.id) { source in
                            AkiRow(title: source.name, subtitle: source.nsfw ? Copy.Sources.repositoryLanguagesAdult(source.languages.joined(separator: ", ").uppercased()) : source.languages.joined(separator: ", ").uppercased())
                        }
                    }
                }
                AkiText(Copy.Sources.repositoryDisclaimer, .caption, tone: .muted)
                AkiButton(Copy.Sources.addRepositoryButton, variant: .primary, size: .large, fullWidth: true) { Task { await add(view) } }
            } else {
                AkiSpinner().frame(maxWidth: .infinity, minHeight: 160)
            }
            AkiButton(Copy.Sources.cancel, variant: .ghost, fullWidth: true) { dismiss() }
        }
        .padding(24)
        }
        .task { await load() }
    }

    private func load() async {
        error = nil
        do { view = try await model.engine.previewRepo(link: pending.address) } catch { self.error = describe(error, Copy.Sources.couldNotReadRepository) }
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
        } catch { self.error = describe(error, Copy.Sources.couldNotAddRepository) }
    }
}
