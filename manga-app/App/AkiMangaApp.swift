import SwiftUI

@main
struct AkiMangaApp: App {
    @State private var model = AppModel()

    var body: some Scene {
        WindowGroup {
            RootView()
                .environment(model)
                .preferredColorScheme(.dark)
                .tint(Aki.accent)
        }
    }
}

struct RootView: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        #if DEBUG
        // `-autoSeries local|solo-leveling|Title` opens straight on a series, `-autoContinue YES` then opens
        // the reader where it left off: for driving the app from a script.
        if let spec = UserDefaults.standard.string(forKey: "autoSeries")?.split(separator: "|").map(String.init), spec.count == 3 {
            NavigationStack { SeriesView(ref: SeriesRef(source: spec[0], id: spec[1], title: spec[2], cover: nil)) }
        } else {
            tabs
        }
        #else
        tabs
        #endif
    }

    private var tabs: some View {
        @Bindable var model = model
        return TabView(selection: $model.tab) {
            Tab("Library", systemImage: "books.vertical", value: AppTab.library) { LibraryView() }
            Tab("Browse", systemImage: "square.grid.2x2", value: AppTab.browse) { BrowseView() }
            Tab("Sources", systemImage: "puzzlepiece.extension", value: AppTab.sources) { SourcesView() }
        }
        .background(Aki.canvas)
    }
}
