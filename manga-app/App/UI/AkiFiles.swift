import SwiftUI
import UniformTypeIdentifiers

extension View {
    /// Asks for a file from the device and hands over a copy the app may read.
    func akiFileImporter(isPresented: Binding<Bool>, types: [UTType], onPick: @escaping (URL) -> Void) -> some View {
        fileImporter(isPresented: isPresented, allowedContentTypes: types) { result in
            guard case .success(let url) = result else { return }
            let scoped = url.startAccessingSecurityScopedResource()
            defer { if scoped { url.stopAccessingSecurityScopedResource() } }
            let copy = FileManager.default.temporaryDirectory.appendingPathComponent(url.lastPathComponent)
            try? FileManager.default.removeItem(at: copy)
            if (try? FileManager.default.copyItem(at: url, to: copy)) != nil { onPick(copy) }
        }
    }
}

/// Hands a file to the share sheet (save to Files, AirDrop, Messages).
struct AkiShare: View {
    let title: String
    let icon: String
    let file: URL

    var body: some View {
        ShareLink(item: file) {
            HStack(spacing: 8) {
                AkiIcon(icon, size: 15, tone: .onYellow)
                AkiText(title, .bodyStrong, tone: .onYellow)
            }
            .padding(.horizontal, 22).frame(height: 50)
            .background(Aki.yellow, in: Capsule())
        }
    }
}
