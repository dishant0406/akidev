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
        case .home: selected ? "house.fill" : "house"
        case .library: selected ? "books.vertical.fill" : "books.vertical"
        case .updates: selected ? "tray.fill" : "tray"
        case .sources: selected ? "puzzlepiece.extension.fill" : "puzzlepiece.extension"
        }
    }
}
