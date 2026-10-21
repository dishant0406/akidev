import AkiManga
import SwiftUI

enum UpdatesTab: Hashable { case unread, recent }

/// New chapters across your library, as a list of cards.
struct UpdatesView: View {
    @Environment(AppModel.self) private var model
    @State private var tab = UpdatesTab.unread

    private struct Row: Identifiable {
        let entry: LibraryEntry
        let latest: ChapterEntry?
        var id: String { entry.key }
    }

    private var rows: [Row] {
        let all = model.library.map { entry -> Row in
            let chapters = (try? model.engine.cachedChapters(source: entry.source, series: entry.id)) ?? []
            return Row(entry: entry, latest: chapters.first)
        }
        let shown = tab == .unread ? all.filter { $0.entry.unread > 0 } : all
        return shown.sorted { ($0.latest?.dateMs ?? 0) > ($1.latest?.dateMs ?? 0) }
    }

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    AkiHeader(title: "Updates", subtitle: model.unreadTotal > 0 ? "\(model.unreadTotal) unread chapters" : "All caught up") {
                        if model.refreshing { AkiSpinner().frame(width: 44, height: 44) } else {
                            AkiIconButton(symbol: "refresh", label: "Check for new chapters") { Task { await refresh() } }
                        }
                    }
                    AkiTabs(items: [(UpdatesTab.unread, "Unread"), (UpdatesTab.recent, "Recent")], selection: $tab)
                    if !model.updateFailures.isEmpty { failures }
                    let list = rows
                    if list.isEmpty {
                        AkiEmptyState(title: tab == .unread ? "All caught up" : "No series saved", message: tab == .unread ? "New chapters of the series you saved appear here." : "Save a series and its chapters show up here.", action: tab == .unread ? ("Check for new chapters", { Task { await refresh() } }) : nil)
                            .frame(minHeight: 340)
                    } else {
                        LazyVStack(spacing: 12) {
                            ForEach(list) { row in
                                AkiLink(value: SeriesRef(source: row.entry.source, id: row.entry.id, title: row.entry.title, cover: row.entry.cover)) {
                                    AkiListCard(thumbnail: row.entry.cover,
                                                label: row.entry.unread > 0 ? "\(row.entry.unread) new chapter\(row.entry.unread == 1 ? "" : "s")" : "Up to date",
                                                title: row.entry.title,
                                                badges: [row.latest.map { chapterName(number: $0.number, title: $0.title) }, shortDate(row.latest?.dateMs)].compactMap { $0 },
                                                dimmed: row.entry.unread == 0)
                                }
                            }
                        }
                    }
                }
                .padding(.horizontal, 20).padding(.top, 8)
            }
            .refreshable { await refresh() }
            .underTabBar()
            .background(AkiBackdrop())
            .akiNoNavigationBar()
            .akiDestination(for: SeriesRef.self) { SeriesView(ref: $0) }
        }
        .onAppear { model.refresh() }
    }

    private var failures: some View {
        AkiCard(padding: 16) {
            VStack(alignment: .leading, spacing: 8) {
                AkiText("Could not check \(model.updateFailures.count) series", .bodyStrong)
                ForEach(model.updateFailures) { failure in
                    AkiText("\(failure.title) (\(failure.source)): \(failure.message)", .caption, tone: .dim, lines: 2)
                }
            }
        }
    }

    private func refresh() async { await model.refreshLibrary(force: true) }
}
