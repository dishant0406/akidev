import AVFAudio
import AkiManga
import MediaPlayer
import Observation

/// Reads a book aloud, one sentence at a time, from the page the person is on.
///
/// The voice model makes the speech on this phone a few sentences ahead of the one being heard; each is queued on an audio
/// player as it is ready, and the sentence heard is what the page marks. It carries on through the chapters of the book and
/// stops at the back matter.
@MainActor @Observable
final class BookListener {
    enum State { case loading, playing, paused, stopped }

    private(set) var state = State.loading
    /// The sentence being heard, for the page to mark.
    private(set) var current: BookSpan?
    /// How fast it is read, as a multiple of the voice's own pace.
    var rate: Double { didSet { pitch.rate = Float(rate); saveRate(rate) } }

    private let reader: BookReader
    private let engine: Engine
    /// What the lock screen shows: the book's title and author.
    private let title: String
    private let author: String?
    private let model: String
    private let sections: [BookSection]
    private let saveRate: (Double) -> Void
    /// Told each time another sentence starts, to move the page to it.
    private let follow: (BookSpan) -> Void
    /// Told when reading ends, of itself (nothing) or because of a failure (what to tell the person).
    private let ended: ((title: String, message: String)?) -> Void

    private let audio = AVAudioEngine()
    private let player = AVAudioPlayerNode()
    private let pitch = AVAudioUnitTimePitch()
    private var speaker: VoiceSpeaker?
    /// The sentences queued on the player, the first being heard now.
    private var queued: [BookSpan] = []
    private var finishedQueueing = false
    /// Bumped by anything that changes where reading is, so what the old position still has in flight is ignored.
    private var generation = 0
    /// The task making the speech; a new one waits for the one before it, so the voice is never asked twice at once.
    private var producing: Task<Void, Never>?
    private var sentences: [Int: [BookSpeech]] = [:]
    private var observers: [NSObjectProtocol] = []
    private var commands: [(command: MPRemoteCommand, target: Any)] = []
    private var idle: Task<Void, Never>?
    /// How many sentences the queue holds, counting the one being heard.
    private let queueLimit = 3

    init(reader: BookReader, engine: Engine, title: String, author: String?, model: String, sections: [BookSection], rate: Double, saveRate: @escaping (Double) -> Void, follow: @escaping (BookSpan) -> Void, ended: @escaping ((title: String, message: String)?) -> Void) {
        self.reader = reader
        self.engine = engine
        self.title = title
        self.author = author
        self.model = model
        self.sections = sections
        self.rate = rate
        self.saveRate = saveRate
        self.follow = follow
        self.ended = ended
        pitch.rate = Float(rate)
    }

    // MARK: Controls

    /// Starts reading at `offset` in `chapter`.
    func start(chapter: Int, offset: Int) async {
        let mine = generation
        let engine = engine, model = model
        do {
            let speaker = try await Task.detached(priority: .userInitiated) { try engine.openSpeaker(id: model) }.value
            // Stopped while the model was loading: nothing may start behind the person's back.
            guard mine == generation else { return }
            self.speaker = speaker
            try begin(sampleRate: Double(speaker.sampleRate()))
        } catch {
            return stop(failure: describe(error))
        }
        run(chapter: chapter, offset: offset)
    }

    func pause() {
        guard state == .playing else { return }
        player.pause()
        state = .paused
        publishNowPlaying()
        // A paused voice is not kept in memory for long.
        idle = Task { [weak self] in
            try? await Task.sleep(for: .seconds(180))
            guard !Task.isCancelled else { return }
            self?.stop()
        }
    }

    func resume() {
        guard state == .paused else { return }
        idle?.cancel()
        // A call or a change of headphones stops the audio engine: start again from the sentence that was being heard.
        if !audio.isRunning, let current { return restart(at: current) }
        player.play()
        state = .playing
        publishNowPlaying()
    }

    func toggle() { state == .paused ? resume() : pause() }

