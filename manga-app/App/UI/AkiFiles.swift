import PhotosUI
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
            if (try? FileManager.default.copyItem(at: url, to: copy)) != nil {
                onPick(copy)
                try? FileManager.default.removeItem(at: copy)
            }
        }
    }
}

extension View {
    /// Asks for a picture from the photo library (no permission needed: the system shows its own picker) and hands it
    /// over as a JPEG no wider than `width` points times the screen scale, small enough to keep as a cover.
    func akiPhotoPicker(isPresented: Binding<Bool>, width: CGFloat = 300, onPick: @escaping (Data) -> Void) -> some View {
        modifier(PhotoPicking(isPresented: isPresented, width: width, onPick: onPick))
    }
}

private struct PhotoPicking: ViewModifier {
    @Binding var isPresented: Bool
    let width: CGFloat
    let onPick: (Data) -> Void
    @State private var item: PhotosPickerItem?
    @Environment(\.displayScale) private var scale

    func body(content: Content) -> some View {
        content
            .photosPicker(isPresented: $isPresented, selection: $item, matching: .images)
            .onChange(of: item) { _, picked in
                guard let picked else { return }
                item = nil
                Task {
                    guard let data = try? await picked.loadTransferable(type: Data.self), let jpeg = coverJPEG(data, width: width * scale) else { return }
                    onPick(jpeg)
                }
            }
    }
}

/// A picture's bytes as a JPEG at most `width` pixels wide (never enlarged): a cover is shown small.
func coverJPEG(_ data: Data, width: CGFloat) -> Data? {
    guard let image = UIImage(data: data), image.size.width > 0 else { return nil }
    let scale = min(1, width / image.size.width)
    let size = CGSize(width: (image.size.width * scale).rounded(), height: (image.size.height * scale).rounded())
    let format = UIGraphicsImageRendererFormat.default()
    format.scale = 1
    return UIGraphicsImageRenderer(size: size, format: format).jpegData(withCompressionQuality: 0.85) { _ in image.draw(in: CGRect(origin: .zero, size: size)) }
}

/// Hands a file to the share sheet (save to Files, AirDrop, Messages).
struct AkiShare: View {
    let title: String
    let icon: String
    let file: URL

    var body: some View {
        ShareLink(item: file) {
            HStack(spacing: 8) {
                AkiIcon(icon, size: 15, tone: .onPrimary)
                AkiText(title, .bodyStrong, tone: .onPrimary)
            }
            .padding(.horizontal, 22).frame(height: 50)
            .background(Aki.primary, in: Capsule())
        }
    }
}
