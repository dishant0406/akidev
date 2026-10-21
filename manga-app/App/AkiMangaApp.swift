import SwiftUI

@main
struct AkiMangaApp: App {
    @State private var model = AppModel()

    var body: some Scene {
        WindowGroup {
            RootView()
                .environment(model)
                .tint(Aki.yellow)
                .fontDesign(.rounded)
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
            shell
        }
        #else
        shell
        #endif
    }

    private var shell: some View {
        @Bindable var model = model
        return ZStack(alignment: .bottom) {
            TabView(selection: $model.tab) {
                Tab(value: AppTab.home) { HomeView().toolbarVisibility(.hidden, for: .tabBar) }
                Tab(value: AppTab.library) { LibraryView().toolbarVisibility(.hidden, for: .tabBar) }
                Tab(value: AppTab.updates) { UpdatesView().toolbarVisibility(.hidden, for: .tabBar) }
                Tab(value: AppTab.sources) { SourcesView().toolbarVisibility(.hidden, for: .tabBar) }
            }
            if !model.tabBarHidden {
                FloatingTabBar(selection: $model.tab, badge: [.updates: model.unreadTotal])
                    .padding(.bottom, 6)
                    .transition(.move(edge: .bottom).combined(with: .opacity))
            }
        }
        .animation(.smooth(duration: 0.3), value: model.tabBarHidden)
        .background(Aki.canvas)
    }
}

/// Added to a screen that scrolls under the floating tab bar, so its last row can be reached.
extension View {
    func underTabBar() -> some View { contentMargins(.bottom, 104, for: .scrollContent) }
}

/// A screen pushed over a tab that hides the tab bar while it is on screen.
private struct HidesTabBar: ViewModifier {
    @Environment(AppModel.self) private var model
    func body(content: Content) -> some View {
        content
            .onAppear { model.barHiders += 1 }
            .onDisappear { model.barHiders = max(model.barHiders - 1, 0) }
    }
}

extension View {
    func hidesTabBar() -> some View { modifier(HidesTabBar()) }
}
