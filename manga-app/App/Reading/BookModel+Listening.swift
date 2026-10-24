import AkiManga
import SwiftUI

extension BookModel {
    /// Starts reading aloud from the top of the page being read, in the voice of the model in use.
    func listen(voice: VoiceModel, prefs: Preferences) {
        guard listener == nil, let place, place.page >= 0, let layout = layouts[place.chapter] else { return }
        let listener = BookListener(
            reader: reader, engine: engine, title: info.title, author: info.authors.first, model: voice.id, sections: sections, rate: prefs.speechRate,
            saveRate: { prefs.speechRate = $0 },
            follow: { [weak self] in self?.heard($0) },
            ended: { [weak self] failure in self?.listenerEnded(failure) }
        )
        self.listener = listener
        let (chapter, offset) = (place.chapter, layout.start(of: place.page))
        Task { await listener.start(chapter: chapter, offset: offset) }
    }

    func stopListening() { listener?.stop() }

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
