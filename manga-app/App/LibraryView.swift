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
                    AkiHeader(title: "Library", subtitle: "\(model.library.count) saved")
                    AkiTabs(items: [(LibraryTab.saved, "Saved"), (LibraryTab.history, "History")], selection: $tab)
                    switch tab {
                    case .saved: saved
                    case .history: historyList
                    }
                }
                .padding(.horizontal, 20).padding(.top, 8)
            }
            .underTabBar()
            .background(AkiBackdrop())
            .akiNoNavigationBar()
            .akiDestination(for: SeriesRef.self) { SeriesView(ref: $0) }
        }
        .onAppear { model.refresh(); history = (try? model.engine.history(limit: 60)) ?? [] }
    }

    @ViewBuilder private var saved: some View {
        if model.library.isEmpty {
            AkiEmptyState(title: "Nothing saved yet", message: "Save a series with the bookmark on its page and it stays here, with how far you have read.").frame(minHeight: 340)
        } else {
            LazyVGrid(columns: columns, spacing: 22) {
                ForEach(model.library, id: \.key) { item in
                    AkiLink(value: SeriesRef(source: item.source, id: item.id, title: item.title, cover: item.cover)) {
                        AkiCoverCard(title: item.title, cover: item.cover, subtitle: item.unread > 0 ? "\(item.unread) unread" : "Up to date", badge: Int(item.unread))
                    }
                }
            }
        }
    }

    @ViewBuilder private var historyList: some View {
        if history.isEmpty {
            AkiEmptyState(title: "Nothing read yet", message: "The chapters you open show up here, newest first.").frame(minHeight: 340)
        } else {
            LazyVStack(spacing: 12) {
                ForEach(Array(history.enumerated()), id: \.offset) { _, item in
                    AkiLink(value: SeriesRef(source: item.source, id: item.series, title: item.title, cover: item.cover)) {
                        AkiListCard(thumbnail: item.cover, label: item.finished ? "Finished" : "Reading", title: item.title,
                                    badges: [chapterName(number: item.chapterNumber, title: item.chapterTitle), shortDate(item.atMs)].compactMap { $0 })
                    }
                }
            }
        }
    }
}
