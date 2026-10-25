import SwiftUI

/// The manga screens in the Manga tab, one row of choices: the library's two lists, the updates' two lists, and Discover.
enum MangaPart: Hashable { case saved, history, unread, recent, discover }

private struct MangaPartKey: EnvironmentKey { static let defaultValue: Binding<MangaPart>? = nil }

extension EnvironmentValues {
    /// Set while a manga screen is shown inside the Manga tab: which part is on, to switch it.
    var mangaPart: Binding<MangaPart>? {
        get { self[MangaPartKey.self] }
        set { self[MangaPartKey.self] = newValue }
    }
}

/// Manga for someone who mostly reads books: one tab with the manga library, new chapters and Discover inside it.
struct MangaHubView: View {
    @State private var part = MangaPart.saved

    var body: some View {
        NavigationStack {
            switch part {
            case .saved, .history: LibraryView()
            case .unread, .recent: UpdatesView()
            case .discover: HomeView()
            }
        }
        .toolbar(.hidden, for: .navigationBar)
        .environment(\.mangaPart, $part)
    }
}

/// A manga screen's own navigation stack, unless it is inside the Manga tab, which has one.
struct MangaStack<Content: View>: View {
    @Environment(\.mangaPart) private var part
    @ViewBuilder let content: () -> Content

    var body: some View {
        if part == nil { NavigationStack(root: content) } else { content() }
    }
}

/// The pills that switch between the manga screens, under the header of whichever is showing; nothing outside the Manga tab.
struct MangaPartPills: View {
    @Environment(\.mangaPart) private var part

    var body: some View {
        if let part {
            AkiChips(items: [(MangaPart.saved, "Saved"), (MangaPart.history, "History"), (MangaPart.unread, "Unread"), (MangaPart.recent, "Recent"), (MangaPart.discover, "Discover")], selection: part)
        }
    }
}