    /// Goes to the sentence after or before the one being heard.
    func skip(forward: Bool) {
        guard let current else { return }
        Task {
            guard let list = await sentences(of: current.chapter), let index = list.firstIndex(where: { $0.start == current.start }) else { return }
            if forward, index + 1 >= list.count {
                guard let next = chapter(after: current.chapter) else { return stop() }
                return run(chapter: next, offset: 0)
            }
            run(chapter: current.chapter, offset: Int(list[forward ? index + 1 : max(index - 1, 0)].start))
        }
    }

    func stop() { stop(failure: nil) }

    private func stop(failure: (title: String, message: String)?) {
        guard state != .stopped else { return }
        generation += 1
        idle?.cancel()
        player.stop()
        audio.stop()
        speaker = nil
        queued = []
        current = nil
        state = .stopped
        for observer in observers { NotificationCenter.default.removeObserver(observer) }
        observers = []
        for (command, target) in commands { command.removeTarget(target) }
        commands = []
        MPNowPlayingInfoCenter.default().nowPlayingInfo = nil
        try? AVAudioSession.sharedInstance().setActive(false, options: .notifyOthersOnDeactivation)
        ended(failure)
    }

    // MARK: Audio

    private func begin(sampleRate: Double) throws {
        let format = AVAudioFormat(standardFormatWithSampleRate: sampleRate, channels: 1)
        audio.attach(player)
        audio.attach(pitch)
        audio.connect(player, to: pitch, format: format)
        audio.connect(pitch, to: audio.mainMixerNode, format: format)
        try startAudio()
        let center = NotificationCenter.default
        // A call or an alarm pauses reading, and so does taking the headphones off.
        observers.append(center.addObserver(forName: AVAudioSession.interruptionNotification, object: nil, queue: .main) { [weak self] note in
            guard note.userInfo?[AVAudioSessionInterruptionTypeKey] as? UInt == AVAudioSession.InterruptionType.began.rawValue else { return }
            MainActor.assumeIsolated { self?.pause() }
        })
        observers.append(center.addObserver(forName: AVAudioSession.routeChangeNotification, object: nil, queue: .main) { [weak self] note in
            guard note.userInfo?[AVAudioSessionRouteChangeReasonKey] as? UInt == AVAudioSession.RouteChangeReason.oldDeviceUnavailable.rawValue else { return }
            MainActor.assumeIsolated { self?.pause() }
        })
        // Another output (Bluetooth, say) stops the audio engine, and what was queued on it with it: carry on from the sentence heard.
        observers.append(center.addObserver(forName: .AVAudioEngineConfigurationChange, object: audio, queue: .main) { [weak self] _ in
            MainActor.assumeIsolated {
                guard let self, self.state == .playing || self.state == .loading, let current = self.current else { return }
                self.restart(at: current)
            }
        })
        // The lock screen and the headphones' buttons.
        let remote = MPRemoteCommandCenter.shared()
        let handlers: [(MPRemoteCommand, @MainActor (BookListener) -> Void)] = [
            (remote.playCommand, { $0.resume() }), (remote.pauseCommand, { $0.pause() }), (remote.togglePlayPauseCommand, { $0.toggle() }),
            (remote.nextTrackCommand, { $0.skip(forward: true) }), (remote.previousTrackCommand, { $0.skip(forward: false) }),
        ]
        for (command, handler) in handlers {
            command.isEnabled = true
            commands.append((command, command.addTarget { [weak self] _ in
                Task { @MainActor in if let self { handler(self) } }
                return .success
            }))
        }
    }

    private func startAudio() throws {
        let session = AVAudioSession.sharedInstance()
        try session.setCategory(.playback, mode: .spokenAudio)
        try session.setActive(true)
        try audio.start()
    }

    private func restart(at span: BookSpan) {
        do { try startAudio() } catch { return stop(failure: describe(error)) }
        run(chapter: span.chapter, offset: span.start)
    }

    // MARK: Reading

    /// Reads from `offset` in `chapter`, dropping whatever was queued for any other place.
    private func run(chapter: Int, offset: Int) {
        generation += 1
        let mine = generation
        player.stop()
        queued = []
        current = nil
        finishedQueueing = false
        idle?.cancel()
        state = .loading
        let before = producing
        producing = Task {
            await before?.value
            await produce(chapter: chapter, offset: offset, generation: mine)
        }
    }

