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
        TabView {
            Tab("Library", systemImage: "books.vertical") { LibraryView() }
            Tab("Browse", systemImage: "square.grid.2x2") { BrowseView() }
            Tab("Sources", systemImage: "externaldrive.connected.to.line.below") { SourcesView() }
        }
        .background(Aki.canvas)
    }
}
