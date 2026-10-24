import SwiftUI

enum AppTab: Hashable, CaseIterable {
    case home, library, books, updates, sources

    /// The tabs in the order the person's kind of reading wants them: what they read most is first, and where they start. Sources is
    /// always last: it is set up once, not used every day.
    static func order(for kind: ReaderKind?) -> [AppTab] {
        kind == .books ? [.home, .books, .library, .updates, .sources] : [.home, .library, .books, .updates, .sources]
    }

    /// What the tab is called: for someone who reads books, the first tab is their home and the manga tabs say what they are.
    func title(for kind: ReaderKind?) -> String {
        switch self {
        case .home: kind == .books ? "Home" : "Discover"
        case .library: kind == .books ? "Manga" : "Library"
        case .books: "Books"
        case .updates: kind == .books ? "Manga updates" : "Updates"
        case .sources: "Sources"
        }
    }

    func symbol(selected: Bool) -> String {
        switch self {
        case .home: selected ? "home-solid" : "home"
        case .library: selected ? "library-solid" : "library"
        case .books: selected ? "book-solid" : "book"
        case .updates: selected ? "inbox-solid" : "inbox"
        case .sources: selected ? "sources-solid" : "sources"
        }
    }
}
