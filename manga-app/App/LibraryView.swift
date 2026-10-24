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
    @State private var categories: [CategoryInfo] = []
    /// -1 is every category.
    @State private var category: Int64 = -1
    /// Changes each time Random is chosen, so choosing it again shuffles again.
    @State private var shuffle = UInt64.random(in: 0...UInt64.max)
    @State private var onlyUnread = false
    @State private var managing = false
    @State private var selecting = false
    @State private var picked: Set<String> = []
    @State private var assigning = false
    /// The series the category picker is for, fixed when it opens (the picked ones are cleared as it closes).
    @State private var assignees: [CategoryPicker.Series] = []
    @State private var removing = false
    private let columns = [GridItem(.flexible(), spacing: 14, alignment: .top), GridItem(.flexible(), spacing: 14, alignment: .top)]

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    AkiHeader(title: "Library", subtitle: selecting ? "\(pickedShown.count) selected" : "\(model.library.count) saved") {
                        AkiMenu(symbol: "filter", label: "Sort and filter", items:
                            LibrarySort.allCases.map { option in AkiMenuItem(title: option.rawValue, icon: "order", isSelected: option == model.prefs.librarySort) { choose(option) } }
                            + [AkiMenuItem(title: "Reverse order", icon: "swap", isSelected: model.prefs.libraryReversed) { model.prefs.libraryReversed.toggle() },
                               AkiMenuItem(title: "Only unread", icon: "check", isSelected: onlyUnread) { onlyUnread.toggle() },
                               AkiMenuItem(title: selecting ? "Done selecting" : "Select", icon: "check") { selecting.toggle(); picked = [] },
                               AkiMenuItem(title: "Categories", icon: "folder") { managing = true }])
                    }
                    AkiTabs(items: [(LibraryTab.saved, "Saved"), (LibraryTab.history, "History")], selection: $tab)
                    switch tab {
                    case .saved: saved
                    case .history: historyList
                    }
                }
                .padding(.horizontal, 20).padding(.top, 8)
            }
            .akiRefreshable {
                await model.refreshLibrary(force: true)
                history = (try? model.engine.history(limit: 60)) ?? []
                loadCategories()
            }
            .underTabBar()
            .overlay(alignment: .bottom) { if tab == .saved && selecting && !pickedShown.isEmpty { selectionBar } }
            .background(AkiBackdrop())
            .akiNoNavigationBar()
            .akiDestination(for: SeriesRef.self) { SeriesView(ref: $0) }
        }
        .onAppear { model.refresh(); history = (try? model.engine.history(limit: 60)) ?? []; loadCategories() }
        // The counts on the shelves are read from the same place as the library: they change when it does, wherever the change came from.
        .onChange(of: model.library) { loadCategories() }
        .akiSheet(isPresented: $managing, content: { CategoriesSheet().onDisappear { loadCategories(); model.refresh() } })
        .akiSheet(isPresented: $assigning, content: { CategoryPicker(series: assignees).onDisappear { loadCategories(); picked = []; selecting = false } })
        .akiConfirm("Remove \(pickedShown.count) from your library?", message: "Their read state stays; you can save them again.", confirm: "Remove", isPresented: $removing) { removePicked() }
    }

    private var shown: [LibraryEntry] {
        let filtered = model.library.filter { onShelf($0) && (!onlyUnread || $0.unread > 0) }
        let ordered = sorted(filtered)
        return model.prefs.libraryReversed ? ordered.reversed() : ordered
    }

    /// -1 is every shelf, -2 is the series on none.
    private func onShelf(_ item: LibraryEntry) -> Bool {
        switch category {
        case -1: true
        case -2: item.categories.isEmpty
        default: item.categories.contains(category)
        }
    }

    /// The series picked that are on screen: filtering after picking does not leave hidden ones to be changed unseen.
    private var pickedShown: [LibraryEntry] { shown.filter { picked.contains($0.key) } }

    /// Each order puts the biggest first (the most recent, the most unread), except the title, which is A to Z. Ties fall back
    /// to the title so the order never reshuffles on its own.
    private func sorted(_ items: [LibraryEntry]) -> [LibraryEntry] {
        func byTitle(_ a: LibraryEntry, _ b: LibraryEntry) -> Bool { a.title.localizedCaseInsensitiveCompare(b.title) == .orderedAscending }
        func biggestFirst(_ value: (LibraryEntry) -> Int64) -> [LibraryEntry] {
            items.sorted { value($0) != value($1) ? value($0) > value($1) : byTitle($0, $1) }
        }
        switch model.prefs.librarySort {
        case .recent: return biggestFirst { $0.lastReadMs ?? 0 }
        case .added: return biggestFirst { $0.savedMs }
        case .title: return items.sorted(by: byTitle)
        case .unread: return biggestFirst { Int64($0.unread) }
        case .chapters: return biggestFirst { Int64($0.total) }
        case .latest:
            let dates = Dictionary(model.updates.map { ($0.key, $0.latestDateMs ?? 0) }, uniquingKeysWith: { first, _ in first })
            return biggestFirst { dates[$0.key] ?? 0 }
        case .random: return items.sorted { "\($0.key)\(shuffle)".hashValue < "\($1.key)\(shuffle)".hashValue }
        }
    }

    /// Random shuffles anew each time it is chosen.
    private func choose(_ option: LibrarySort) {
        if option == .random { shuffle = UInt64.random(in: 0...UInt64.max) }
        model.prefs.librarySort = option
    }

    @ViewBuilder private var saved: some View {
        if model.library.isEmpty {
            AkiEmptyState(title: "Nothing saved yet", message: "Save a series with the bookmark on its page and it stays here, with how far you have read.").frame(minHeight: 340)
        } else {
            if !categories.isEmpty {
                AkiChips(items: [(Int64(-1), "All")] + categories.filter { $0.count > 0 || $0.books == 0 }.map { ($0.id, "\($0.name) \($0.count)") } + [(Int64(-2), "No category")], selection: $category)
            }
            if shown.isEmpty {
                AkiEmptyState(title: "Nothing here", message: onlyUnread ? "Everything in this shelf is read." : "No series on this shelf yet. Long-press a series to put it on one.").frame(minHeight: 260)
            }
            LazyVGrid(columns: columns, spacing: 22) {
                ForEach(shown, id: \.key) { item in
                    if selecting {
                        AkiTap(action: { if !picked.insert(item.key).inserted { picked.remove(item.key) } }) {
                            AkiCoverCard(title: item.title, cover: item.cover, subtitle: item.unread > 0 ? "\(item.unread) unread" : "Up to date", badge: Int(item.unread))
                                .overlay(alignment: .topLeading) { tick(picked.contains(item.key)) }
                        }
                    } else {
                        AkiLink(value: SeriesRef(source: item.source, id: item.id, title: item.title, cover: item.cover)) {
                            AkiCoverCard(title: item.title, cover: item.cover, subtitle: item.unread > 0 ? "\(item.unread) unread" : "Up to date", badge: Int(item.unread))
                        }
                        .akiContextMenu(menu(for: item))
                    }
                }
            }
        }
    }

    private func tick(_ on: Bool) -> some View {
        AkiIcon(on ? "check-solid" : "check", size: 22, tone: on ? .onPrimary : .onMedia)
            .padding(6)
            .background(on ? Aki.primary : Aki.scrim, in: Circle())
            .padding(8)
    }

    /// What can be done to the series picked: put them on categories, mark them read, take them out of the library. Icons
    /// alone, round, so three of them fit and sit clear of the tab bar.
    private var selectionBar: some View {
        AkiGlassGroup(spacing: 12) {
            HStack(spacing: 12) {
                AkiIconButton(symbol: "folder", label: "Categories") { assignees = pickedShown.map { CategoryPicker.Series(source: $0.source, id: $0.id) }; assigning = true }
                AkiIconButton(symbol: "check", label: "Mark as read") { markPickedRead() }
                AkiIconButton(symbol: "bookmark-off", label: "Remove from library", tint: Aki.destructive) { removing = true }
            }
        }
        .padding(.bottom, 112)
    }

    private func markPickedRead() {
        for item in pickedShown {
            let ids = ((try? model.engine.cachedChapters(source: item.source, series: item.id)) ?? []).map(\.id)
            try? model.engine.mark(source: item.source, series: item.id, chapters: ids, read: true)
        }
        model.refresh()
        picked = []
        selecting = false
    }

    private func removePicked() {
        let chosen = pickedShown
        Task {
            for item in chosen { try? await model.engine.setInLibrary(source: item.source, id: item.id, saved: false) }
            model.refresh()
            loadCategories()
            picked = []
            selecting = false
        }
    }

    private func menu(for item: LibraryEntry) -> [AkiMenuItem] {
        categories.map { cat in
            AkiMenuItem(title: cat.name, icon: "folder", isSelected: item.categories.contains(cat.id)) {
                try? model.engine.setCategory(source: item.source, series: item.id, category: cat.id, member: !item.categories.contains(cat.id))
                model.refresh(); loadCategories()
            }
        } + [
            AkiMenuItem(title: "Mark all as read", icon: "check") {
                let ids = ((try? model.engine.cachedChapters(source: item.source, series: item.id)) ?? []).map(\.id)
                try? model.engine.mark(source: item.source, series: item.id, chapters: ids, read: true); model.refresh()
            },
            AkiMenuItem(title: "Remove from library", icon: "bookmark-off", destructive: true) {
                Task { try? await model.engine.setInLibrary(source: item.source, id: item.id, saved: false); model.refresh(); loadCategories() }
            },
        ]
    }

    /// Reads the categories, and leaves a shelf that has gone (deleted in the manage sheet) for All.
    private func loadCategories() {
        categories = (try? model.engine.categories()) ?? []
        if category >= 0 && !categories.contains(where: { $0.id == category }) || category == -2 && categories.isEmpty { category = -1 }
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
