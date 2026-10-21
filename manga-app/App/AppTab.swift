import SwiftUI

enum AppTab: Hashable, CaseIterable {
    case home, library, updates, sources

    var title: String {
        switch self {
        case .home: "Home"
        case .library: "Library"
        case .updates: "Updates"
        case .sources: "Sources"
        }
    }

    func symbol(selected: Bool) -> String {
        switch self {
        case .home: selected ? "home-solid" : "home"
        case .library: selected ? "library-solid" : "library"
        case .updates: selected ? "inbox-solid" : "inbox"
        case .sources: selected ? "sources-solid" : "sources"
        }
    }
}
