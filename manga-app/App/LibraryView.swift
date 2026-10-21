import AkiManga
import SwiftUI

enum LibraryTab: Hashable { case saved, history }

extension LibraryEntry {
    var key: String { "\(source)/\(id)" }
}

/// What you saved, and what you read last.
struct LibraryView: View {
    @Environment(AppModel.self) private var model
    @State private var tab = LibraryTab.saved
    @State private var history: [HistoryEntry] = []
    private let columns = [GridItem(.flexible(), spacing: 14, alignment: .top), GridItem(.flexible(), spacing: 14, alignment: .top)]

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    ScreenHeader(title: "Library", subtitle: "\(model.library.count) saved") { EmptyView() }
                    PillTabs(items: [(LibraryTab.saved, "Saved"), (LibraryTab.history, "History")], selection: $tab)
                    switch tab {
                    case .saved: saved
                    case .history: historyList
                    }
                }
                .padding(.horizontal, 20).padding(.top, 8)
            }
            .underTabBar()
            .background(AmbientBackground())
            .toolbar(.hidden, for: .navigationBar)
            .navigationDestination(for: SeriesRef.self) { SeriesView(ref: $0) }
        }
        .onAppear { model.refresh(); history = (try? model.engine.history(limit: 60)) ?? [] }
    }

    @ViewBuilder private var saved: some View {
        if model.library.isEmpty {
            StateCard(title: "Nothing saved yet", message: "Save a series with the bookmark on its page and it stays here, with how far you have read.").frame(minHeight: 340)
        } else {
            LazyVGrid(columns: columns, spacing: 22) {
                ForEach(model.library, id: \.key) { item in
                    NavigationLink(value: SeriesRef(source: item.source, id: item.id, title: item.title, cover: item.cover)) {
                        CoverCard(title: item.title, cover: item.cover, subtitle: item.unread > 0 ? "\(item.unread) unread" : "Up to date", badge: Int(item.unread))
                    }.buttonStyle(.plain)
                }
            }
        }
    }

    @ViewBuilder private var historyList: some View {
        if history.isEmpty {
            StateCard(title: "Nothing read yet", message: "The chapters you open show up here, newest first.").frame(minHeight: 340)
        } else {
            LazyVStack(spacing: 12) {
                ForEach(Array(history.enumerated()), id: \.offset) { _, item in
                    NavigationLink(value: SeriesRef(source: item.source, id: item.series, title: item.title, cover: item.cover)) {
                        InboxRow(thumbnail: item.cover, label: item.finished ? "Finished" : "Reading", title: item.title,
                                 chips: [chapterName(number: item.chapterNumber, title: item.chapterTitle), shortDate(item.atMs)].compactMap { $0 })
                    }.buttonStyle(.plain)
                }
            }
        }
    }
}

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
                    ScreenHeader(title: "Updates", subtitle: model.unreadTotal > 0 ? "\(model.unreadTotal) unread chapters" : "All caught up") {
                        if refreshing { ProgressView().frame(width: 44, height: 44) } else {
                            GlassIconButton(symbol: "arrow.clockwise") { Task { await refresh() } }
                        }
                    }
                    PillTabs(items: [(UpdatesTab.unread, "Unread"), (UpdatesTab.recent, "Recent")], selection: $tab)
                    let list = rows
                    if list.isEmpty {
                        StateCard(title: tab == .unread ? "All caught up" : "No series saved", message: tab == .unread ? "New chapters of the series you saved appear here." : "Save a series and its chapters show up here.", action: tab == .unread ? ("Check for new chapters", { Task { await refresh() } }) : nil)
                            .frame(minHeight: 340)
                    } else {
                        LazyVStack(spacing: 12) {
                            ForEach(list) { row in
                                NavigationLink(value: SeriesRef(source: row.entry.source, id: row.entry.id, title: row.entry.title, cover: row.entry.cover)) {
                                    InboxRow(thumbnail: row.entry.cover,
                                             label: row.entry.unread > 0 ? "\(row.entry.unread) new chapter\(row.entry.unread == 1 ? "" : "s")" : "Up to date",
                                             title: row.entry.title,
                                             chips: [row.latest.map { chapterName(number: $0.number, title: $0.title) }, shortDate(row.latest?.dateMs)].compactMap { $0 },
                                             dimmed: row.entry.unread == 0)
                                }.buttonStyle(.plain)
                            }
                        }
                    }
                }
                .padding(.horizontal, 20).padding(.top, 8)
            }
            .refreshable { await refresh() }
            .underTabBar()
            .background(AmbientBackground())
            .toolbar(.hidden, for: .navigationBar)
            .navigationDestination(for: SeriesRef.self) { SeriesView(ref: $0) }
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
