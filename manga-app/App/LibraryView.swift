import AkiManga
import SwiftUI

enum LibraryTab: Hashable { case saved, history }

enum LibrarySort: String, CaseIterable {
    case recent = "Last read", added = "Date added", title = "Title", unread = "Unread"
}

extension LibraryEntry {
    var key: String { "\(source)/\(id)" }
}

/// What you saved, and what you read last.
struct LibraryView: View {
    @Environment(AppModel.self) private var model
    @State private var tab = LibraryTab.saved
    @State private var history: [HistoryEntry] = []
    @State private var categories: [CategoryInfo] = []
    /// -1 is every category.
    @State private var category: Int64 = -1
    @State private var sort = LibrarySort.recent
    @State private var onlyUnread = false
    @State private var naming = false
    @State private var newName = ""
    private let columns = [GridItem(.flexible(), spacing: 14, alignment: .top), GridItem(.flexible(), spacing: 14, alignment: .top)]

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    AkiHeader(title: "Library", subtitle: "\(model.library.count) saved") {
                        AkiMenu(symbol: "line.3.horizontal.decrease", label: "Sort and filter", items:
                            LibrarySort.allCases.map { option in AkiMenuItem(title: option.rawValue, icon: "arrow.up.arrow.down", isSelected: option == sort) { sort = option } }
                            + [AkiMenuItem(title: "Only unread", icon: "circle.fill", isSelected: onlyUnread) { onlyUnread.toggle() },
                               AkiMenuItem(title: "New category", icon: "folder.badge.plus") { naming = true }]
                            + categories.map { cat in AkiMenuItem(title: "Delete “\(cat.name)”", icon: "trash", destructive: true) { deleteCategory(cat) } })
                    }
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
        .onAppear { model.refresh(); history = (try? model.engine.history(limit: 60)) ?? []; loadCategories() }
        .akiPrompt("New category", message: "A name for a shelf in your library.", placeholder: "Reading", confirm: "Add", text: $newName, isPresented: $naming) {
            _ = try? model.engine.addCategory(name: newName); newName = ""; loadCategories()
        }
    }

    private var shown: [LibraryEntry] {
        let filtered = model.library.filter { (category < 0 || $0.categories.contains(category)) && (!onlyUnread || $0.unread > 0) }
        func byTitle(_ a: LibraryEntry, _ b: LibraryEntry) -> Bool { a.title.localizedCaseInsensitiveCompare(b.title) == .orderedAscending }
        // Ties fall back to the title so the order never reshuffles on its own.
        switch sort {
        case .recent: return filtered.sorted { ($0.lastReadMs ?? 0) != ($1.lastReadMs ?? 0) ? ($0.lastReadMs ?? 0) > ($1.lastReadMs ?? 0) : byTitle($0, $1) }
        case .added: return filtered.sorted { $0.savedMs != $1.savedMs ? $0.savedMs > $1.savedMs : byTitle($0, $1) }
        case .title: return filtered.sorted(by: byTitle)
        case .unread: return filtered.sorted { $0.unread != $1.unread ? $0.unread > $1.unread : byTitle($0, $1) }
        }
    }

    @ViewBuilder private var saved: some View {
        if model.library.isEmpty {
            AkiEmptyState(title: "Nothing saved yet", message: "Save a series with the bookmark on its page and it stays here, with how far you have read.").frame(minHeight: 340)
        } else {
            if !categories.isEmpty {
                AkiChips(items: [(Int64(-1), "All")] + categories.map { ($0.id, "\($0.name) \($0.count)") }, selection: $category)
            }
            if shown.isEmpty {
                AkiEmptyState(title: "Nothing here", message: onlyUnread ? "Everything in this shelf is read." : "No series on this shelf yet. Long-press a series to put it on one.").frame(minHeight: 260)
            }
            LazyVGrid(columns: columns, spacing: 22) {
                ForEach(shown, id: \.key) { item in
                    AkiLink(value: SeriesRef(source: item.source, id: item.id, title: item.title, cover: item.cover)) {
                        AkiCoverCard(title: item.title, cover: item.cover, subtitle: item.unread > 0 ? "\(item.unread) unread" : "Up to date", badge: Int(item.unread))
                    }
                    .akiContextMenu(menu(for: item))
                }
            }
        }
    }

    private func menu(for item: LibraryEntry) -> [AkiMenuItem] {
        categories.map { cat in
            AkiMenuItem(title: cat.name, icon: "folder", isSelected: item.categories.contains(cat.id)) {
                try? model.engine.setCategory(source: item.source, series: item.id, category: cat.id, member: !item.categories.contains(cat.id))
                model.refresh(); loadCategories()
            }
        } + [
            AkiMenuItem(title: "Mark all as read", icon: "checkmark.circle") {
                let ids = ((try? model.engine.cachedChapters(source: item.source, series: item.id)) ?? []).map(\.id)
                try? model.engine.mark(source: item.source, series: item.id, chapters: ids, read: true); model.refresh()
            },
            AkiMenuItem(title: "Remove from library", icon: "bookmark.slash", destructive: true) {
                Task { try? await model.engine.setInLibrary(source: item.source, id: item.id, saved: false); model.refresh(); loadCategories() }
            },
        ]
    }

    private func loadCategories() { categories = (try? model.engine.categories()) ?? [] }

    private func deleteCategory(_ cat: CategoryInfo) {
        try? model.engine.deleteCategory(id: cat.id)
        if category == cat.id { category = -1 }
        loadCategories(); model.refresh()
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
