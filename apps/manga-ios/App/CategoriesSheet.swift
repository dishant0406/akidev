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
                AkiText(Copy.Library.categories, .title).accessibilityAddTraits(.isHeader)
                if categories.isEmpty {
                    AkiEmptyState(title: Copy.Library.noCategoriesYet, message: Copy.Library.categoriesExplainer)
                        .frame(minHeight: 220)
                }
                ForEach(Array(categories.enumerated()), id: \.element.id) { index, category in
                    AkiRow(title: category.name, subtitle: contents(of: category)) {
                        AkiMenu(symbol: "more", label: Copy.Library.optionsFor(category.name), items: menu(for: category, at: index))
                    }
                }
                AkiButton(Copy.Library.addCategory, icon: "folder-add", variant: .primary, size: .large, fullWidth: true) { renaming = nil; name = ""; naming = true }
            }
            .padding(24)
        }
        .onAppear(perform: load)
        .akiPrompt(renaming == nil ? Copy.Library.newCategory : Copy.Library.renameCategory, message: Copy.Library.chooseCategoryName, placeholder: Copy.Library.categoryNamePlaceholder, confirm: renaming == nil ? Copy.Library.addCategory : Copy.Library.renameCategory, text: $name, isPresented: $naming, onConfirm: save)
        .akiConfirm(Copy.Library.deleteCategoryQuestion(deleting?.name ?? ""), message: Copy.Library.categoryContentsStay, confirm: Copy.Library.deleteCategory, isPresented: Binding(get: { deleting != nil }, set: { if !$0 { deleting = nil } })) {
            if let deleting {
                try? model.engine.deleteCategory(id: deleting.id)
                if model.prefs.saveCategory == deleting.id { model.prefs.saveCategory = 0 }
            }
            reload()
        }
        .akiAlert(Copy.Library.couldNotSaveCategory, message: problem, isPresented: Binding(get: { problem != nil }, set: { if !$0 { problem = nil } }))
    }

    private func menu(for category: CategoryInfo, at index: Int) -> [AkiMenuItem] {
        var items = [AkiMenuItem(title: Copy.Library.rename, icon: "sliders") { renaming = category; name = category.name; naming = true }]
        if index > 0 { items.append(AkiMenuItem(title: Copy.Library.moveUp, icon: "order") { move(index, by: -1) }) }
        if index < categories.count - 1 { items.append(AkiMenuItem(title: Copy.Library.moveDown, icon: "down") { move(index, by: 1) }) }
        items.append(AkiMenuItem(title: Copy.Library.delete, icon: "trash", destructive: true) { deleting = category })
        return items
    }

    private func contents(of category: CategoryInfo) -> String {
        let parts = [category.count > 0 ? Copy.Library.seriesCount(Int(category.count)) : nil,
                     category.books > 0 ? Copy.Library.bookCount(Int(category.books)) : nil].compactMap { $0 }
        return parts.isEmpty ? Copy.Library.empty : parts.joined(separator: " · ")
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
                AkiText(Copy.Library.categories, .title).accessibilityAddTraits(.isHeader)
                if categories.isEmpty {
                    AkiEmptyState(title: Copy.Library.noCategoriesYet, message: Copy.Library.noCategoriesYetFromMenu).frame(minHeight: 220)
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
