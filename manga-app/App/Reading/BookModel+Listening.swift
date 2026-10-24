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
        let (chapter, offset) = word.map { ($0.chapter, $0.start) } ?? (place.chapter, layout.start(of: place.page))
        Task { await listener.start(chapter: chapter, offset: offset) }
    }

    func stopListening() { listener?.stop() }

    /// Reads on from a word the person pointed at: from there at once if it is already reading, otherwise it starts there.
    func read(from word: BookSpan) {
        if let listener { listener.jump(chapter: word.chapter, offset: word.start) } else { listen(from: word) }
    }

    /// The page goes where the voice is, and marks the sentence.
    private func heard(_ span: BookSpan) {
        speaking = span
        if let place, place.chapter == span.chapter, place.page >= 0, let layout = layouts[span.chapter] {
            guard layout.page(containing: span.start) != place.page else { return }
        }
        go(to: Spot(chapter: span.chapter, offset: span.start), forward: true, animated: !style.ink)
    }

    private func listenerEnded(_ problem: (title: String, message: String)?) {
        listener = nil
        speaking = nil
        if let problem { failure = problem }
    }
}
