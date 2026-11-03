import AkiManga
import Foundation

extension AppModel {
    /// A book or a comic opened from Files, Mail or another app with "Open in Akidev".
    func open(file url: URL) {
        let engine = engine
        let ext = url.pathExtension.lowercased()
        guard ["epub", "cbz", "zip"].contains(ext) else {
            fileNotice = ("Akidev cannot open this file", "It reads EPUB books and CBZ comics.")
            return
        }
        let scoped = url.startAccessingSecurityScopedResource()
        let folder = FileManager.default.temporaryDirectory.appendingPathComponent("opened-\(UUID().uuidString)")
        let copy = folder.appendingPathComponent(url.lastPathComponent)
        let copied = (try? FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)) != nil && (try? FileManager.default.copyItem(at: url, to: copy)) != nil
        if scoped { url.stopAccessingSecurityScopedResource() }
        guard copied else {
            fileNotice = ("That file could not be read", "Try opening it again.")
            return
        }
        Task {
            defer { try? FileManager.default.removeItem(at: folder) }
            do {
                if ext == "epub" {
                    let book = try await Task.detached { try engine.importBook(path: copy.path) }.value
                    booksRevision += 1
                    fileNotice = ("Added to your books", book.title)
                } else {
                    _ = try await Task.detached { try engine.importFile(path: copy.path) }.value
                    refresh()
                    fileNotice = ("Added to your library", "Find it under Saved.")
                }
            } catch { fileNotice = describe(error) }
        }
    }
}
