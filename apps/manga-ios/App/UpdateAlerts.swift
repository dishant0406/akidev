import AkiManga
import Foundation
import UserNotifications

/// One series that got chapters since it was last asked, and the newest of them.
struct NewChapters: Sendable {
    let source: String
    let series: String
    let title: String
    let adult: Bool
    /// Oldest first.
    let chapters: [ChapterEntry]
}

/// Local notifications for new chapters. They are posted by the app itself when iOS wakes it to look for updates, so
/// nothing leaves the device and no server is involved; the price is that iOS, not Akidev, decides when that happens.
enum UpdateAlerts {
    static func status() async -> UNAuthorizationStatus {
        await UNUserNotificationCenter.current().notificationSettings().authorizationStatus
    }

    /// Asks the person once; a person who said no has to turn it on in iOS Settings, and this says so by returning false.
    static func requestPermission() async -> Bool {
        let center = UNUserNotificationCenter.current()
        switch await status() {
        case .authorized, .provisional, .ephemeral: return true
        case .denied: return false
        case .notDetermined: return (try? await center.requestAuthorization(options: [.alert, .sound])) ?? false
        @unknown default: return false
        }
    }

    /// One notification for a whole refresh, however many series it found chapters for. Titles of 18+ sources stay off
    /// the lock screen.
    static func post(_ found: [NewChapters]) async {
        guard !found.isEmpty, await status() == .authorized else { return }
        let content = UNMutableNotificationContent()
        content.title = "New chapters"
        content.body = body(found)
        content.sound = .default
        content.threadIdentifier = "new-chapters"
        let request = UNNotificationRequest(identifier: "new-chapters-\(Int(Date().timeIntervalSince1970))", content: content, trigger: nil)
        try? await UNUserNotificationCenter.current().add(request)
    }

    /// "Solo Leveling · Chapter 179", "Solo Leveling · 3 new chapters", "Solo Leveling, Tower of God and 2 more".
    static func body(_ found: [NewChapters]) -> String {
        let count = found.reduce(0) { $0 + $1.chapters.count }
        if found.contains(where: \.adult) { return count == 1 ? "1 new chapter in your library." : "\(count) new chapters in your library." }
        if found.count == 1, let only = found.first {
            let what = only.chapters.count == 1 ? chapterName(number: only.chapters[0].number, title: only.chapters[0].title) : "\(only.chapters.count) new chapters"
            return "\(only.title) · \(what)"
        }
        let names = found.prefix(2).map(\.title)
        let rest = found.count - names.count
        return rest > 0 ? "\(names.joined(separator: ", ")) and \(rest) more" : names.joined(separator: " and ")
    }
}
