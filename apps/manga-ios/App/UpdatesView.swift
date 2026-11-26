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
                    AkiHeader(title: Copy.Home.updates, subtitle: model.unreadTotal == 0 ? Copy.Home.allCaughtUp : Copy.Home.unreadChapters(model.unreadTotal)) {
                        if model.refreshing { AkiSpinner().frame(width: 44, height: 44).accessibilityLabel(Copy.Home.checkingForNewChapters) } else {
                            AkiIconButton(symbol: "refresh", label: Copy.Home.checkForNewChapters) { Task { await refresh() } }
                        }
                    }
                    MangaPartPills()
                    if part == nil { AkiTabs(items: [(UpdatesTab.unread, Copy.Home.unread), (UpdatesTab.recent, Copy.Home.recent)], selection: $tab) }
                    if !model.updateFailures.isEmpty { failures }
                    let list = rows
                    if list.isEmpty {
                        AkiEmptyState(title: tab == .unread ? Copy.Home.allCaughtUp : Copy.Home.noSeriesInLibrary, message: tab == .unread ? Copy.Home.newChaptersAppearHere : Copy.Home.addSeriesToLibrary, action: tab == .unread ? (Copy.Home.checkForNewChapters, { Task { await refresh() } }) : nil)
                            .frame(minHeight: 340)
                    } else {
                        LazyVStack(spacing: 12) {
                            ForEach(list, id: \.key) { row in
                                AkiLink(value: SeriesRef(source: row.source, id: row.id, title: row.title, cover: row.cover)) {
                                    AkiListCard(thumbnail: row.cover,
                                                label: row.unread == 0 ? Copy.Home.upToDate : Copy.Home.newChapterCount(Int(row.unread)),
                                                title: row.title,
                                                badges: [row.latestNumber == nil && row.latestTitle == nil ? nil : chapterName(number: row.latestNumber, title: row.latestTitle), shortDate(row.latestDateMs)].compactMap { $0 },
                                                dimmed: row.unread == 0)
                                }
                            }
                        }
                    }
                }
                .akiPage()
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
                AkiText(Copy.Home.couldNotCheck(model.updateFailures.count), .bodyStrong, digits: true)
                ForEach(model.updateFailures) { failure in
                    AkiText(Copy.Home.updateFailure(failure.title, failure.source, failure.message), .caption, tone: .muted, lines: 2)
                }
            }
        }
        .onAppear { akiAnnounce(Copy.Home.couldNotCheck(model.updateFailures.count)) }
    }

    private func refresh() async { await model.refreshLibrary(force: true) }
}
