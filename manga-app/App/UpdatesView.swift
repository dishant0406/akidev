import AkiManga
import SwiftUI

enum UpdatesTab: Hashable { case unread, recent }

/// New chapters across your library, as a list of cards.
struct UpdatesView: View {
    @Environment(AppModel.self) private var model
    @State private var tab = UpdatesTab.unread
    @State private var refreshing = false

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
                        if refreshing { AkiSpinner().frame(width: 44, height: 44) } else {
                            AkiIconButton(symbol: "arrow.clockwise", label: "Check for new chapters") { Task { await refresh() } }
                        }
                    }
                    AkiTabs(items: [(UpdatesTab.unread, "Unread"), (UpdatesTab.recent, "Recent")], selection: $tab)
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

    /// Asks every source for the chapters of the series you saved.
    private func refresh() async {
        refreshing = true
        defer { refreshing = false }
        let entries = model.library
        await withTaskGroup(of: Void.self) { group in
            for entry in entries {
                group.addTask { _ = try? await model.engine.chapters(source: entry.source, id: entry.id) }
            }
        }
        model.refresh()
    }
}
