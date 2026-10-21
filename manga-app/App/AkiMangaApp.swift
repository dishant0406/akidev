import BackgroundTasks
import SwiftUI

/// Holds what only a delegate can: the orientations the reader allows, and the background refresh task.
final class AppDelegate: NSObject, UIApplicationDelegate {
    static let refreshTask = "live.akidev.manga.refresh"
    nonisolated(unsafe) private static var orientations: UIInterfaceOrientationMask = .allButUpsideDown
    /// What the background task runs, set by the app model.
    @MainActor static var refresh: (() async -> Void)?

    func application(_ application: UIApplication, didFinishLaunchingWithOptions options: [UIApplication.LaunchOptionsKey: Any]? = nil) -> Bool {
        BGTaskScheduler.shared.register(forTaskWithIdentifier: Self.refreshTask, using: nil) { task in
            Self.schedule()
            let work = Task { @MainActor in
                guard let refresh = Self.refresh else { return task.setTaskCompleted(success: false) }
                await refresh()
                task.setTaskCompleted(success: !Task.isCancelled)
            }
            task.expirationHandler = { work.cancel() }
        }
        return true
    }

    func application(_ application: UIApplication, supportedInterfaceOrientationsFor window: UIWindow?) -> UIInterfaceOrientationMask { Self.orientations }

    /// Asks the system to wake the app in the next hour or so to look for new chapters; it decides when.
    static func schedule() {
        let request = BGAppRefreshTaskRequest(identifier: refreshTask)
        request.earliestBeginDate = Date(timeIntervalSinceNow: 3600)
        try? BGTaskScheduler.shared.submit(request)
    }

    @MainActor static func lock(_ mask: UIInterfaceOrientationMask) {
        orientations = mask
        UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.forEach { scene in
            scene.requestGeometryUpdate(.iOS(interfaceOrientations: mask)) { _ in }
        }
    }
}

@main
struct AkiMangaApp: App {
    @UIApplicationDelegateAdaptor(AppDelegate.self) private var delegate
    @Environment(\.scenePhase) private var phase
    @State private var model = AppModel()

    var body: some Scene {
        WindowGroup {
            RootView()
                .environment(model)
                .tint(Aki.yellow)
                .fontDesign(.rounded)
                .preferredColorScheme(model.prefs.appearance.scheme)
                .onOpenURL { model.propose(repository: $0.absoluteString) }
                .onChange(of: phase) { _, new in
                    if new == .background { AppDelegate.schedule() }
                    if new == .active { Task { await model.refreshLibrary() } }
                }
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
                AkiTabBar(selection: $model.tab, badge: [.updates: model.unreadTotal])
                    .padding(.bottom, 6)
                    .transition(.move(edge: .bottom).combined(with: .opacity))
            }
        }
        .animation(.smooth(duration: 0.3), value: model.tabBarHidden)
        .background(Aki.canvas)
        .akiSheet(item: $model.pendingRepo) { RepoAddSheet(pending: $0) }
    }
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
