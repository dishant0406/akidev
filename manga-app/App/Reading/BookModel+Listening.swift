import AkiManga
import SwiftUI

extension BookModel {
    /// Whether a voice model is on the phone to read with.
    var canRead: Bool { speech?.readAloud.current != nil }

    /// Starts reading aloud, in the voice of the model in use: from the word given, or the top of the page being read.
    func listen(from word: BookSpan? = nil) {
        guard listener == nil, let (readAloud, prefs) = speech, let model = readAloud.current, let place, place.page >= 0, let layout = layouts[place.chapter] else { return }
        let listener = BookListener(
            reader: reader, engine: engine, title: info.title, author: info.authors.first, model: model.id, voice: readAloud.chosenVoice?.id, sections: sections, rate: prefs.speechRate,
            saveRate: { prefs.speechRate = $0 },
            follow: { [weak self] in self?.heard($0) },
            ended: { [weak self] failure in self?.listenerEnded(failure) }
        )
        self.listener = listener
        userMoved = false
        let (chapter, offset) = word.map { ($0.chapter, $0.start) } ?? (place.chapter, layout.start(of: place.page))
        Task { await listener.start(chapter: chapter, offset: offset) }
    }

    func stopListening() { listener?.stop() }

    /// Reads on from a word the person pointed at: from there at once if it is already reading, otherwise it starts there.
    func read(from word: BookSpan) {
        userMoved = false
        if let listener { listener.jump(chapter: word.chapter, offset: word.start) } else { listen(from: word) }
    }

    /// The sentence is marked, and the page goes where the voice is, unless the person has turned away from it: then the page stays
    /// where they put it, and follows again once they are back on the page being read.
    private func heard(_ span: BookSpan) {
        speaking = span
        let spoken = layouts[span.chapter].map { BookPlace(chapter: span.chapter, page: $0.page(containing: span.start)) }
        if userMoved {
            if let spoken, spoken == place { userMoved = false }
            return
        }
        guard spoken != place else { return }
        go(to: Spot(chapter: span.chapter, offset: span.start, speech: true), forward: true, animated: !style.ink)
    }

    private func listenerEnded(_ problem: (title: String, message: String)?) {
        listener = nil
        speaking = nil
        if let problem { failure = problem }
    }
}
