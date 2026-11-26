import Darwin
import Foundation

/// The lock screen's card and the headphones' buttons, through MediaPlayer. The framework takes about 20 ms to load, which every
/// launch would pay if it were linked, so it is loaded here, when the first book is read aloud, and found by name.
@MainActor
final class LockScreen {
    private static let handle = dlopen("/System/Library/Frameworks/MediaPlayer.framework/MediaPlayer", RTLD_NOW)
    private var targets: [(command: NSObject, target: AnyObject)] = []

    /// A string constant the framework exports, by its name.
    private static func constant(_ name: String) -> String? {
        guard let handle, let symbol = dlsym(handle, name) else { return nil }
        return symbol.assumingMemoryBound(to: NSString?.self).pointee as String?
    }

    private static func shared(_ className: String, _ selector: String) -> NSObject? {
        guard handle != nil, let type = NSClassFromString(className) as? NSObject.Type else { return nil }
        return type.perform(NSSelectorFromString(selector))?.takeUnretainedValue() as? NSObject
    }

    /// Asks to be told when the person uses one of the lock screen's or the headphones' buttons.
    func listen(play: @escaping () -> Void, pause: @escaping () -> Void, toggle: @escaping () -> Void, next: @escaping () -> Void, previous: @escaping () -> Void) {
        guard let center = Self.shared("MPRemoteCommandCenter", "sharedCommandCenter") else { return }
        let actions: [(String, () -> Void)] = [("playCommand", play), ("pauseCommand", pause), ("togglePlayPauseCommand", toggle), ("nextTrackCommand", next), ("previousTrackCommand", previous)]
        for (name, action) in actions {
            guard let command = center.value(forKey: name) as? NSObject else { continue }
            command.setValue(true, forKey: "enabled")
            // `addTargetWithHandler:` takes a block that answers with a status; 0 is success.
            let handler: @convention(block) (AnyObject) -> Int = { _ in
                Task { @MainActor in action() }
                return 0
            }
            if let target = command.perform(NSSelectorFromString("addTargetWithHandler:"), with: handler)?.takeUnretainedValue() {
                targets.append((command, target))
            }
        }
    }

    func stopListening() {
        for (command, target) in targets { _ = command.perform(NSSelectorFromString("removeTarget:"), with: target) }
        targets = []
    }

    /// What the lock screen shows: the book, its author, and whether it is playing.
    func show(title: String, author: String?, playing: Bool) {
        var info: [String: Any] = [:]
        if let key = Self.constant("MPMediaItemPropertyTitle") { info[key] = title }
        if let key = Self.constant("MPMediaItemPropertyArtist"), let author { info[key] = author }
        if let key = Self.constant("MPNowPlayingInfoPropertyPlaybackRate") { info[key] = playing ? 1.0 : 0.0 }
        Self.shared("MPNowPlayingInfoCenter", "defaultCenter")?.setValue(info, forKey: "nowPlayingInfo")
    }

    func clear() {
        Self.shared("MPNowPlayingInfoCenter", "defaultCenter")?.setValue(nil, forKey: "nowPlayingInfo")
    }
}
