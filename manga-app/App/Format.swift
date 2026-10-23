import AkiManga
import Foundation

/// "Oct 20, 2026" for a time in milliseconds.
func shortDate(_ ms: Int64?) -> String? {
    guard let ms else { return nil }
    return Date(timeIntervalSince1970: Double(ms) / 1000).formatted(.dateTime.month(.abbreviated).day().year())
}

/// A chapter's name: its title, else "Chapter 12".
func chapterName(number: Double?, title: String?) -> String {
    if let title, !title.isEmpty { return title }
    if let number { return "Chapter \(number.formatted(.number.precision(.fractionLength(0...2))))" }
    return "Chapter"
}

/// "120 chapters" for a card, when its source said how many there are.
func chaptersLabel(_ count: UInt32?) -> String? {
    guard let count, count > 0 else { return nil }
    return count == 1 ? "1 chapter" : "\(count) chapters"
}

/// The whole chapter numbers a source skips between a chapter and the next one it lists: 178.5 then 180 skips 179.
func skippedChapters(from number: Double, to next: Double) -> ClosedRange<Int>? {
    let first = Int(number.rounded(.down)) + 1
    let last = Int(next.rounded(.down)) - 1
    return first <= last ? first...last : nil
}

extension HistoryEntry {
    var key: String { "\(source)/\(series)" }
}