    /// Makes the speech of each sentence in turn and queues it, never more than `queueLimit` sentences in the queue.
    private func produce(chapter first: Int, offset: Int, generation mine: Int) async {
        var chapter = first, from = offset
        var failures = 0
        while mine == generation {
            guard let list = await sentences(of: chapter) else { return }
            let start = list.firstIndex { Int($0.start + $0.length) > from } ?? list.count
            for sentence in list[start...] {
                while queued.count >= queueLimit, mine == generation { try? await Task.sleep(for: .milliseconds(100)) }
                guard mine == generation, let speaker else { return }
                let text = sentence.text
                let samples: [Float]
                do {
                    samples = try await Task.detached(priority: .userInitiated) { try speaker.speak(text: text) }.value
                } catch {
                    failures += 1
                    if failures >= 3, mine == generation { stop(failure: describe(error)) }
                    continue
                }
                failures = 0
                guard mine == generation else { return }
                queue(samples, pause: sentence.heading, span: BookSpan(chapter: chapter, start: Int(sentence.start), end: Int(sentence.start + sentence.length)), generation: mine)
            }
            guard let next = self.chapter(after: chapter) else { break }
            chapter = next
            from = 0
        }
        guard mine == generation else { return }
        finishedQueueing = true
        if queued.isEmpty { stop() }
    }

    private func queue(_ samples: [Float], pause: Bool, span: BookSpan, generation mine: Int) {
        guard let format = AVAudioFormat(standardFormatWithSampleRate: Double(speaker?.sampleRate() ?? 24_000), channels: 1) else { return }
        // A heading is followed by a pause, as a reader would leave one.
        let silence = pause ? Int(format.sampleRate * 0.6) : 0
        let frames = AVAudioFrameCount(samples.count + silence)
        guard let buffer = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: frames), let channel = buffer.floatChannelData?[0] else { return }
        buffer.frameLength = frames
        samples.withUnsafeBufferPointer { channel.update(from: $0.baseAddress!, count: samples.count) }
        (channel + samples.count).update(repeating: 0, count: silence)
        queued.append(span)
        if queued.count == 1 { heard(span) }
        player.scheduleBuffer(buffer, completionCallbackType: .dataPlayedBack) { [weak self] _ in
            Task { @MainActor in self?.played(generation: mine) }
        }
        if state == .loading {
            player.play()
            state = .playing
            publishNowPlaying()
        }
    }

    /// The sentence at the front of the queue has been heard.
    private func played(generation mine: Int) {
        guard mine == generation, !queued.isEmpty else { return }
        queued.removeFirst()
        if let next = queued.first { heard(next) } else if finishedQueueing { stop() }
    }

    private func publishNowPlaying() {
        var info: [String: Any] = [MPMediaItemPropertyTitle: title, MPNowPlayingInfoPropertyPlaybackRate: state == .playing ? 1.0 : 0.0]
        if let author { info[MPMediaItemPropertyArtist] = author }
        MPNowPlayingInfoCenter.default().nowPlayingInfo = info
    }

    private func heard(_ span: BookSpan) {
        current = span
        follow(span)
    }

    /// A chapter's sentences, read once; reading stops, saying why, if the chapter cannot be read.
    private func sentences(of chapter: Int) async -> [BookSpeech]? {
        if let found = sentences[chapter] { return found }
        let reader = reader
        do {
            let found = try await Task.detached(priority: .userInitiated) { try reader.speech(chapter: UInt32(chapter)) }.value
            // Only the chapters near the one being read are kept.
            sentences = sentences.filter { abs($0.key - chapter) <= 1 }
            sentences[chapter] = found
            return found
        } catch {
            stop(failure: describe(error))
            return nil
        }
    }

    /// The chapter reading goes on to: the next one that is not a cover or a contents page; none at the back matter.
    private func chapter(after chapter: Int) -> Int? {
        var next = chapter + 1
        while next < sections.count, sections[next] == .cover || sections[next] == .contents { next += 1 }
        guard next < sections.count, sections[next] != .back else { return nil }
        return next
    }
}
