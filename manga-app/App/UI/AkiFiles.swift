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
    /// Asks for one or more files from the device and hands over copies the app may read, all together.
    func akiFilesImporter(isPresented: Binding<Bool>, types: [UTType], onPick: @escaping ([URL]) -> Void) -> some View {
        fileImporter(isPresented: isPresented, allowedContentTypes: types, allowsMultipleSelection: true) { result in
            guard case .success(let urls) = result else { return }
            let folder = FileManager.default.temporaryDirectory.appendingPathComponent("picked-\(UUID().uuidString)")
            try? FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
            let copies: [URL] = urls.compactMap { url in
                let scoped = url.startAccessingSecurityScopedResource()
                defer { if scoped { url.stopAccessingSecurityScopedResource() } }
                let copy = folder.appendingPathComponent(url.lastPathComponent)
                return (try? FileManager.default.copyItem(at: url, to: copy)) != nil ? copy : nil
            }
            onPick(copies)
        }
    }
}

extension View {
    /// Asks for a picture from the photo library (the system shows its own picker, so no permission is needed) and hands it
    /// over as a JPEG no wider than `width` points times the screen scale, small enough to keep as a cover.
    ///
    /// UIKit's picker rather than PhotosUI's: linking PhotosUI makes the app load audio, video and MIDI libraries at every
    /// launch, which measured about 800 ms in the simulator.
    func akiPhotoPicker(isPresented: Binding<Bool>, width: CGFloat = 300, onPick: @escaping (Data) -> Void) -> some View {
        modifier(PhotoPicking(isPresented: isPresented, width: width, onPick: onPick))
    }
}

private struct PhotoPicking: ViewModifier {
    @Binding var isPresented: Bool
    let width: CGFloat
    let onPick: (Data) -> Void
    @Environment(\.displayScale) private var scale

    func body(content: Content) -> some View {
        content.sheet(isPresented: $isPresented) {
            ImagePicker { image in
                isPresented = false
                if let image, let jpeg = coverJPEG(image, width: width * scale) { onPick(jpeg) }
            }
            .ignoresSafeArea()
        }
    }
}

private struct ImagePicker: UIViewControllerRepresentable {
    let done: (UIImage?) -> Void

    func makeUIViewController(context: Context) -> UIImagePickerController {
        let picker = UIImagePickerController()
        picker.sourceType = .photoLibrary
        picker.delegate = context.coordinator
        return picker
    }

    func updateUIViewController(_ picker: UIImagePickerController, context: Context) {}

    func makeCoordinator() -> Coordinator { Coordinator(done) }

    final class Coordinator: NSObject, UIImagePickerControllerDelegate, UINavigationControllerDelegate {
        let done: (UIImage?) -> Void
        init(_ done: @escaping (UIImage?) -> Void) { self.done = done }

        func imagePickerController(_ picker: UIImagePickerController, didFinishPickingMediaWithInfo info: [UIImagePickerController.InfoKey: Any]) {
            done(info[.originalImage] as? UIImage)
        }

        func imagePickerControllerDidCancel(_ picker: UIImagePickerController) { done(nil) }
    }
}

/// A picture as a JPEG for a cover: at most `width` pixels wide (never enlarged) and no taller than a book cover is. A tall
/// picture (a webtoon strip) is cut to that height around `centre`, how far down it the wanted part's middle is (0 to 1; 0 keeps
/// the top). Encoded once.
func coverJPEG(_ image: UIImage, width: CGFloat, centre: CGFloat = 0) -> Data? {
    guard image.size.width > 0, image.size.height > 0 else { return nil }
    let scale = min(1, width / image.size.width)
    let drawn = CGSize(width: (image.size.width * scale).rounded(), height: (image.size.height * scale).rounded())
    let size = CGSize(width: drawn.width, height: min(drawn.height, (drawn.width * 1.6).rounded()))
    let top = min(max(centre * drawn.height - size.height / 2, 0), drawn.height - size.height)
    let format = UIGraphicsImageRendererFormat.default()
    format.scale = 1
    return UIGraphicsImageRenderer(size: size, format: format).jpegData(withCompressionQuality: 0.85) { _ in image.draw(in: CGRect(x: 0, y: -top, width: drawn.width, height: drawn.height)) }
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
