import AkiManga
import Foundation

enum BookSort: String, CaseIterable {
    case recent = "Last read", added = "Date added", title = "Title", author = "Author", progress = "Progress"
}

enum BookFilter: String, CaseIterable {
    case all = "All books", reading = "Reading", notStarted = "Not started", finished = "Finished"
}

/// What is on the shelf: the books on the category chosen (-1 is every one, -2 the ones on none), those that fit the filter and the
/// words typed, in the order asked for. Ties fall back to the title so the order never reshuffles on its own.
func shelf(_ books: [BookInfo], sort: BookSort, reversed: Bool, filter: BookFilter, category: Int64, words: String) -> [BookInfo] {
    let words = words.trimmingCharacters(in: .whitespacesAndNewlines)
    let kept = books.filter { book in
        switch category {
        case -1: true
        case -2: book.categories.isEmpty
        default: book.categories.contains(category)
        }
    }.filter { book in
        switch filter {
        case .all: true
        case .reading: book.openedMs != nil && !book.finished
        case .notStarted: book.openedMs == nil
        case .finished: book.finished
        }
    }.filter { book in
        words.isEmpty || book.title.localizedCaseInsensitiveContains(words) || book.authors.contains { $0.localizedCaseInsensitiveContains(words) }
    }
    func byTitle(_ a: BookInfo, _ b: BookInfo) -> Bool { a.title.localizedCaseInsensitiveCompare(b.title) == .orderedAscending }
    func biggestFirst(_ value: (BookInfo) -> Double) -> [BookInfo] {
        kept.sorted { value($0) != value($1) ? value($0) > value($1) : byTitle($0, $1) }
    }
    let ordered: [BookInfo]
    switch sort {
    case .recent: ordered = biggestFirst { Double($0.openedMs ?? $0.addedMs) }
    case .added: ordered = biggestFirst { Double($0.addedMs) }
    case .title: ordered = kept.sorted(by: byTitle)
    case .author: ordered = kept.sorted { ($0.authors.first ?? "￿").localizedCaseInsensitiveCompare($1.authors.first ?? "￿") == .orderedAscending }
    case .progress: ordered = biggestFirst { $0.finished ? 1 : $0.progress }
    }
    return reversed ? ordered.reversed() : ordered
}
