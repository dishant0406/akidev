import BackgroundTasks
import SwiftUI
import UserNotifications

/// Holds what only a delegate can: the orientations the reader allows, and the background refresh task.
final class AppDelegate: NSObject, UIApplicationDelegate, UNUserNotificationCenterDelegate {
    static let refreshTask = "live.akidev.manga.refresh"
    static let downloadTask = "live.akidev.manga.download"
    nonisolated(unsafe) private static var orientations: UIInterfaceOrientationMask = .allButUpsideDown
    /// What the background task runs, set by the app model.
    @MainActor static var refresh: (() async -> Void)?
    /// What the download task runs: the queue of new chapters.
    @MainActor static var drain: (() async -> Void)?
    /// What tapping a new-chapters notification does.
    @MainActor static var openUpdates: (() -> Void)?

    func application(_ application: UIApplication, didFinishLaunchingWithOptions options: [UIApplication.LaunchOptionsKey: Any]? = nil) -> Bool {
        // The delegate is main-actor, so this closure is too; on the main queue the system calls it where Swift expects it.
        BGTaskScheduler.shared.register(forTaskWithIdentifier: Self.refreshTask, using: .main) { task in
            Self.schedule()
            let work = Task { @MainActor in
                guard let refresh = Self.refresh else { return task.setTaskCompleted(success: false) }
                await refresh()
                task.setTaskCompleted(success: !Task.isCancelled)
            }
            task.expirationHandler = { work.cancel() }
        }
        // Downloads can take minutes, more than a refresh is given, so the system is also asked for a longer slot.
        BGTaskScheduler.shared.register(forTaskWithIdentifier: Self.downloadTask, using: .main) { task in
            let work = Task { @MainActor in
                await Self.drain?()
                task.setTaskCompleted(success: !Task.isCancelled)
            }
            task.expirationHandler = { work.cancel() }
        }
        UNUserNotificationCenter.current().delegate = self
        return true
    }

    nonisolated func userNotificationCenter(_ center: UNUserNotificationCenter, didReceive response: UNNotificationResponse) async {
        await MainActor.run { Self.openUpdates?() }
    }

    nonisolated func userNotificationCenter(_ center: UNUserNotificationCenter, willPresent notification: UNNotification) async -> UNNotificationPresentationOptions {
        [.banner]
    }

    /// Asks the system for time, when it suits it, to finish downloads that did not finish in the refresh.
    static func scheduleDownloads() {
        let request = BGProcessingTaskRequest(identifier: downloadTask)
        request.requiresNetworkConnectivity = true
        try? BGTaskScheduler.shared.submit(request)
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
                // Yellow is too light for text: alert buttons, menu rows and the cursor take the ink colour. Yellow is only ever a fill.
                .tint(Aki.text)
                .fontDesign(.rounded)
                .preferredColorScheme(model.prefs.appearance.scheme)
                .onOpenURL { model.propose(repository: $0.absoluteString) }
                .onChange(of: phase) { _, new in
                    if new == .background { AppDelegate.schedule() }
                    if new == .active { Task { await model.refreshLibrary(); await model.resumeDownloads() } }
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
                // Only the five tabs of the person's kind of reading: more than five and the system folds the rest into "More".
                ForEach(model.tabs, id: \.self) { tab in
                    Tab(value: tab) { screen(of: tab).toolbarVisibility(.hidden, for: .tabBar) }
                }
            }
            if !model.tabBarHidden {
                AkiTabBar(tabs: model.tabs, kind: model.prefs.readerKind, selection: $model.tab, badge: [.updates: model.unreadTotal, .manga: model.unreadTotal])
                    .padding(.bottom, 6)
                    .transition(.move(edge: .bottom).combined(with: .opacity))
            }
        }
        .akiAnimation(value: model.tabBarHidden)
        .background(Aki.background)
        .overlay { if model.prefs.readerKind == nil { OnboardingView().accessibilityAddTraits(.isModal).transition(.opacity) } }
        .akiAnimation(value: model.prefs.readerKind == nil)
        // The bar stays at the bottom of the screen and the keyboard covers it, rather than lifting it over the page.
        .ignoresSafeArea(.keyboard, edges: .bottom)
        .akiSheet(item: $model.pendingRepo) { RepoAddSheet(pending: $0) }
        #if DEBUG
        // `-orientation landscape` turns the screen at start, to look at a landscape layout from a script.
        .task { if UserDefaults.standard.string(forKey: "orientation") != nil { try? await Task.sleep(for: .seconds(1)); AppDelegate.lock(model.prefs.orientation.mask) } }
        #endif
    }
}

extension RootView {
    @ViewBuilder fileprivate func screen(of tab: AppTab) -> some View {
        switch tab {
        case .home: if model.prefs.readerKind == .books { BooksHomeView() } else { HomeView() }
        case .library: LibraryView()
        case .books: BooksView()
        case .updates: UpdatesView()
        case .browse: BooksBrowseView()
        case .manga: MangaHubView()
        case .sources: SourcesView()
        }
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
