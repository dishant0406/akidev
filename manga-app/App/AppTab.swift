import SwiftUI

enum AppTab: Hashable, CaseIterable {
    case home, library, books, updates, sources

    /// The tabs in the order the person's kind of reading wants them: what they read most is first, and where they start.
    static func order(for kind: ReaderKind?) -> [AppTab] {
        kind == .books ? [.books, .sources, .library, .updates, .home] : [.home, .library, .books, .updates, .sources]
    }

    var title: String {
        switch self {
        case .home: "Discover"
        case .library: "Library"
        case .books: "Books"
        case .updates: "Updates"
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
