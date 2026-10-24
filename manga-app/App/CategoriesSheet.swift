import AkiManga
import SwiftUI

/// Add, rename, put in order and delete the categories: the shelves of the library.
struct CategoriesSheet: View {
    @Environment(AppModel.self) private var model
    @State private var categories: [CategoryInfo] = []
    @State private var naming = false
    /// The category being renamed; nil when the name asked for is a new one.
    @State private var renaming: CategoryInfo?
    @State private var name = ""
    @State private var deleting: CategoryInfo?
    @State private var problem: String?

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                AkiText("Categories", .title)
                if categories.isEmpty {
                    AkiEmptyState(title: "No categories yet", message: "A category is a shelf, for series in your library and for books. Make one, then put series or books on it.")
                        .frame(minHeight: 220)
                }
                ForEach(Array(categories.enumerated()), id: \.element.id) { index, category in
                    AkiRow(title: category.name, subtitle: contents(of: category)) {
                        AkiMenu(symbol: "more", label: "Options for \(category.name)", items: menu(for: category, at: index))
                    }
                }
                AkiButton("Add a category", icon: "folder-add", variant: .primary, size: .large, fullWidth: true) { renaming = nil; name = ""; naming = true }
            }
            .padding(24)
        }
        .onAppear(perform: load)
        .akiPrompt(renaming == nil ? "New category" : "Rename category", message: "A name for a shelf, for series and for books.", placeholder: "Reading", confirm: renaming == nil ? "Add" : "Rename", text: $name, isPresented: $naming, onConfirm: save)
        .akiConfirm("Delete “\(deleting?.name ?? "")”?", message: "The series and books on it stay where they are.", confirm: "Delete", isPresented: Binding(get: { deleting != nil }, set: { if !$0 { deleting = nil } })) {
            if let deleting {
                try? model.engine.deleteCategory(id: deleting.id)
                if model.prefs.saveCategory == deleting.id { model.prefs.saveCategory = 0 }
            }
            reload()
        }
        .akiAlert("That did not work", message: problem, isPresented: Binding(get: { problem != nil }, set: { if !$0 { problem = nil } }))
    }

    private func menu(for category: CategoryInfo, at index: Int) -> [AkiMenuItem] {
        var items = [AkiMenuItem(title: "Rename", icon: "sliders") { renaming = category; name = category.name; naming = true }]
        if index > 0 { items.append(AkiMenuItem(title: "Move up", icon: "order") { move(index, by: -1) }) }
        if index < categories.count - 1 { items.append(AkiMenuItem(title: "Move down", icon: "down") { move(index, by: 1) }) }
        items.append(AkiMenuItem(title: "Delete", icon: "trash", destructive: true) { deleting = category })
        return items
    }

    private func contents(of category: CategoryInfo) -> String {
        let parts = [category.count > 0 ? (category.count == 1 ? "1 series" : "\(category.count) series") : nil,
                     category.books > 0 ? (category.books == 1 ? "1 book" : "\(category.books) books") : nil].compactMap { $0 }
        return parts.isEmpty ? "Empty" : parts.joined(separator: " · ")
    }

    private func load() { categories = (try? model.engine.categories()) ?? [] }

    /// Reads the categories again and tells the library (the screen under this sheet) to do the same.
    private func reload() { load(); model.refresh() }

    private func save() {
        let text = name.trimmingCharacters(in: .whitespacesAndNewlines)
        name = ""
        guard !text.isEmpty else { return }
        do {
            if let renaming { try model.engine.renameCategory(id: renaming.id, name: text) } else { _ = try model.engine.addCategory(name: text) }
            reload()
        } catch { problem = describe(error).message }
    }

    private func move(_ index: Int, by step: Int) {
        var ids = categories.map(\.id)
        ids.swapAt(index, index + step)
        try? model.engine.reorderCategories(ids: ids)
        reload()
    }
}

/// A series (or several) and the categories it is on, to change: each category is a switch, on when every one of them is on it.
struct CategoryPicker: View {
    struct Series: Hashable {
        let source: String
        let id: String
    }

    let series: [Series]
    /// Books to put on categories, instead of series.
    var books: [BookInfo] = []
    @Environment(AppModel.self) private var model
    @State private var categories: [CategoryInfo] = []
    @State private var on: Set<Int64> = []

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                AkiText("Categories", .title)
                if categories.isEmpty {
                    AkiEmptyState(title: "No categories yet", message: "Make one from the menu on the Library or Books screen, under Categories.").frame(minHeight: 220)
                } else {
                    AkiCard(padding: 16) {
                        VStack(spacing: 16) {
                            ForEach(categories, id: \.id) { category in
                                AkiSwitch(title: category.name, isOn: Binding(get: { on.contains(category.id) }, set: { set(category, $0) }))
                            }
                        }
                    }
                }
            }
            .padding(24)
        }
        .onAppear {
            categories = (try? model.engine.categories()) ?? []
            let entries = model.library.filter { series.contains(Series(source: $0.source, id: $0.id)) }
            on = Set(categories.map(\.id).filter { id in
                !entries.isEmpty && entries.allSatisfy { $0.categories.contains(id) } || !books.isEmpty && books.allSatisfy { $0.categories.contains(id) }
            })
        }
    }

    private func set(_ category: CategoryInfo, _ member: Bool) {
        for item in series { try? model.engine.setCategory(source: item.source, series: item.id, category: category.id, member: member) }
        for book in books { try? model.engine.setBookCategory(id: book.id, category: category.id, member: member) }
        if member { on.insert(category.id) } else { on.remove(category.id) }
        if !series.isEmpty { model.refresh() }
    }
}
