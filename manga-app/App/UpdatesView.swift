import AkiManga
import SwiftUI

enum UpdatesTab: Hashable { case unread, recent }

/// New chapters across your library, as a list of cards.
struct UpdatesView: View {
    @Environment(AppModel.self) private var model
    @State private var tab = UpdatesTab.unread
    @Environment(\.mangaPart) private var part

    private var rows: [UpdateEntry] {
        tab == .unread ? model.updates.filter { $0.unread > 0 } : model.updates
    }

    var body: some View {
        MangaStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    AkiHeader(title: "Updates", subtitle: model.unreadTotal > 0 ? "\(model.unreadTotal) unread chapters" : "All caught up") {
                        if model.refreshing { AkiSpinner().frame(width: 44, height: 44) } else {
                            AkiIconButton(symbol: "refresh", label: "Check for new chapters") { Task { await refresh() } }
                        }
                    }
                    MangaPartPills()
                    if part == nil { AkiTabs(items: [(UpdatesTab.unread, "Unread"), (UpdatesTab.recent, "Recent")], selection: $tab) }
                    if !model.updateFailures.isEmpty { failures }
                    let list = rows
                    if list.isEmpty {
                        AkiEmptyState(title: tab == .unread ? "All caught up" : "No series saved", message: tab == .unread ? "New chapters of the series you saved appear here." : "Save a series and its chapters show up here.", action: tab == .unread ? ("Check for new chapters", { Task { await refresh() } }) : nil)
                            .frame(minHeight: 340)
                    } else {
                        LazyVStack(spacing: 12) {
                            ForEach(list, id: \.key) { row in
                                AkiLink(value: SeriesRef(source: row.source, id: row.id, title: row.title, cover: row.cover)) {
                                    AkiListCard(thumbnail: row.cover,
                                                label: row.unread > 0 ? "\(row.unread) new chapter\(row.unread == 1 ? "" : "s")" : "Up to date",
                                                title: row.title,
                                                badges: [row.latestNumber == nil && row.latestTitle == nil ? nil : chapterName(number: row.latestNumber, title: row.latestTitle), shortDate(row.latestDateMs)].compactMap { $0 },
                                                dimmed: row.unread == 0)
                                }
                            }
                        }
                    }
                }
                .padding(.horizontal, 20).padding(.top, 8)
            }
            .akiRefreshable { await refresh() }
            .underTabBar()
            .background(AkiBackdrop())
            .akiNoNavigationBar()
            .akiDestination(for: SeriesRef.self) { SeriesView(ref: $0) }
        }
        .onChange(of: part?.wrappedValue, initial: true) { _, now in if now == .recent { tab = .recent } else if now == .unread { tab = .unread } }
        .onAppear { model.refresh() }
    }

    private var failures: some View {
        AkiCard(padding: 16) {
            VStack(alignment: .leading, spacing: 8) {
                AkiText("Could not check \(model.updateFailures.count) series", .bodyStrong)
                ForEach(model.updateFailures) { failure in
                    AkiText("\(failure.title) (\(failure.source)): \(failure.message)", .caption, tone: .muted, lines: 2)
                }
            }
        }
    }

    private func refresh() async { await model.refreshLibrary(force: true) }
}
