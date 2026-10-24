import AVFAudio
import Foundation

/// Plays a few seconds of speech once: what a voice sounds like, before it is chosen.
@MainActor
final class VoicePreview {
    private var player: AVAudioPlayer?

    func play(_ samples: [Float], rate: Int) {
        guard let data = Self.wav(samples, rate: rate) else { return }
        try? AVAudioSession.sharedInstance().setCategory(.playback, mode: .spokenAudio)
        player?.stop()
        player = try? AVAudioPlayer(data: data)
        player?.play()
    }

    /// The samples as a 16-bit mono WAV file, which is what the player reads from memory.
    private static func wav(_ samples: [Float], rate: Int) -> Data? {
        guard !samples.isEmpty else { return nil }
        var data = Data()
        func put<T: FixedWidthInteger>(_ value: T) { withUnsafeBytes(of: value.littleEndian) { data.append(contentsOf: $0) } }
        let bytes = samples.count * 2
        data.append(contentsOf: Array("RIFF".utf8)); put(UInt32(36 + bytes)); data.append(contentsOf: Array("WAVEfmt ".utf8))
        put(UInt32(16)); put(UInt16(1)); put(UInt16(1)); put(UInt32(rate)); put(UInt32(rate * 2)); put(UInt16(2)); put(UInt16(16))
        data.append(contentsOf: Array("data".utf8)); put(UInt32(bytes))
        for sample in samples { put(Int16(max(-1, min(1, sample)) * Float(Int16.max))) }
        return data
    }
}
