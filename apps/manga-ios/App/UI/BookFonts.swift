import AkiManga
import CoreText
import UIKit

/// The fonts a book carries, read from its files. A font the system cannot read (a web font format) is left out and the
/// reader's own font is used for that text.
final class BookFontBank: @unchecked Sendable {
    private struct Entry {
        let descriptor: CTFontDescriptor
        let bold: Bool
        let italic: Bool
    }

    private let lock = NSLock()
    private var entries: [String: [Entry]] = [:]
    private var tried: Set<String> = []

    /// Whether this font file has been looked at already, so each is read once.
    func needs(_ face: BookFace) -> Bool {
        lock.withLock { !tried.contains(face.path) }
    }

    func add(_ face: BookFace, data: Data?) {
        lock.withLock {
            tried.insert(face.path)
            guard let data, let descriptor = CTFontManagerCreateFontDescriptorFromData(data as CFData) else { return }
            entries[face.family.lowercased(), default: []].append(Entry(descriptor: descriptor, bold: face.bold, italic: face.italic))
        }
    }

    /// The book's font of that name, in the style nearest to the one asked for.
    func font(family: String, size: CGFloat, bold: Bool, italic: Bool) -> UIFont? {
        let found = lock.withLock { entries[family.lowercased()] }
        guard let found else { return nil }
        let best = found.min { score($0, bold, italic) < score($1, bold, italic) }
        return best.map { UIFont(descriptor: $0.descriptor as UIFontDescriptor, size: size) }
    }

    private func score(_ entry: Entry, _ bold: Bool, _ italic: Bool) -> Int {
        (entry.bold == bold ? 0 : 2) + (entry.italic == italic ? 0 : 1)
    }
}
